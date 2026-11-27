//! GitHub repository scaffolding, workflow linting, collaboration presets,
//! and read-only Rulesets audit expectations.

use serde::{Deserialize, Serialize};

use crate::id::KebabName;

/// Opt-in built-in `.github/*` governance scaffold presets.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GitHubScaffold {
    /// Conventional Commits PR title validation workflow (`.github/workflows/pr-title.yml`).
    PrTitle,
    /// `zizmor` GitHub Actions supply-chain audit workflow.
    SecurityZizmor,
    /// Grouped Dependabot updates with supply-chain cooldown (`.github/dependabot.yml`).
    Dependabot,
    /// Structured YAML bug report and feature request templates (`.github/ISSUE_TEMPLATE/`).
    IssueTemplates,
    /// Standard `CONTRIBUTING.md` aligned with `[engineering.git]` and `[engineering.verify]`.
    Contributing,
    /// Standard `SECURITY.md` aligned with private advisories and supply-chain baseline.
    SecurityPolicy,
}

/// How `.github/workflows/lazyagents.yml` installs or invokes `lazyagents` in CI.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CiInstallMode {
    /// Download the prebuilt `lazyagents` binary from GitHub Releases (default for consumer repos).
    #[default]
    ReleaseBinary,
    /// Run `lazyagents` directly from the current Cargo workspace via `cargo run --locked`.
    CargoWorkspace,
}

/// Ownership mode used when synchronizing `.github/pull_request_template.md`.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrTemplateSyncMode {
    /// Manage the entire `.github/pull_request_template.md` file.
    #[default]
    WholeFile,
    /// Manage only a fenced HTML-comment block, preserving hand-authored PR template sections.
    ManagedBlock,
}

/// Core opt-in `.github/*` file synchronization toggles.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GitHubSyncOptions {
    /// Generate SHA-pinned `.github/workflows/lazyagents.yml` running `lazyagents check`.
    pub ci_workflow: bool,
    /// How the generated CI workflow installs or runs `lazyagents`.
    pub ci_install: CiInstallMode,
    /// Generate or update `.github/pull_request_template.md` linked to `[engineering.verify]`.
    pub pr_template: bool,
    /// Ownership mode for `.github/pull_request_template.md` (`whole-file` or `managed-block`).
    pub pr_template_mode: PrTemplateSyncMode,
    /// Audit `.github/workflows/*.yml` for SHA pinning, `permissions: {}`, and injection hygiene.
    pub lint_workflows: bool,
}

/// Expected Git tag protection pattern audited via `lazyagents github audit`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TagProtectionRule {
    /// Tag glob pattern expected to be immutable (e.g., `"v*"`).
    pub pattern: Option<String>,
    /// Whether GitHub Immutable Releases should be enabled on the repository.
    pub immutable_releases: bool,
}

/// Expected live GitHub Repository Rulesets audited read-only via local `gh` CLI.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExpectedRulesets {
    /// Expect an active Ruleset protecting the default branch with zero direct pushes.
    pub protect_default_branch: bool,
    /// Expect `required_linear_history` on the default branch.
    pub require_linear_history: bool,
    /// Expect a pull request before merging into the default branch.
    pub require_pr_before_merge: bool,
    /// Status check names expected to be mandatory on the default branch.
    pub required_checks: Vec<String>,
    /// Expected release tag immutability settings.
    pub release_tags: TagProtectionRule,
}

/// Declarative `[engineering.github]` integration policy.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GitHubPolicy {
    /// Enables GitHub-specific policy compilation and audits.
    pub enabled: bool,
    /// Repository slug (`owner/name`) used for links and `lazyagents github audit`.
    pub repo: Option<String>,
    /// Opt-in synchronization and linting toggles for repository `.github/*` files.
    pub sync: GitHubSyncOptions,
    /// Enables the built-in Streamable HTTP GitHub MCP preset (`variables`-only).
    pub enable_mcp_preset: bool,
    /// Built-in GitHub collaboration skills to include (e.g., `pr-author`, `ci-remediation`).
    pub enable_skills: Vec<KebabName>,
    /// Additional opt-in built-in governance scaffolds.
    pub scaffolds: Vec<GitHubScaffold>,
    /// Expected remote Rulesets verified read-only via local `gh` CLI.
    pub expected_rulesets: ExpectedRulesets,
}

impl Default for ExpectedRulesets {
    fn default() -> Self {
        Self {
            protect_default_branch: true,
            require_linear_history: true,
            require_pr_before_merge: true,
            required_checks: Vec::new(),
            release_tags: TagProtectionRule::default(),
        }
    }
}
