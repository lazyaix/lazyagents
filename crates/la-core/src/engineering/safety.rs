//! Fast post-edit formatting (`[engineering.format]`) and non-Git shell safety guardrails
//! (`[engineering.shell_guard]`) under `[engineering]`.

use serde::{Deserialize, Serialize};

use crate::id::{PolicyDecision, ResourceId};

/// Default timeout (in milliseconds) for single-file `auto-format` `PostToolUse` hook commands.
pub const DEFAULT_FORMAT_TIMEOUT_MS: u32 = 2_000;

/// Single glob-matched fast formatter rule declared under `[engineering.format]`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormatRule {
    /// Stable rule identifier (e.g., `rustfmt`, `ruff`, `prettier`).
    pub id: ResourceId,
    /// Workspace-relative glob pattern matching edited files (e.g., `**/*.rs`).
    pub glob: String,
    /// Direct argv command array (zero shell interpolation; `{path}` expands to the edited file).
    pub cmd: Vec<String>,
    /// Execution timeout in milliseconds (must be > 0).
    #[serde(default = "default_format_timeout_ms")]
    pub timeout_ms: u32,
}

/// Declarative `[engineering.format]` policy driving the built-in `auto-format` `PostToolUse` hook.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FormatPolicy {
    /// Whether the built-in `auto-format` `PostToolUse` hook is enabled.
    pub enabled: bool,
    /// Ordered single-file formatter rules evaluated against edited paths.
    pub rules: Vec<FormatRule>,
}

/// Declarative `[engineering.shell_guard]` policy driving the built-in `shell-guard` `PreToolUse` hook.
///
/// Uses `Option<bool>` for `enabled` so lower configuration scopes omitting `[engineering.shell_guard]`
/// inherit an `Organization`-level `EnforceLock` without falsely attempting to disable it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShellGuardPolicy {
    /// Explicit enablement toggle (`None` inherits upper scopes and defaults to `false`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Decision when an agent runs destructive removals (e.g., `rm -rf /`, `rm -rf ~`, `chmod 777`).
    pub block_destructive_rm: PolicyDecision,
    /// Decision when an agent pipes remote content directly to a shell (`curl ... | sh`, `wget ... | bash`).
    pub block_pipe_to_shell: PolicyDecision,
    /// Decision when an agent attempts to read sensitive local credentials (`~/.ssh`, `~/.aws`, `~/.gnupg`, `.env`).
    pub block_credential_reads: PolicyDecision,
    /// Additional regular expressions blocked with `Deny` when matched against a shell command.
    pub extra_deny_patterns: Vec<String>,
}

impl ShellGuardPolicy {
    /// Returns `true` when `shell-guard` is explicitly enabled.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        matches!(self.enabled, Some(true))
    }
}

impl Default for ShellGuardPolicy {
    fn default() -> Self {
        Self {
            enabled: None,
            block_destructive_rm: PolicyDecision::Deny,
            block_pipe_to_shell: PolicyDecision::Deny,
            block_credential_reads: PolicyDecision::Deny,
            extra_deny_patterns: Vec::new(),
        }
    }
}

const fn default_format_timeout_ms() -> u32 {
    DEFAULT_FORMAT_TIMEOUT_MS
}
