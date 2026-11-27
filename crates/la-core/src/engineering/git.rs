//! Git workflow conventions, worktree isolation, destructive-command guardrails,
//! and protected-path blast-radius policies.

use serde::{Deserialize, Serialize};

use crate::id::PolicyDecision;

/// Repository Git branching model.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BranchingModel {
    /// Single permanent `main` branch with short-lived topic branches.
    #[default]
    TrunkBased,
    /// Classic GitHub Flow centered around pull requests to the default branch.
    GithubFlow,
    /// Multi-branch release model (`main` + `develop` + release branches).
    Gitflow,
}

/// Allowed pull-request merge strategy on the default branch.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MergeStrategy {
    /// Linear history via Squash and Merge using PR title/body.
    #[default]
    SquashOnly,
    /// Standard merge commits permitted.
    Merge,
    /// Rebase-and-merge workflow.
    Rebase,
}

/// Scope at which Conventional Commits 1.0.0 is enforced.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConventionalCommitMode {
    /// Enforced on PR titles (pairs with `SquashOnly` so WIP branch commits stay frictionless).
    #[default]
    PrTitle,
    /// Enforced on every individual commit message.
    EveryCommit,
    /// Disabled.
    Off,
}

/// Policy for AI-assisted contribution disclosure in PRs or commits.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AiDisclosureMode {
    /// Include an optional AI-Assisted Disclosure checklist item in the PR template.
    #[default]
    PrCheckbox,
    /// Include a `Co-authored-by` trailer convention.
    CoAuthoredBy,
    /// Emit no AI disclosure prompt or trailer rule.
    None,
}

/// Concurrent AI agent workspace isolation configuration via `git worktree`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorktreeConfig {
    /// Whether parallel agent tasks should isolate in dedicated worktrees.
    pub enabled: bool,
    /// Naming pattern for sibling worktree paths (e.g., `../{repo}-wt-{task}`).
    pub pattern: String,
}

/// Built-in destructive-Git guardrail decisions evaluated by `lazyagents hook eval git-guard`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GitGuardrails {
    /// Decision when an agent attempts to push directly to the default branch.
    pub block_push_default_branch: PolicyDecision,
    /// Decision when an agent attempts `git push --force` or `-f`.
    pub block_force_push: PolicyDecision,
    /// Decision when an agent attempts `git reset --hard` or `git clean -fd`.
    pub block_destructive_reset: PolicyDecision,
    /// Decision when an agent runs broad staging (`git add .` / `git add -A`).
    pub guard_broad_staging: PolicyDecision,
}

/// Declarative `[engineering.git]` workflow specification.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GitPolicy {
    /// Primary protected branch name (e.g., `"main"`).
    pub default_branch: String,
    /// Branching topology followed by the repository.
    pub branching_model: BranchingModel,
    /// Merge strategy enforced on the default branch.
    pub merge_strategy: MergeStrategy,
    /// Recommended branch prefixes documented for humans and agents.
    pub branch_prefixes: Vec<String>,
    /// Conventional Commits enforcement mode.
    pub conventional_commits: ConventionalCommitMode,
    /// Whether large changes should be decomposed into stacked PRs (`Depends on #<pr>`).
    pub stacked_prs: bool,
    /// Concurrent `git worktree` isolation settings.
    pub worktree: WorktreeConfig,
    /// AI disclosure policy for pull requests and commits.
    pub ai_disclosure: AiDisclosureMode,
    /// Whether SSH commit signing is recommended in contributor guidance.
    pub recommend_ssh_signing: bool,
    /// Opt-in generation of local `.git/hooks` / `lefthook.yml` (disabled by default).
    pub local_git_hooks: bool,
    /// Deterministic `PreToolUse` guardrail policies evaluated by `lazyagents hook eval git-guard`.
    pub guardrails: GitGuardrails,
}

/// Single sensitive path glob paired with code owners and optional decision override.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProtectedPathEntry {
    /// Repository-relative glob pattern (e.g., `.github/**` or `deny.toml`).
    pub glob: String,
    /// GitHub owners (e.g., `@org/security` or `@maintainer`) synced to `CODEOWNERS`.
    #[serde(default)]
    pub owners: Vec<String>,
    /// Optional per-entry `PreToolUse` decision overriding `ProtectedPathsPolicy::default_decision`.
    #[serde(default)]
    pub decision: Option<PolicyDecision>,
}

/// Declarative `[engineering.protected_paths]` blast-radius policy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProtectedPathsPolicy {
    /// Default hook decision applied to protected paths without a specific override.
    pub default_decision: PolicyDecision,
    /// Whether to maintain a managed fenced block in `.github/CODEOWNERS`.
    pub codeowners_sync: bool,
    /// Ordered list of sensitive path entries.
    pub entries: Vec<ProtectedPathEntry>,
}

impl Default for WorktreeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            pattern: "../{repo}-wt-{task}".to_owned(),
        }
    }
}

impl Default for GitGuardrails {
    fn default() -> Self {
        Self {
            block_push_default_branch: PolicyDecision::Deny,
            block_force_push: PolicyDecision::Deny,
            block_destructive_reset: PolicyDecision::Deny,
            guard_broad_staging: PolicyDecision::Ask,
        }
    }
}

impl Default for GitPolicy {
    fn default() -> Self {
        Self {
            default_branch: "main".to_owned(),
            branching_model: BranchingModel::TrunkBased,
            merge_strategy: MergeStrategy::SquashOnly,
            branch_prefixes: vec![
                "feat/".to_owned(),
                "fix/".to_owned(),
                "refactor/".to_owned(),
                "docs/".to_owned(),
                "test/".to_owned(),
                "ci/".to_owned(),
                "chore/".to_owned(),
                "agent/".to_owned(),
            ],
            conventional_commits: ConventionalCommitMode::PrTitle,
            stacked_prs: true,
            worktree: WorktreeConfig::default(),
            ai_disclosure: AiDisclosureMode::PrCheckbox,
            recommend_ssh_signing: true,
            local_git_hooks: false,
            guardrails: GitGuardrails::default(),
        }
    }
}

impl Default for ProtectedPathsPolicy {
    fn default() -> Self {
        Self {
            default_decision: PolicyDecision::ForceAsk,
            codeowners_sync: false,
            entries: Vec::new(),
        }
    }
}
