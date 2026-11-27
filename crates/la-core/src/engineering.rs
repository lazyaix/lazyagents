//! Declarative `[engineering]` governance policy models compiled across agent and GitHub surfaces.

mod git;
mod github;
mod quality;

use serde::{Deserialize, Serialize};

pub use self::git::{
    AiDisclosureMode, BranchingModel, ConventionalCommitMode, GitGuardrails, GitPolicy,
    MergeStrategy, ProtectedPathEntry, ProtectedPathsPolicy, WorktreeConfig,
};
pub use self::github::{
    ExpectedRulesets, GitHubPolicy, GitHubScaffold, GitHubSyncOptions, TagProtectionRule,
};
pub use self::quality::{
    ArchitectureLayer, ArchitecturePolicy, ReleasePolicy, ReleaseTool, StopHookMode,
    SupplyChainPolicy, VerifyCommand, VerifyPolicy,
};

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
}
