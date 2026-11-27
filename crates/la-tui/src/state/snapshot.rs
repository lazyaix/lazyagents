//! Pure Sans-I/O workspace snapshot, live telemetry log ring buffer, toast banners,
//! and decoupled GitHub audit view models for [`crate::state::AppState`].

use std::collections::{BTreeMap, VecDeque};

use la_core::{
    CompileInputs, CompileOutput, ConfigScope, DiagnosticSeverity, LintInputs, LintReport,
    Lockfile, ProjectFacts, ProjectManifest, WritePlan, compile_all, lint_project, plan_writes,
    redact_ghs_tokens,
};

/// Maximum log entries retained in [`LogRingBuffer`].
const MAX_LOG_ENTRIES: usize = 200;

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
