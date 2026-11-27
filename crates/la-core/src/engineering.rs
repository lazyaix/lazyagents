//! Declarative `[engineering]` governance policy models compiled across agent and GitHub surfaces.

mod git;
mod github;
mod quality;
mod safety;

use serde::{Deserialize, Serialize};

pub use self::git::{
    AiDisclosureMode, BranchingModel, ConventionalCommitMode, GitGuardrails, GitPolicy,
    MergeStrategy, ProtectedPathEntry, ProtectedPathsPolicy, WorktreeConfig,
};
pub use self::github::{
    CiInstallMode, ExpectedRulesets, GitHubPolicy, GitHubScaffold, GitHubSyncOptions,
    PrTemplateSyncMode, TagProtectionRule,
};
pub use self::quality::{
    ArchitectureLayer, ArchitecturePolicy, ReleasePolicy, ReleaseTool, StopHookMode,
    SupplyChainPolicy, VerifyCommand, VerifyPolicy,
};
pub use self::safety::{DEFAULT_FORMAT_TIMEOUT_MS, FormatPolicy, FormatRule, ShellGuardPolicy};

/// Top-level `[engineering]` governance specification declared once and compiled across surfaces.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineeringPolicy {
    /// Git workflow conventions, worktree isolation, and destructive-command guardrails.
    pub git: GitPolicy,
    /// Sensitive paths guarded by `PreToolUse` hooks and optional `.github/CODEOWNERS` sync.
    pub protected_paths: ProtectedPathsPolicy,
    /// Required local verification commands compiled into `AGENTS.md`, PR templates, and hooks.
    pub verify: VerifyPolicy,
    /// GitHub repository templates, Actions security linting, MCP/Skills, and read-only `gh` audit.
    pub github: GitHubPolicy,
    /// Dependency license allowlist, publish cooldown, and stateless token safety rules.
    pub supply_chain: SupplyChainPolicy,
    /// Architectural boundary layers and core design document pointers.
    pub architecture: ArchitecturePolicy,
    /// Release automation, tagging conventions, and immutable release expectations.
    pub release: ReleasePolicy,
    /// Fast glob-matched single-file formatters driving the `auto-format` `PostToolUse` hook.
    pub format: FormatPolicy,
    /// Deterministic non-Git shell guardrails driving the `shell-guard` `PreToolUse` hook.
    pub shell_guard: ShellGuardPolicy,
}
