//! Project metadata, target toggles, scope settings, and linter configuration
//! inside `.lazyagents/lazyagents.toml`.

use serde::{Deserialize, Serialize};

/// Antigravity hard per-file byte cap for `AGENTS.md` and rule files (after include expansion).
pub const DEFAULT_RULE_MAX_BYTES: usize = 24_000;

/// Antigravity aggregate token budget for always-on and global rules (`defaultRulesBudget`).
pub const DEFAULT_ALWAYS_ON_MAX_TOKENS: usize = 20_000;

/// How generated target outputs (`AGENTS.md`, `.agents/**`) interact with Git.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GitOutputMode {
    /// Commit generated files to Git so teammates need zero extra tooling, verified via CI drift check.
    #[default]
    Commit,
    /// Treat generated files as build artifacts and list them in a managed `.gitignore` block.
    Ignore,
}

/// Supported built-in UI and template language codes.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TemplateLang {
    /// English (default).
    #[default]
    En,
    /// Simplified Chinese.
    Zh,
}

/// Primary instruction filename emitted for the Antigravity target.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum InstructionFileKind {
    /// Open cross-agent standard `AGENTS.md` (default).
    #[default]
    #[serde(rename = "AGENTS.md")]
    AgentsMd,
    /// Legacy Antigravity/Gemini filename `GEMINI.md`.
    #[serde(rename = "GEMINI.md")]
    GeminiMd,
}

/// Diagnostic enforcement level for configurable lint rules.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LintLevel {
    /// Emit an error (causes non-zero exit code in `lazyagents check`).
    #[default]
    Error,
    /// Emit a warning (fails CI only under `--strict`).
    Warn,
    /// Suppress this lint rule.
    Off,
}

/// Project-level metadata declared under `[project]`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectMeta {
    /// Project or repository display name.
    pub name: String,
    /// Whether generated files are committed to Git or ignored as build artifacts.
    pub git_mode: GitOutputMode,
    /// Preferred default language for generated template sections.
    pub template_lang: TemplateLang,
}

/// Reserved remote/local organization policy pointer under `[scopes]`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "kebab-case")]
pub enum OrgPolicyRef {
    /// Local filesystem directory or manifest file (offline testing / monorepo root).
    Local {
        /// Path to the organization policy manifest.
        path: String,
    },
    /// Reserved remote Git repository reference pinned to a tag or commit SHA.
    Git {
        /// Repository clone URL.
        url: String,
        /// Optional pinned Git ref, tag, or commit SHA.
        rev: Option<String>,
    },
}

/// Scope inheritance configuration under `[scopes]` (User & Organization reserved for post-V1).
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScopeSettings {
    /// Reserved flag to inherit personal defaults from `~/.config/lazyagents/`.
    pub inherit_user: bool,
    /// Reserved pointer to an organization-level policy baseline.
    pub org_policy: Option<OrgPolicyRef>,
}

/// Active Google Antigravity target settings (`[targets.antigravity]`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AntigravityTargetConfig {
    /// Whether compilation emits Antigravity outputs (`AGENTS.md`, `.agents/**`).
    pub enabled: bool,
    /// Instruction filename emitted at the repository root and configured subdirectories.
    pub instruction_file: InstructionFileKind,
}

/// Reserved target toggle (`[targets.claude_code]`, `[targets.codex]`, `[targets.github_copilot]`).
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReservedTargetConfig {
    /// Reserved activation flag for future cross-agent compiler adapters.
    pub enabled: bool,
}

/// Per-target activation and option table (`[targets]`).
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TargetConfigs {
    /// Google Antigravity target (V1 primary target).
    pub antigravity: AntigravityTargetConfig,
    /// Reserved Anthropic Claude Code target.
    pub claude_code: ReservedTargetConfig,
    /// Reserved `OpenAI` Codex target.
    pub codex: ReservedTargetConfig,
    /// Reserved GitHub Copilot target.
    pub github_copilot: ReservedTargetConfig,
}

/// Linter threshold and severity settings (`[lint]`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LintConfig {
    /// Per-file UTF-8 byte cap for `AGENTS.md` and rule files (`LA002`).
    pub rule_max_bytes: usize,
    /// Aggregate token budget across always-on rules (`LA003`).
    pub always_on_max_tokens: usize,
    /// Severity for plaintext secret detection (`LA001`).
    pub secrets: LintLevel,
    /// Severity for legacy SSE MCP endpoints on Antigravity (`LA006`).
    pub legacy_sse_mcp: LintLevel,
    /// Severity for `SKILL.md` frontmatter and directory consistency (`LA004`).
    pub skill_frontmatter: LintLevel,
    /// Severity for GitHub Actions workflow security hygiene (`LA014`–`LA016`).
    pub github_actions_security: LintLevel,
    /// Severity for lossy cross-agent mappings (`LA012`).
    pub compatibility_loss: LintLevel,
}

impl LintLevel {
    /// Returns `true` if `self` is at least as strict as `baseline` (`Error > Warn > Off`).
    #[must_use]
    pub const fn is_at_least_as_strict_as(self, baseline: Self) -> bool {
        self.strictness_rank() >= baseline.strictness_rank()
    }

    const fn strictness_rank(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::Warn => 1,
            Self::Error => 2,
        }
    }
}

impl Default for ProjectMeta {
    fn default() -> Self {
        Self {
            name: "unnamed-project".to_owned(),
            git_mode: GitOutputMode::Commit,
            template_lang: TemplateLang::En,
        }
    }
}

impl Default for AntigravityTargetConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            instruction_file: InstructionFileKind::AgentsMd,
        }
    }
}

impl Default for LintConfig {
    fn default() -> Self {
        Self {
            rule_max_bytes: DEFAULT_RULE_MAX_BYTES,
            always_on_max_tokens: DEFAULT_ALWAYS_ON_MAX_TOKENS,
            secrets: LintLevel::Error,
            legacy_sse_mcp: LintLevel::Error,
            skill_frontmatter: LintLevel::Error,
            github_actions_security: LintLevel::Error,
            compatibility_loss: LintLevel::Warn,
        }
    }
}
