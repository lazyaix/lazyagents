//! Unidirectional TEA application state (`AppState`) for the 3-Zone Cyberpunk Command Deck.
//!
//! `AppState` holds pure domain snapshots from `la-core`, recomputes compilation, budgets,
//! diffs, and `LA001`–`LA017` diagnostics deterministically in-memory on every edit, and emits
//! [`StudioCommand`] intents without performing any filesystem or network I/O.

mod catalog_modal;
mod governance;
mod modal;
mod navigation;
mod resource_forms;
mod resources;
mod snapshot;
mod sync_audit;

use core::time::Duration;
use std::collections::BTreeMap;

use ratatui::layout::Rect;

use crate::action::{Action, StudioCommand, StudioPreferences};
use crate::config::{MotionLevel, TuiConfig};
use crate::effects::StudioEffects;
use crate::form::ModalFormState;
use crate::highlight::PreviewRenderer;
use crate::i18n::I18n;
use crate::media::PluginLogoPreview;
use crate::protocol::{BackgroundTone, TerminalCapabilities};
use crate::theme::{ResolvedTheme, Theme};

pub use self::catalog_modal::{
    CatalogCategory, CatalogEntryKind, CatalogItem, CatalogPickerState, ConfirmDeleteState,
    DeleteTarget, FilteredCatalogEntry, all_catalog_items,
};
pub use self::governance::{GovernanceDomain, GovernanceRow, governance_rows};
pub use self::modal::{
    CommandPaletteState, FilteredPaletteEntry, HookPlaygroundState, HookVerdictView,
    InitWizardState, InlineEditState, PaletteCommand, all_palette_commands,
};
pub use self::resource_forms::FormTarget;
pub use self::snapshot::{
    AuditCheckView, AuditStatusView, GitHubAuditSnapshot, LogEntry, LogLevel, LogRingBuffer,
    ToastLevel, ToastMessage, WorkspaceSnapshot,
};

/// Maximum splash display duration before auto-entering the Command Deck (540ms).
pub(super) const SPLASH_DURATION_MS: u64 = 540;

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
    /// Reusable shadowed structured form modal (with optional target extension tabs).
    FormModal,
    /// Shadowed delete confirmation modal with optional source-file purge toggle.
    ConfirmDelete,
    /// Fuzzy-searchable offline Built-in Catalog & Preset Picker (`C` key).
    CatalogPicker,
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
    /// Active reusable shadowed structured form modal state (when `modal == Some(FormModal)`).
    pub active_form: Option<ModalFormState>,
    /// Target entity modified when submitting `active_form` (if opened via resource CRUD).
    pub active_form_target: Option<FormTarget>,
    /// Active shadowed delete confirmation modal state (when `modal == Some(ConfirmDelete)`).
    pub delete_confirm: Option<ConfirmDeleteState>,
    /// Interactive offline Built-in Catalog & Preset Picker state.
    pub catalog: CatalogPickerState,
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
            active_form: None,
            active_form_target: None,
            delete_confirm: None,
            catalog: CatalogPickerState::default(),
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

    /// Returns the currently selected compiled output file (if any).
    #[must_use]
    pub fn selected_output(&self) -> Option<&la_core::GeneratedFile> {
        self.workspace
            .compiled
            .files
            .get(self.selected_output_idx)
            .or_else(|| self.workspace.compiled.files.first())
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
            Action::StartInlineEdit
            | Action::SaveInlineEdit
            | Action::CancelInlineEdit
            | Action::ToggleVimMode
            | Action::OpenExternalEditor
            | Action::EditorInput(_) => self.update_editor_action(action),
            Action::OpenDeleteConfirm
            | Action::ToggleDeletePurge
            | Action::ConfirmDelete
            | Action::OpenCatalogPicker
            | Action::CatalogInput(_)
            | Action::CatalogBackspace
            | Action::CatalogCycleCategory
            | Action::CatalogConfirm
            | Action::OpenAddResource
            | Action::OpenAddInstructionPlan
            | Action::OpenEditResource
            | Action::OpenForm(_)
            | Action::FormInput(_) => self.update_form_or_catalog(action),
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
                } else if kind == ModalKind::CatalogPicker {
                    self.catalog.reset();
                }
                self.modal = Some(kind);
                self.effects
                    .trigger_modal_entrance(self.geometry.workspace_area, &self.theme);
                Vec::new()
            }
            Action::CloseModal => {
                self.modal = None;
                self.active_form = None;
                self.active_form_target = None;
                self.delete_confirm = None;
                Vec::new()
            }
            Action::MouseClick { col, row } => {
                self.handle_mouse_click(col, row);
                Vec::new()
            }
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

    pub(super) fn push_toast(&mut self, level: ToastLevel, message: String) {
        self.toast = Some(ToastMessage {
            level,
            message,
            remaining_ms: TOAST_TTL_MS,
        });
    }

    pub(super) const fn current_preferences(&self) -> StudioPreferences {
        StudioPreferences {
            theme: self.config.theme,
            shader_preset: self.config.shader_preset,
            motion: self.config.motion,
            lang: self.config.lang,
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
}

#[cfg(test)]
mod tests {
    use core::assert_matches;

    use la_core::{ProjectFacts, ProjectManifest};

    use crate::config::{ShaderPreset, ThemeId, UiLang};

    use super::*;

    fn sample_state() -> AppState {
        let mut manifest = ProjectManifest::default();
        "lazyagents".clone_into(&mut manifest.project.name);
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
