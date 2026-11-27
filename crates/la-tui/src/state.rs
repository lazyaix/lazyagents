//! Unidirectional TEA application state (`AppState`) for the 3-Zone Cyberpunk Command Deck.
//!
//! `AppState` holds pure domain snapshots from `la-core`, recomputes compilation, budgets,
//! diffs, and `LA001`–`LA017` diagnostics deterministically in-memory on every edit, and emits
//! [`StudioCommand`] intents without performing any filesystem or network I/O.

mod governance;
mod modal;
mod resources;

use core::time::Duration;
use std::collections::{BTreeMap, VecDeque};

use la_core::manifest::GitOutputMode;
use la_core::{
    CompileInputs, CompileOutput, ConfigScope, DiagnosticSeverity, LintInputs, LintReport,
    Lockfile, ProjectFacts, ProjectManifest, WritePlan, compile_all, lint_project, plan_writes,
    redact_ghs_tokens,
};
use ratatui::layout::Rect;

use crate::action::{Action, StudioCommand, StudioPreferences};
use crate::config::{MotionLevel, TuiConfig};
use crate::editor::TextEditor;
use crate::effects::StudioEffects;
use crate::highlight::PreviewRenderer;
use crate::i18n::I18n;
use crate::media::PluginLogoPreview;
use crate::protocol::{BackgroundTone, TerminalCapabilities};
use crate::theme::{ResolvedTheme, Theme};

pub use self::governance::{GovernanceDomain, GovernanceRow, governance_rows};
pub use self::modal::{
    CommandPaletteState, FilteredPaletteEntry, HookPlaygroundState, HookVerdictView,
    InitWizardState, InlineEditState, PaletteCommand, all_palette_commands,
};

/// Maximum splash display duration before auto-entering the Command Deck (540ms).
const SPLASH_DURATION_MS: u64 = 540;

/// Maximum log entries retained in [`LogRingBuffer`].
const MAX_LOG_ENTRIES: usize = 200;

/// Maximum toast banner display lifetime in milliseconds.
const TOAST_TTL_MS: u64 = 3_200;

/// Top-level navigation sections displayed in the Left Neon Navigation Rail.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum NavSection {
    /// HUD Overview, Governance Health Gauge, and `GraphType::Area` Budget Radar.
    #[default]
    Dashboard,
    /// 7-domain `[engineering]` declarative governance policy workbench.
    Engineering,
    /// Asynchronous read-only `gh` CLI Rulesets & merge-strategy audit matrix.
    GitHubAudit,
    /// Dynamic `AGENTS.md` instruction plans and modular section ordering.
    Instructions,
    /// Conditional `.agents/rules/*.md` rules (`always_on` / `glob` / `model_decision` / `manual`).
    Rules,
    /// Skill bundles (`.agents/skills/<name>/SKILL.md`).
    Skills,
    /// Custom subagents (`.agents/agents/<name>.md`).
    Subagents,
    /// Lifecycle hooks (`.agents/hooks.json`) + interactive Hook Policy Playground.
    Hooks,
    /// MCP server configurations (Stdio & Streamable HTTP + `${VAR}` declarations).
    McpServers,
    /// Declarative Antigravity JSON index manifests (`skills.json` / `rules.json` / `plugins.json`).
    Manifests,
    /// Cohesive project-level plugins (`.agents/plugins/<name>/`).
    Plugins,
    /// Unified / side-by-side Diff & safe Sync center with Conflict Guard.
    SyncDiff,
}

/// Active focus region inside the 3-Zone Cyberpunk Command Deck.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum FocusPane {
    /// Left collapsible navigation rail.
    Rail,
    /// Center primary workspace pane.
    #[default]
    Workspace,
    /// Right toggleable live inspector pod.
    Inspector,
    /// Bottom collapsible redacted telemetry/logs drawer.
    LogsDrawer,
}

/// Active tab inside the Right Live Inspector Pod.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum InspectorTab {
    /// Live syntax-highlighted Markdown/TOML/JSON/YAML compiled preview.
    #[default]
    Preview,
    /// Live unified or side-by-side diff against disk.
    Diff,
    /// Structured `LA001`–`LA017` compliance diagnostics.
    Diagnostics,
    /// Per-file 24 KB and 20,000-token budget breakdown & offload suggestions.
    Budget,
}

/// Floating modal overlays rendered with `Block::shadow` and `TachyonFX` dimmed background scrim.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ModalKind {
    /// Fuzzy-searchable `Ctrl+K` Command Palette.
    CommandPalette,
    /// Keybindings `?` cheat sheet.
    Help,
    /// External Conflict Guard confirmation modal shown before overwriting hand-edited files.
    ConflictGuard,
    /// Guided First-Run Init & Import Wizard when `.lazyagents/` is missing.
    InitWizard,
}

/// Severity tier for transient toast notifications.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ToastLevel {
    /// Informational feedback.
    Info,
    /// Operation succeeded cleanly.
    Success,
    /// Non-fatal warning or drift alert.
    Warning,
    /// Error requiring user attention.
    Error,
}

/// Transient toast notification displayed at the bottom-right of the HUD.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToastMessage {
    /// Severity tier.
    pub level: ToastLevel,
    /// Concise message text.
    pub message: String,
    /// Remaining display lifetime in milliseconds.
    pub remaining_ms: u64,
}

/// Decoupled status enum mirroring `la-runtime::AuditCheckStatus` without depending on `la-runtime`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AuditStatusView {
    /// Remote repository setting matches `.lazyagents/lazyagents.toml`.
    Pass,
    /// Remote setting diverges from declared policy.
    Mismatch,
    /// Check could not be evaluated (e.g., optional API permissions).
    Skipped,
}

/// Single read-only GitHub governance check displayed in the GitHub Audit matrix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditCheckView {
    /// Stable check identifier.
    pub id: String,
    /// Human-readable check title.
    pub label: String,
    /// Pass / Mismatch / Skipped status.
    pub status: AuditStatusView,
    /// Concise explanation of observed vs expected state.
    pub detail: String,
    /// Optional copyable `gh api` remediation command (never auto-executed).
    pub remediation_cmd: Option<String>,
}

/// Complete read-only GitHub audit snapshot fed asynchronously by `la-cli`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitHubAuditSnapshot {
    /// Audited repository slug (`owner/name`).
    pub repo: String,
    /// Ordered audit checks.
    pub checks: Vec<AuditCheckView>,
}

/// Structured tracing log severity for the in-TUI redacted log drawer.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LogLevel {
    /// Debug diagnostic message.
    Debug,
    /// Standard operational milestone.
    Info,
    /// Non-fatal warning.
    Warn,
    /// Error condition.
    Error,
}

/// Single log line stored inside [`LogRingBuffer`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogEntry {
    /// Compact timestamp string (e.g., `"20:41:08"`).
    pub timestamp: String,
    /// Severity level.
    pub level: LogLevel,
    /// Tracing target/module name.
    pub target: String,
    /// Always secret-redacted message text.
    pub message: String,
}

/// Bounded in-memory tracing ring buffer that unconditionally redacts `ghs_*` & secrets on push.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LogRingBuffer {
    entries: VecDeque<LogEntry>,
}

/// Pure Sans-I/O snapshot of a loaded `.lazyagents/` workspace paired with live compiled outputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSnapshot {
    /// Display root path string (for HUD breadcrumbs).
    pub root_display: String,
    /// Whether `.lazyagents/lazyagents.toml` exists on disk (false triggers Init Wizard).
    pub initialized: bool,
    /// Raw manifest TOML (updated automatically on in-TUI mutations for `LA001` scanning).
    pub raw_manifest_toml: String,
    /// Scope-resolved effective canonical manifest.
    pub manifest: ProjectManifest,
    /// Active scopes participating in resolution.
    pub active_scopes: Vec<ConfigScope>,
    /// Winning scope provenance keyed by logical resource key.
    pub provenance: BTreeMap<String, ConfigScope>,
    /// Probed repository metadata facts.
    pub facts: ProjectFacts,
    /// `.lazyagents/`-relative source file contents.
    pub source_files: BTreeMap<String, String>,
    /// Existing workspace files on disk for diffing, includes, and GitHub Actions linting.
    pub workspace_files: BTreeMap<String, String>,
    /// Previously persisted `.lazyagents/lock.json`.
    pub lockfile: Option<Lockfile>,
    /// Live deterministically compiled target files & compatibility diagnostics.
    pub compiled: CompileOutput,
    /// Live write & drift plan against disk and `lock.json`.
    pub write_plan: WritePlan,
    /// Live `LA001`–`LA017` compliance report & Antigravity context budget telemetry.
    pub lint_report: LintReport,
    /// Optional compilation error if a live edit violated a structural invariant.
    pub compile_error: Option<String>,
}

/// Rendered region hitboxes updated every frame for accurate mouse click & scroll targeting.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LayoutGeometry {
    /// Left navigation rail outer area.
    pub rail_area: Rect,
    /// Center primary workspace outer area.
    pub workspace_area: Rect,
    /// Right inspector pod outer area.
    pub inspector_area: Rect,
    /// Bottom logs drawer outer area.
    pub logs_area: Rect,
    /// Vertical start row of clickable items inside `rail_area`.
    pub rail_items_top: u16,
}

/// Visibility toggles for collapsible panes of the 3-Zone Command Deck.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeckVisibility {
    /// Whether the left navigation rail is expanded.
    pub rail: bool,
    /// Whether the right live inspector pod is expanded.
    pub inspector: bool,
    /// Whether the bottom redacted logs drawer is open.
    pub logs_drawer: bool,
}

impl Default for DeckVisibility {
    fn default() -> Self {
        Self {
            rail: true,
            inspector: true,
            logs_drawer: false,
        }
    }
}

/// Diff presentation style inside Diff views.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum DiffStyle {
    /// Classic unified hunk diff (`---`/`+++`/`@@`).
    #[default]
    Unified,
    /// Two-column word-level highlighted side-by-side diff.
    SideBySide,
}

/// Complete unidirectional TEA state driving the `la-tui` Config Studio.
#[derive(Debug)]
pub struct AppState {
    /// Runtime visual/motion/language/viewport configuration.
    pub config: TuiConfig,
    /// Detected terminal capabilities (OSC 11 light/dark, SSH, `tmux`, color profile).
    pub capabilities: TerminalCapabilities,
    /// Resolved semantic design token theme.
    pub theme: Theme,
    /// Loaded workspace data, live compile outputs, diffs, and diagnostics.
    pub workspace: WorkspaceSnapshot,
    /// Active left-rail navigation section.
    pub nav: NavSection,
    /// Active focused Command Deck pane.
    pub focus: FocusPane,
    /// Active tab in the right live inspector pod.
    pub inspector_tab: InspectorTab,
    /// Collapsible Command Deck pane visibility flags.
    pub visibility: DeckVisibility,
    /// Active diff rendering style (`Unified` or `SideBySide`).
    pub diff_style: DiffStyle,
    /// Active floating modal (if any).
    pub modal: Option<ModalKind>,
    /// Whether the animated startup splash is currently visible.
    pub splash_active: bool,
    /// Elapsed splash milliseconds (auto-dismisses after 540 ms).
    pub splash_elapsed_ms: u64,
    /// Active domain tab inside the `[engineering]` Governance view.
    pub governance_domain: GovernanceDomain,
    /// Selected row index inside the active `[engineering]` domain.
    pub governance_row: usize,
    /// Selected row index inside the active resource list or audit matrix.
    pub list_index: usize,
    /// Selected section index inside the active `AGENTS.md` instruction plan.
    pub section_index: usize,
    /// Active fuzzy Command Palette modal state.
    pub palette: CommandPaletteState,
    /// Guided First-Run Init & Import Wizard state.
    pub wizard: InitWizardState,
    /// Interactive Hook Policy Playground state inside the Hooks & Lab workspace.
    pub playground: HookPlaygroundState,
    /// Active native grapheme inline editor session (when editing a backing Markdown body).
    pub inline_edit: Option<InlineEditState>,
    /// Pre-decoded `ratatui-image` plugin logo protocols keyed by plugin name.
    pub plugin_logos: BTreeMap<String, PluginLogoPreview>,
    /// Vertical scroll offset inside the right live inspector pod.
    pub inspector_scroll: u16,
    /// Vertical scroll offset inside the bottom logs drawer.
    pub logs_scroll: u16,
    /// Selected compiled file index for Preview / Diff inspection.
    pub selected_output_idx: usize,
    /// Latest asynchronous read-only GitHub audit report (if fetched).
    pub github_audit: Option<GitHubAuditSnapshot>,
    /// Whether a background `gh` audit is currently in flight.
    pub github_audit_loading: bool,
    /// Last GitHub audit error message (if any).
    pub github_audit_error: Option<String>,
    /// Bounded secret-redacted tracing ring buffer.
    pub logs: LogRingBuffer,
    /// Active transient toast message.
    pub toast: Option<ToastMessage>,
    /// Active `TachyonFX` shader pipeline.
    pub effects: StudioEffects,
    /// Cached pure-Rust Markdown/syntax highlighter.
    pub preview_renderer: PreviewRenderer,
    /// Last rendered layout hitboxes for mouse click/scroll routing.
    pub geometry: LayoutGeometry,
    /// Set to `true` once the user confirms exiting.
    pub should_quit: bool,
}

impl NavSection {
    /// Complete ordered list of all 12 Command Deck navigation sections.
    pub const ALL: [Self; 12] = [
        Self::Dashboard,
        Self::Engineering,
        Self::GitHubAudit,
        Self::Instructions,
        Self::Rules,
        Self::Skills,
        Self::Subagents,
        Self::Hooks,
        Self::McpServers,
        Self::Manifests,
        Self::Plugins,
        Self::SyncDiff,
    ];

    /// Advances to the next section with wrap-around.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Dashboard => Self::Engineering,
            Self::Engineering => Self::GitHubAudit,
            Self::GitHubAudit => Self::Instructions,
            Self::Instructions => Self::Rules,
            Self::Rules => Self::Skills,
            Self::Skills => Self::Subagents,
            Self::Subagents => Self::Hooks,
            Self::Hooks => Self::McpServers,
            Self::McpServers => Self::Manifests,
            Self::Manifests => Self::Plugins,
            Self::Plugins => Self::SyncDiff,
            Self::SyncDiff => Self::Dashboard,
        }
    }

    /// Moves to the previous section with wrap-around.
    #[must_use]
    pub const fn prev(self) -> Self {
        match self {
            Self::Dashboard => Self::SyncDiff,
            Self::Engineering => Self::Dashboard,
            Self::GitHubAudit => Self::Engineering,
            Self::Instructions => Self::GitHubAudit,
            Self::Rules => Self::Instructions,
            Self::Skills => Self::Rules,
            Self::Subagents => Self::Skills,
            Self::Hooks => Self::Subagents,
            Self::McpServers => Self::Hooks,
            Self::Manifests => Self::McpServers,
            Self::Plugins => Self::Manifests,
            Self::SyncDiff => Self::Plugins,
        }
    }

    /// Returns the localized display label for this navigation section.
    #[must_use]
    pub const fn label(self, i18n: I18n) -> &'static str {
        match self {
            Self::Dashboard => i18n.nav_dashboard(),
            Self::Engineering => i18n.nav_engineering(),
            Self::GitHubAudit => i18n.nav_github_audit(),
            Self::Instructions => i18n.nav_instructions(),
            Self::Rules => i18n.nav_rules(),
            Self::Skills => i18n.nav_skills(),
            Self::Subagents => i18n.nav_agents(),
            Self::Hooks => i18n.nav_hooks(),
            Self::McpServers => i18n.nav_mcp(),
            Self::Manifests => i18n.nav_manifests(),
            Self::Plugins => i18n.nav_plugins(),
            Self::SyncDiff => i18n.nav_sync_diff(),
        }
    }

    /// Returns the glyph icon from `theme.glyphs` for this section.
    #[must_use]
    pub const fn glyph(self, theme: &Theme) -> &'static str {
        match self {
            Self::Dashboard => theme.glyphs.dashboard,
            Self::Engineering | Self::GitHubAudit => theme.glyphs.governance,
            Self::Instructions => theme.glyphs.instructions,
            Self::Rules => theme.glyphs.rules,
            Self::Skills => theme.glyphs.skills,
            Self::Subagents => theme.glyphs.agents,
            Self::Hooks => theme.glyphs.hooks,
            Self::McpServers => theme.glyphs.mcp,
            Self::Manifests => theme.glyphs.manifests,
            Self::Plugins => theme.glyphs.plugins,
            Self::SyncDiff => theme.glyphs.diff,
        }
    }
}

impl InspectorTab {
    /// Cycles to the next tab inside the right live inspector pod.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Preview => Self::Diff,
            Self::Diff => Self::Diagnostics,
            Self::Diagnostics => Self::Budget,
            Self::Budget => Self::Preview,
        }
    }

    /// Cycles to the previous tab inside the right live inspector pod.
    #[must_use]
    pub const fn prev(self) -> Self {
        match self {
            Self::Preview => Self::Budget,
            Self::Diff => Self::Preview,
            Self::Diagnostics => Self::Diff,
            Self::Budget => Self::Diagnostics,
        }
    }
}

impl LogRingBuffer {
    /// Pushes a log entry, unconditionally redacting stateless `ghs_*` and sensitive tokens.
    pub fn push(&mut self, mut entry: LogEntry) {
        entry.message = redact_ghs_tokens(&entry.message);
        if self.entries.len() >= MAX_LOG_ENTRIES {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    /// Returns the number of buffered log entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when no log entries are buffered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterates over buffered log entries from oldest to newest.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &LogEntry> {
        self.entries.iter()
    }
}

impl WorkspaceSnapshot {
    /// Creates a live [`WorkspaceSnapshot`] from raw domain pieces and computes its initial
    /// compilation, write plan, budgets, and `LA001`–`LA017` diagnostics in-memory.
    #[must_use]
    pub fn from_manifest(
        root_display: impl Into<String>,
        initialized: bool,
        manifest: ProjectManifest,
        facts: ProjectFacts,
        source_files: BTreeMap<String, String>,
        workspace_files: BTreeMap<String, String>,
        lockfile: Option<Lockfile>,
    ) -> Self {
        let raw_manifest_toml = manifest.to_toml_pretty().unwrap_or_default();
        let mut snapshot = Self {
            root_display: root_display.into(),
            initialized,
            raw_manifest_toml,
            manifest,
            active_scopes: vec![ConfigScope::Project],
            provenance: BTreeMap::new(),
            facts,
            source_files,
            workspace_files,
            lockfile,
            compiled: CompileOutput::default(),
            write_plan: WritePlan {
                files: Vec::new(),
                next_lock: Lockfile::default(),
            },
            lint_report: LintReport {
                diagnostics: Vec::new(),
                budget: la_core::BudgetReport {
                    files: Vec::new(),
                    always_on_tokens: 0,
                    max_always_on_tokens: la_core::manifest::DEFAULT_ALWAYS_ON_MAX_TOKENS,
                    always_on_status: la_core::BudgetStatus::Ok,
                    offload_suggestions: Vec::new(),
                },
            },
            compile_error: None,
        };
        snapshot.recompute();
        snapshot
    }

    /// Re-runs `la-core` compilation, diff planning, budget telemetry, and `LA001`–`LA017` linting
    /// purely in-memory (< 1 ms, zero disk I/O).
    pub fn recompute(&mut self) {
        if let Ok(pretty) = self.manifest.to_toml_pretty() {
            self.raw_manifest_toml = pretty;
        }
        let inputs = CompileInputs {
            facts: self.facts.clone(),
            source_files: self.source_files.clone(),
            target_filter: None,
        };
        match compile_all(&self.manifest, &inputs) {
            Ok(compiled) => {
                self.compile_error = None;
                let plan = plan_writes(
                    &compiled.files,
                    self.lockfile.as_ref(),
                    &self.workspace_files,
                )
                .unwrap_or_else(|err| {
                    self.compile_error = Some(err.to_string());
                    WritePlan {
                        files: Vec::new(),
                        next_lock: Lockfile::default(),
                    }
                });
                let report = lint_project(&LintInputs {
                    manifest_toml: &self.raw_manifest_toml,
                    manifest: &self.manifest,
                    source_files: &self.source_files,
                    compile_output: &compiled,
                    write_plan: Some(&plan),
                    workspace_files: &self.workspace_files,
                });
                self.compiled = compiled;
                self.write_plan = plan;
                self.lint_report = report;
            }
            Err(err) => {
                self.compile_error = Some(err.to_string());
            }
        }
    }

    /// Counts `(errors, warnings)` across the current `LA001`–`LA017` diagnostics.
    #[must_use]
    pub fn diagnostic_counts(&self) -> (usize, usize) {
        let mut errors = 0usize;
        let mut warns = 0usize;
        for diag in &self.lint_report.diagnostics {
            match diag.severity {
                DiagnosticSeverity::Error => errors = errors.saturating_add(1),
                DiagnosticSeverity::Warning => warns = warns.saturating_add(1),
                DiagnosticSeverity::Info => {}
            }
        }
        (errors, warns)
    }

    /// Computes a deterministic 0–100 Governance Health Score for the Cyber HUD Gauge.
    #[must_use]
    pub fn health_score(&self) -> u8 {
        let (errors, warns) = self.diagnostic_counts();
        let mut score: u16 = 100;
        let err_penalty = u16::try_from(errors).unwrap_or(10).saturating_mul(18);
        let warn_penalty = u16::try_from(warns).unwrap_or(10).saturating_mul(6);
        score = score
            .saturating_sub(err_penalty)
            .saturating_sub(warn_penalty);
        if self.write_plan.has_conflicts() {
            score = score.saturating_sub(15);
        } else if self.write_plan.has_drift() {
            score = score.saturating_sub(5);
        }
        if self.compile_error.is_some() {
            score = score.saturating_sub(25);
        }
        u8::try_from(score.min(100)).unwrap_or(0)
    }
}

impl AppState {
    /// Creates a new [`AppState`] initialized with resolved theme, shader pipeline, and workspace.
    #[must_use]
    pub fn new(
        mut config: TuiConfig,
        capabilities: TerminalCapabilities,
        workspace: WorkspaceSnapshot,
    ) -> Self {
        config.motion = capabilities.effective_motion(config.motion);
        let is_light = matches!(capabilities.background, BackgroundTone::Light);
        let resolved_theme = ResolvedTheme::resolve(config.theme, is_light);
        let theme = Theme::build(resolved_theme, config.color_profile, config.glyphs);
        let splash_active = config.show_splash && config.motion != MotionLevel::None;
        let effects = StudioEffects::new(config.shader_preset, config.motion);
        let modal = if workspace.initialized {
            None
        } else {
            Some(ModalKind::InitWizard)
        };

        Self {
            config,
            capabilities,
            theme,
            workspace,
            nav: NavSection::Dashboard,
            focus: FocusPane::Workspace,
            inspector_tab: InspectorTab::Preview,
            visibility: DeckVisibility::default(),
            diff_style: DiffStyle::Unified,
            modal,
            splash_active,
            splash_elapsed_ms: 0,
            governance_domain: GovernanceDomain::Git,
            governance_row: 0,
            list_index: 0,
            section_index: 0,
            palette: CommandPaletteState::default(),
            wizard: InitWizardState::default(),
            playground: HookPlaygroundState::default(),
            inline_edit: None,
            plugin_logos: BTreeMap::new(),
            inspector_scroll: 0,
            logs_scroll: 0,
            selected_output_idx: 0,
            github_audit: None,
            github_audit_loading: false,
            github_audit_error: None,
            logs: LogRingBuffer::default(),
            toast: None,
            effects,
            preview_renderer: PreviewRenderer::new(),
            geometry: LayoutGeometry::default(),
            should_quit: false,
        }
    }

    /// Returns the active [`I18n`] catalog for the current UI language.
    #[must_use]
    pub const fn i18n(&self) -> I18n {
        I18n::new(self.config.lang)
    }

    /// Returns `true` when the adaptive frame loop should schedule active animation ticks.
    #[must_use]
    pub fn needs_animation_tick(&self) -> bool {
        self.splash_active
            || self.toast.is_some()
            || self.github_audit_loading
            || self.effects.needs_animation_tick(&self.theme)
    }

    /// Applies one [`Action`] to `self` and returns zero or more [`StudioCommand`] side effects.
    pub fn update(&mut self, action: Action) -> Vec<StudioCommand> {
        if self.splash_active {
            return self.update_during_splash(action);
        }

        match action {
            Action::SkipSplash => {
                self.splash_active = false;
                Vec::new()
            }
            Action::SelectNav(_)
            | Action::NextNav
            | Action::PrevNav
            | Action::MoveDown
            | Action::MoveUp
            | Action::MoveLeft
            | Action::MoveRight
            | Action::JumpTop
            | Action::JumpBottom
            | Action::CycleFocusPane
            | Action::FocusPane(_)
            | Action::ToggleRail
            | Action::ToggleInspector
            | Action::SelectInspectorTab(_)
            | Action::ToggleDiffMode
            | Action::ToggleLogsDrawer
            | Action::ScrollLines(_) => {
                self.update_navigation(&action);
                Vec::new()
            }
            Action::ActivateSelected => self.handle_activate(),
            Action::CycleItemProperty => self.handle_cycle_property(),
            Action::ReorderItem { up } => self.handle_reorder_section(up),
            Action::StartInlineEdit => {
                self.start_inline_edit();
                Vec::new()
            }
            Action::SaveInlineEdit => self.save_inline_edit(),
            Action::CancelInlineEdit => {
                self.inline_edit = None;
                Vec::new()
            }
            Action::ToggleVimMode => {
                if let Some(ref mut edit) = self.inline_edit {
                    let next = !edit.editor.vim_enabled();
                    edit.editor.set_vim_enabled(next);
                }
                Vec::new()
            }
            Action::OpenExternalEditor => self.handle_open_external_editor(),
            Action::EditorInput(ed_action) => {
                if let Some(ref mut edit) = self.inline_edit {
                    edit.editor.apply(ed_action);
                }
                Vec::new()
            }
            Action::PaletteInput(_)
            | Action::PaletteBackspace
            | Action::PaletteConfirm
            | Action::WizardNext
            | Action::WizardPrev
            | Action::WizardToggleOption
            | Action::WizardConfirm
            | Action::PlaygroundToggleFocus
            | Action::PlaygroundInput(_)
            | Action::PlaygroundBackspace
            | Action::PlaygroundCycleGuard
            | Action::PlaygroundNextSample
            | Action::PlaygroundEvaluate => self.update_modal_or_playground(&action),
            Action::Resize(_, _) => Vec::new(),
            Action::RequestSync { force } => self.handle_request_sync(force),
            Action::TriggerGitHubAudit => {
                self.github_audit_loading = true;
                self.github_audit_error = None;
                vec![StudioCommand::RunGitHubAudit]
            }
            Action::CopySelected => self.handle_copy_selected(),
            Action::ReloadWorkspace => vec![StudioCommand::ReloadWorkspace],
            Action::CycleTheme => self.cycle_theme(),
            Action::CycleShaderPreset => self.cycle_shader_preset(),
            Action::CycleMotion => self.cycle_motion(),
            Action::ToggleLang => self.toggle_lang(),
            Action::OpenModal(kind) => {
                if kind == ModalKind::CommandPalette {
                    self.palette.reset();
                }
                self.modal = Some(kind);
                self.effects
                    .trigger_modal_entrance(self.geometry.workspace_area, &self.theme);
                Vec::new()
            }
            Action::CloseModal => {
                self.modal = None;
                Vec::new()
            }
            Action::MouseClick { col, row } => self.handle_mouse_click(col, row),
            Action::MouseScroll { col, row, up } => {
                self.handle_mouse_scroll(col, row, up);
                Vec::new()
            }
            Action::Tick(dt) => {
                self.advance_timers(dt);
                Vec::new()
            }
            Action::WindowFocus(focused) => {
                self.effects.set_window_focused(focused);
                Vec::new()
            }
            Action::Quit => {
                self.should_quit = true;
                vec![StudioCommand::Quit]
            }
            other => self.update_async_event(other),
        }
    }

    fn update_during_splash(&mut self, action: Action) -> Vec<StudioCommand> {
        match action {
            Action::Tick(dt) => {
                let dt_ms = u64::try_from(dt.as_millis()).unwrap_or(16);
                self.splash_elapsed_ms = self.splash_elapsed_ms.saturating_add(dt_ms);
                if self.splash_elapsed_ms >= SPLASH_DURATION_MS {
                    self.splash_active = false;
                }
            }
            Action::Quit => {
                self.should_quit = true;
                return vec![StudioCommand::Quit];
            }
            Action::LogReceived(entry) => self.logs.push(entry),
            Action::Resize(_, _) | Action::WindowFocus(_) => {}
            _ => self.splash_active = false,
        }
        Vec::new()
    }

    fn update_navigation(&mut self, action: &Action) {
        match *action {
            Action::SelectNav(section) => self.switch_nav(section),
            Action::NextNav => self.switch_nav(self.nav.next()),
            Action::PrevNav => self.switch_nav(self.nav.prev()),
            Action::MoveDown => self.handle_vertical_move(1),
            Action::MoveUp => self.handle_vertical_move(-1),
            Action::MoveLeft => self.handle_horizontal_move(false),
            Action::MoveRight => self.handle_horizontal_move(true),
            Action::JumpTop => self.jump_edge(true),
            Action::JumpBottom => self.jump_edge(false),
            Action::CycleFocusPane => self.cycle_focus(),
            Action::FocusPane(pane) => self.focus = pane,
            Action::ToggleRail => {
                self.visibility.rail = !self.visibility.rail;
                if !self.visibility.rail && self.focus == FocusPane::Rail {
                    self.focus = FocusPane::Workspace;
                }
            }
            Action::ToggleInspector => {
                self.visibility.inspector = !self.visibility.inspector;
                if !self.visibility.inspector && self.focus == FocusPane::Inspector {
                    self.focus = FocusPane::Workspace;
                }
            }
            Action::SelectInspectorTab(tab) => {
                self.inspector_tab = tab;
                self.inspector_scroll = 0;
            }
            Action::ToggleDiffMode => {
                self.diff_style = match self.diff_style {
                    DiffStyle::Unified => DiffStyle::SideBySide,
                    DiffStyle::SideBySide => DiffStyle::Unified,
                };
            }
            Action::ToggleLogsDrawer => {
                self.visibility.logs_drawer = !self.visibility.logs_drawer;
                if self.visibility.logs_drawer {
                    self.focus = FocusPane::LogsDrawer;
                } else if self.focus == FocusPane::LogsDrawer {
                    self.focus = FocusPane::Workspace;
                }
            }
            Action::ScrollLines(delta) => self.scroll_active(delta),
            _ => {}
        }
    }

    fn update_async_event(&mut self, action: Action) -> Vec<StudioCommand> {
        match action {
            Action::WorkspaceLoaded(ws) => {
                self.workspace = *ws;
                if self.workspace.initialized && self.modal == Some(ModalKind::InitWizard) {
                    self.modal = None;
                }
                if self.selected_output_idx >= self.workspace.compiled.files.len() {
                    self.selected_output_idx = 0;
                }
                self.push_toast(
                    ToastLevel::Info,
                    self.i18n()
                        .pick("Workspace reloaded", "工作区配置已重新加载")
                        .to_owned(),
                );
                Vec::new()
            }
            Action::PluginLogoDecoded { plugin_name, image } => {
                if let Ok(preview) =
                    PluginLogoPreview::from_rgba(&image, self.capabilities.graphics)
                {
                    self.plugin_logos.insert(plugin_name, preview);
                }
                Vec::new()
            }
            Action::GitHubAuditLoaded(report) => {
                self.github_audit_loading = false;
                self.github_audit_error = None;
                self.github_audit = Some(report);
                self.push_toast(
                    ToastLevel::Success,
                    self.i18n()
                        .pick("GitHub audit updated", "GitHub 治理审计已刷新")
                        .to_owned(),
                );
                Vec::new()
            }
            Action::GitHubAuditFailed(err) => {
                self.github_audit_loading = false;
                self.github_audit_error = Some(err.clone());
                self.push_toast(ToastLevel::Error, err);
                Vec::new()
            }
            Action::SyncCompleted {
                created,
                updated,
                deleted,
            } => {
                self.modal = None;
                self.push_toast(
                    ToastLevel::Success,
                    format!("Sync complete: +{created} ~{updated} -{deleted}"),
                );
                vec![StudioCommand::ReloadWorkspace]
            }
            Action::SyncFailed(err) => {
                self.push_toast(ToastLevel::Error, err);
                Vec::new()
            }
            Action::LogReceived(entry) => {
                self.logs.push(entry);
                Vec::new()
            }
            Action::Notify { level, message } => {
                self.push_toast(level, message);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// Returns the currently selected compiled output file (if any).
    #[must_use]
    pub fn selected_output(&self) -> Option<&la_core::GeneratedFile> {
        self.workspace
            .compiled
            .files
            .get(self.selected_output_idx)
            .or_else(|| self.workspace.compiled.files.first())
    }

    fn switch_nav(&mut self, section: NavSection) {
        if self.nav != section {
            self.nav = section;
            self.list_index = 0;
            self.governance_row = 0;
            self.inspector_scroll = 0;
            self.effects
                .trigger_view_transition(self.geometry.workspace_area, &self.theme);
        }
    }

    fn handle_vertical_move(&mut self, delta: i32) {
        if self.modal == Some(ModalKind::CommandPalette) {
            let count = self.palette.filtered(self.i18n()).len();
            self.palette.selected = shift_index(self.palette.selected, count, delta);
            return;
        }
        if self.modal == Some(ModalKind::InitWizard) && self.wizard.step == 1 {
            self.wizard.option_index = shift_index(self.wizard.option_index, 2, delta);
            return;
        }
        match self.focus {
            FocusPane::Rail => {
                if delta > 0 {
                    self.switch_nav(self.nav.next());
                } else {
                    self.switch_nav(self.nav.prev());
                }
            }
            FocusPane::Inspector => {
                self.scroll_active(if delta > 0 { 2 } else { -2 });
            }
            FocusPane::LogsDrawer => {
                self.scroll_active(if delta > 0 { 1 } else { -1 });
            }
            FocusPane::Workspace => match self.nav {
                NavSection::Engineering => {
                    let count = governance_rows(
                        self.governance_domain,
                        &self.workspace.manifest.engineering,
                        self.i18n(),
                    )
                    .len();
                    self.governance_row = shift_index(self.governance_row, count, delta);
                }
                NavSection::Instructions => {
                    let count = self
                        .workspace
                        .manifest
                        .instructions
                        .get(self.list_index)
                        .map_or(0, |inst| inst.sections.len());
                    self.section_index = shift_index(self.section_index, count, delta);
                }
                NavSection::GitHubAudit => {
                    let count = self
                        .github_audit
                        .as_ref()
                        .map_or(0, |report| report.checks.len());
                    self.list_index = shift_index(self.list_index, count, delta);
                }
                NavSection::SyncDiff => {
                    let count = self.workspace.write_plan.files.len();
                    self.selected_output_idx = shift_index(self.selected_output_idx, count, delta);
                }
                _ => {
                    let count = self.current_resource_count();
                    self.list_index = shift_index(self.list_index, count, delta);
                }
            },
        }
    }

    fn handle_horizontal_move(&mut self, right: bool) {
        match self.focus {
            FocusPane::Inspector => {
                self.inspector_tab = if right {
                    self.inspector_tab.next()
                } else {
                    self.inspector_tab.prev()
                };
                self.inspector_scroll = 0;
            }
            FocusPane::Workspace if self.nav == NavSection::Engineering => {
                self.governance_domain = if right {
                    self.governance_domain.next()
                } else {
                    self.governance_domain.prev()
                };
                self.governance_row = 0;
            }
            FocusPane::Rail if right => {
                self.focus = FocusPane::Workspace;
            }
            FocusPane::Workspace if !right && self.visibility.rail => {
                self.focus = FocusPane::Rail;
            }
            _ => {}
        }
    }

    fn jump_edge(&mut self, top: bool) {
        if self.focus == FocusPane::Inspector {
            self.inspector_scroll = if top { 0 } else { u16::MAX / 2 };
            return;
        }
        if self.nav == NavSection::Engineering {
            let count = governance_rows(
                self.governance_domain,
                &self.workspace.manifest.engineering,
                self.i18n(),
            )
            .len();
            self.governance_row = if top { 0 } else { count.saturating_sub(1) };
        } else {
            let count = self.current_resource_count();
            self.list_index = if top { 0 } else { count.saturating_sub(1) };
        }
    }

    fn cycle_focus(&mut self) {
        let mut order = Vec::with_capacity(4);
        if self.visibility.rail {
            order.push(FocusPane::Rail);
        }
        order.push(FocusPane::Workspace);
        if self.visibility.inspector {
            order.push(FocusPane::Inspector);
        }
        if self.visibility.logs_drawer {
            order.push(FocusPane::LogsDrawer);
        }
        let pos = order.iter().position(|&p| p == self.focus).unwrap_or(0);
        let next_idx = pos.saturating_add(1).checked_rem(order.len()).unwrap_or(0);
        if let Some(&next_pane) = order.get(next_idx) {
            self.focus = next_pane;
        }
    }

    fn scroll_active(&mut self, delta: i16) {
        let target = if self.focus == FocusPane::LogsDrawer {
            &mut self.logs_scroll
        } else {
            &mut self.inspector_scroll
        };
        if delta >= 0 {
            *target = target.saturating_add(delta.unsigned_abs());
        } else {
            *target = target.saturating_sub(delta.unsigned_abs());
        }
    }

    fn handle_activate(&mut self) -> Vec<StudioCommand> {
        if self.focus == FocusPane::Rail {
            self.focus = FocusPane::Workspace;
            return Vec::new();
        }
        if self.nav == NavSection::Engineering
            && governance::toggle_governance_row(
                self.governance_domain,
                self.governance_row,
                &mut self.workspace.manifest.engineering,
            )
        {
            self.workspace.recompute();
            self.push_toast(
                ToastLevel::Success,
                self.i18n()
                    .pick("Governance policy updated", "工程治理策略已更新")
                    .to_owned(),
            );
            return vec![StudioCommand::SaveManifest(Box::new(
                self.workspace.manifest.clone(),
            ))];
        }
        if resources::toggle_resource_enabled(
            self.nav,
            self.list_index,
            &mut self.workspace.manifest,
        ) {
            self.workspace.recompute();
            self.push_toast(
                ToastLevel::Success,
                self.i18n()
                    .pick("Resource toggled & recompiled", "资源状态已切换并重编译")
                    .to_owned(),
            );
            return vec![StudioCommand::SaveManifest(Box::new(
                self.workspace.manifest.clone(),
            ))];
        }
        Vec::new()
    }

    fn handle_cycle_property(&mut self) -> Vec<StudioCommand> {
        if let Some(note) = resources::cycle_resource_property(
            self.nav,
            self.list_index,
            &mut self.workspace.manifest,
        ) {
            self.workspace.recompute();
            self.push_toast(ToastLevel::Success, note.to_owned());
            return vec![StudioCommand::SaveManifest(Box::new(
                self.workspace.manifest.clone(),
            ))];
        }
        Vec::new()
    }

    fn handle_reorder_section(&mut self, up: bool) -> Vec<StudioCommand> {
        if self.nav == NavSection::Instructions
            && let Some(new_idx) = resources::reorder_instruction_section(
                self.list_index,
                self.section_index,
                up,
                &mut self.workspace.manifest,
            )
        {
            self.section_index = new_idx;
            self.workspace.recompute();
            self.push_toast(
                ToastLevel::Info,
                self.i18n()
                    .pick("Section order updated", "AGENTS.md 区块顺序已更新")
                    .to_owned(),
            );
            return vec![StudioCommand::SaveManifest(Box::new(
                self.workspace.manifest.clone(),
            ))];
        }
        Vec::new()
    }

    fn start_inline_edit(&mut self) {
        let Some((rel_path, title)) = resources::editable_source_path(
            self.nav,
            self.list_index,
            self.section_index,
            &self.workspace.manifest,
        ) else {
            self.push_toast(
                ToastLevel::Info,
                self.i18n()
                    .pick(
                        "Select a File section, Rule, Skill, or Subagent to edit inline (or press e).",
                        "请选择文件区块、规则、技能或子代理进行内置编辑（或按 e 外部编辑）。",
                    )
                    .to_owned(),
            );
            return;
        };
        let initial = self
            .workspace
            .source_files
            .get(&rel_path)
            .cloned()
            .unwrap_or_default();
        self.inline_edit = Some(InlineEditState {
            rel_path,
            title,
            editor: TextEditor::from_text(&initial),
        });
    }

    fn save_inline_edit(&mut self) -> Vec<StudioCommand> {
        let Some(ref mut edit) = self.inline_edit else {
            return Vec::new();
        };
        let content = edit.editor.text();
        let rel_path = edit.rel_path.clone();
        edit.editor.mark_clean();
        self.workspace
            .source_files
            .insert(rel_path.clone(), content.clone());
        self.workspace.recompute();
        self.push_toast(ToastLevel::Success, format!("Saved .lazyagents/{rel_path}"));
        vec![StudioCommand::SaveSourceFile { rel_path, content }]
    }

    fn update_modal_or_playground(&mut self, action: &Action) -> Vec<StudioCommand> {
        match *action {
            Action::PaletteInput(ch) => {
                self.palette.query.push(ch);
                self.palette.selected = 0;
                Vec::new()
            }
            Action::PaletteBackspace => {
                self.palette.query.pop();
                self.palette.selected = 0;
                Vec::new()
            }
            Action::PaletteConfirm => {
                let matches = self.palette.filtered(self.i18n());
                let chosen = matches
                    .get(self.palette.selected)
                    .map(|entry| entry.command.action.clone());
                self.modal = None;
                chosen.map_or_else(Vec::new, |act| self.update(act))
            }
            Action::WizardNext => {
                if self.wizard.step < 2 {
                    self.wizard.step = self.wizard.step.saturating_add(1);
                    Vec::new()
                } else {
                    self.update(Action::WizardConfirm)
                }
            }
            Action::WizardPrev => {
                self.wizard.step = self.wizard.step.saturating_sub(1);
                Vec::new()
            }
            Action::WizardToggleOption => {
                if self.wizard.option_index == 0 {
                    self.wizard.git_mode = match self.wizard.git_mode {
                        GitOutputMode::Commit => GitOutputMode::Ignore,
                        GitOutputMode::Ignore => GitOutputMode::Commit,
                    };
                } else {
                    self.wizard.import_existing = !self.wizard.import_existing;
                }
                Vec::new()
            }
            Action::WizardConfirm => {
                self.modal = None;
                vec![StudioCommand::InitializeWorkspace {
                    git_mode: self.wizard.git_mode,
                    import_existing: self.wizard.import_existing,
                }]
            }
            Action::PlaygroundToggleFocus => {
                self.playground.input_focused = !self.playground.input_focused;
                Vec::new()
            }
            Action::PlaygroundInput(ch) => {
                self.playground.input.push(ch);
                Vec::new()
            }
            Action::PlaygroundBackspace => {
                self.playground.input.pop();
                Vec::new()
            }
            Action::PlaygroundCycleGuard => {
                self.playground.cycle_guard();
                if let Some(decision) = self.playground.evaluate(&self.workspace.manifest) {
                    self.effects.trigger_verdict_flash(
                        self.geometry.workspace_area,
                        decision,
                        &self.theme,
                    );
                }
                Vec::new()
            }
            Action::PlaygroundNextSample => {
                self.playground.next_sample();
                if let Some(decision) = self.playground.evaluate(&self.workspace.manifest) {
                    self.effects.trigger_verdict_flash(
                        self.geometry.workspace_area,
                        decision,
                        &self.theme,
                    );
                }
                Vec::new()
            }
            Action::PlaygroundEvaluate => {
                if let Some(decision) = self.playground.evaluate(&self.workspace.manifest) {
                    self.effects.trigger_verdict_flash(
                        self.geometry.workspace_area,
                        decision,
                        &self.theme,
                    );
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn handle_open_external_editor(&self) -> Vec<StudioCommand> {
        let rel_path = match self.nav {
            NavSection::Rules => self
                .workspace
                .manifest
                .rules
                .get(self.list_index)
                .map(|r| format!(".lazyagents/{}", r.source)),
            NavSection::Skills => self
                .workspace
                .manifest
                .skills
                .get(self.list_index)
                .map(|s| format!(".lazyagents/{}/SKILL.md", s.source)),
            NavSection::Subagents => self
                .workspace
                .manifest
                .agents
                .get(self.list_index)
                .map(|a| format!(".lazyagents/{}", a.source)),
            _ => Some(".lazyagents/lazyagents.toml".to_owned()),
        };
        rel_path.map_or_default(|path| vec![StudioCommand::OpenExternalEditor { rel_path: path }])
    }

    fn handle_request_sync(&mut self, force: bool) -> Vec<StudioCommand> {
        if !force && self.workspace.write_plan.has_conflicts() {
            self.modal = Some(ModalKind::ConflictGuard);
            self.effects
                .trigger_modal_entrance(self.geometry.workspace_area, &self.theme);
            return Vec::new();
        }
        vec![StudioCommand::ApplySync { force }]
    }

    fn handle_copy_selected(&mut self) -> Vec<StudioCommand> {
        let cmd = if self.nav == NavSection::GitHubAudit {
            self.github_audit
                .as_ref()
                .and_then(|report| report.checks.get(self.list_index))
                .and_then(|check| check.remediation_cmd.clone())
        } else {
            None
        };
        if let Some(cmd) = cmd {
            self.push_toast(
                ToastLevel::Success,
                self.i18n()
                    .pick(
                        "Copied remediation command via OSC 52",
                        "已通过 OSC 52 复制修复命令",
                    )
                    .to_owned(),
            );
            return vec![StudioCommand::CopyToClipboard(cmd)];
        }
        Vec::new()
    }

    fn cycle_theme(&mut self) -> Vec<StudioCommand> {
        self.config.theme = self.config.theme.next();
        let is_light = matches!(self.capabilities.background, BackgroundTone::Light);
        let variant = ResolvedTheme::resolve(self.config.theme, is_light);
        self.theme = Theme::build(variant, self.config.color_profile, self.config.glyphs);
        self.push_toast(
            ToastLevel::Info,
            format!("Theme: {}", variant.display_name()),
        );
        vec![StudioCommand::SavePreferences(self.current_preferences())]
    }

    fn cycle_shader_preset(&mut self) -> Vec<StudioCommand> {
        self.config.shader_preset = self.config.shader_preset.next();
        self.effects.set_preset(self.config.shader_preset);
        self.effects
            .trigger_view_transition(self.geometry.workspace_area, &self.theme);
        self.push_toast(
            ToastLevel::Info,
            format!("Shader: {}", self.config.shader_preset),
        );
        vec![StudioCommand::SavePreferences(self.current_preferences())]
    }

    fn cycle_motion(&mut self) -> Vec<StudioCommand> {
        self.config.motion = self.config.motion.next();
        self.effects.set_motion(self.config.motion);
        self.push_toast(
            ToastLevel::Info,
            format!("Motion: {}", self.config.motion.as_str()),
        );
        vec![StudioCommand::SavePreferences(self.current_preferences())]
    }

    fn toggle_lang(&mut self) -> Vec<StudioCommand> {
        self.config.lang = self.config.lang.toggle();
        let msg = self
            .i18n()
            .pick("Language switched to English", "界面语言已切换为简体中文")
            .to_owned();
        self.push_toast(ToastLevel::Info, msg);
        vec![StudioCommand::SavePreferences(self.current_preferences())]
    }

    fn handle_mouse_click(&mut self, col: u16, row: u16) -> Vec<StudioCommand> {
        if rect_contains(self.geometry.rail_area, col, row) {
            self.focus = FocusPane::Rail;
            if row >= self.geometry.rail_items_top
                && let Some(section) =
                    rail_section_at_row(row.saturating_sub(self.geometry.rail_items_top))
            {
                self.switch_nav(section);
            }
        } else if rect_contains(self.geometry.inspector_area, col, row) {
            self.focus = FocusPane::Inspector;
        } else if rect_contains(self.geometry.logs_area, col, row) {
            self.focus = FocusPane::LogsDrawer;
        } else if rect_contains(self.geometry.workspace_area, col, row) {
            self.focus = FocusPane::Workspace;
        }
        Vec::new()
    }

    fn handle_mouse_scroll(&mut self, col: u16, row: u16, up: bool) {
        let delta = if up { -2 } else { 2 };
        if rect_contains(self.geometry.inspector_area, col, row) {
            self.focus = FocusPane::Inspector;
            self.scroll_active(delta);
        } else if rect_contains(self.geometry.logs_area, col, row) {
            self.focus = FocusPane::LogsDrawer;
            self.scroll_active(delta);
        } else if rect_contains(self.geometry.workspace_area, col, row) {
            self.focus = FocusPane::Workspace;
            self.handle_vertical_move(if up { -1 } else { 1 });
        }
    }

    fn advance_timers(&mut self, dt: Duration) {
        let dt_ms = u64::try_from(dt.as_millis()).unwrap_or(16);
        if let Some(ref mut toast) = self.toast {
            toast.remaining_ms = toast.remaining_ms.saturating_sub(dt_ms);
            if toast.remaining_ms == 0 {
                self.toast = None;
            }
        }
    }

    fn push_toast(&mut self, level: ToastLevel, message: String) {
        self.toast = Some(ToastMessage {
            level,
            message,
            remaining_ms: TOAST_TTL_MS,
        });
    }

    const fn current_preferences(&self) -> StudioPreferences {
        StudioPreferences {
            theme: self.config.theme,
            shader_preset: self.config.shader_preset,
            motion: self.config.motion,
            lang: self.config.lang,
        }
    }

    const fn current_resource_count(&self) -> usize {
        let m = &self.workspace.manifest;
        match self.nav {
            NavSection::Instructions => m.instructions.len(),
            NavSection::Rules => m.rules.len(),
            NavSection::Skills => m.skills.len(),
            NavSection::Subagents => m.agents.len(),
            NavSection::Hooks => m.hooks.len(),
            NavSection::McpServers => m.mcp_servers.len(),
            NavSection::Manifests => 3,
            NavSection::Plugins => m.plugins.len(),
            NavSection::SyncDiff => self.workspace.write_plan.files.len(),
            _ => 0,
        }
    }
}

fn shift_index(current: usize, len: usize, delta: i32) -> usize {
    if len == 0 {
        return 0;
    }
    if delta >= 0 {
        current.saturating_add(1).min(len.saturating_sub(1))
    } else {
        current.saturating_sub(1)
    }
}

const fn rect_contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

/// Maps a relative line offset inside the left navigation rail (which includes 4 group header rows)
/// back to the clicked [`NavSection`].
pub(crate) const fn rail_section_at_row(offset: u16) -> Option<NavSection> {
    match offset {
        1 => Some(NavSection::Dashboard),
        3 => Some(NavSection::Engineering),
        4 => Some(NavSection::GitHubAudit),
        6 => Some(NavSection::Instructions),
        7 => Some(NavSection::Rules),
        8 => Some(NavSection::Skills),
        9 => Some(NavSection::Subagents),
        10 => Some(NavSection::Hooks),
        11 => Some(NavSection::McpServers),
        12 => Some(NavSection::Manifests),
        13 => Some(NavSection::Plugins),
        15 => Some(NavSection::SyncDiff),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use core::assert_matches;

    use crate::config::{ShaderPreset, ThemeId, UiLang};

    use super::*;

    fn sample_state() -> AppState {
        let mut manifest = ProjectManifest::default();
        manifest.project.name = "lazyagents".to_owned();
        let ws = WorkspaceSnapshot::from_manifest(
            "/repo",
            true,
            manifest,
            ProjectFacts::default(),
            BTreeMap::new(),
            BTreeMap::new(),
            None,
        );
        AppState::new(TuiConfig::default(), TerminalCapabilities::default(), ws)
    }

    #[test]
    fn tea_state_handles_splash_skip_theme_shader_lang_and_governance_toggle() {
        let mut state = sample_state();
        assert!(state.splash_active);
        assert_eq!(state.update(Action::SkipSplash), Vec::new());
        assert!(!state.splash_active);

        let cmds = state.update(Action::CycleTheme);
        assert_eq!(state.config.theme, ThemeId::CyberNeon);
        assert_matches!(cmds.first(), Some(StudioCommand::SavePreferences(_)));

        let cmds = state.update(Action::CycleShaderPreset);
        assert_eq!(state.config.shader_preset, ShaderPreset::AuroraSweep);
        assert_matches!(cmds.first(), Some(StudioCommand::SavePreferences(_)));

        let cmds = state.update(Action::ToggleLang);
        assert_eq!(state.config.lang, UiLang::Zh);
        assert_matches!(cmds.first(), Some(StudioCommand::SavePreferences(_)));

        state.update(Action::SelectNav(NavSection::Engineering));
        let before = state.workspace.manifest.engineering.git.branching_model;
        let save_cmds = state.update(Action::ActivateSelected);
        assert_ne!(
            state.workspace.manifest.engineering.git.branching_model,
            before
        );
        assert_matches!(save_cmds.first(), Some(StudioCommand::SaveManifest(_)));
    }

    #[test]
    fn log_ring_buffer_always_redacts_stateless_github_tokens() {
        let mut ring = LogRingBuffer::default();
        let fake_token = format!("ghs_{}.{}", "a".repeat(25), "b".repeat(25));
        ring.push(LogEntry {
            timestamp: "20:00:00".to_owned(),
            level: LogLevel::Warn,
            target: "la_cli".to_owned(),
            message: format!("auth failed with {fake_token}"),
        });
        let stored = ring.iter().next().unwrap();
        assert!(!stored.message.contains(&fake_token));
        assert!(stored.message.contains("[REDACTED]"));
    }
}
