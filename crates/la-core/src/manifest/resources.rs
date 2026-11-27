//! Canonical IR resource definitions for packs, dynamic `AGENTS.md` instructions,
//! conditional rules, skills, subagents, hooks, MCP servers, index manifests, and plugins.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::id::{HookEvent, KebabName, ResourceId, ToolCapability};

/// Default execution timeout (in seconds) for lifecycle hook handlers.
pub const DEFAULT_HOOK_TIMEOUT_SECS: u32 = 30;

/// Origin of a reusable pack (`[[packs]]`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PackSource {
    /// Curated pack embedded in the `lazyagents` binary (e.g., `lang/rust`, `github/core`).
    Builtin {
        /// Built-in catalog name.
        name: String,
    },
    /// Local filesystem directory pack (offline-safe).
    Local {
        /// Workspace-relative or absolute directory path.
        path: String,
    },
    /// Reserved remote Git repository pack pinned to a tag or commit SHA.
    Git {
        /// Repository URL.
        url: String,
        /// Optional pinned tag, branch, or commit SHA.
        rev: Option<String>,
    },
    /// Reserved organization/community registry pack.
    Registry {
        /// Registry package reference.
        package: String,
        /// Optional version requirement.
        version: Option<String>,
    },
}

/// Reusable pack declaration (`[[packs]]`) with optional allow/deny filtering.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PackRef {
    /// Unique identifier for this pack reference within the manifest.
    pub id: ResourceId,
    /// Pack location or built-in catalog reference.
    pub source: PackSource,
    /// Optional allowlist of resource paths within the pack.
    #[serde(default)]
    pub include_only: Vec<String>,
    /// Optional denylist of resource patterns excluded from the pack.
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// One modular content block inside a dynamic `AGENTS.md` composition plan.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum InstructionSection {
    /// Built-in or pack preset fragment (e.g., `builtin:engineering/git-workflow`).
    Preset {
        /// Preset identifier.
        preset: String,
        /// Optional template predicate determining when this section renders.
        #[serde(default)]
        when: Option<String>,
    },
    /// Project-local Markdown fragment relative to `.lazyagents/` (e.g., `sections/arch.md`).
    File {
        /// Relative path to the source Markdown fragment.
        file: String,
        /// Optional template predicate determining when this section renders.
        #[serde(default)]
        when: Option<String>,
    },
    /// Short inline Markdown snippet stored directly in the manifest.
    Inline {
        /// Inline Markdown body.
        inline: String,
        /// Optional template predicate determining when this section renders.
        #[serde(default)]
        when: Option<String>,
    },
}

/// Dynamic instruction file composition spec (`[[instructions]]`), supporting root and
/// Monorepo nested subdirectory `AGENTS.md` outputs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InstructionSpec {
    /// Workspace-relative output path (e.g., `AGENTS.md` or `crates/la-tui/AGENTS.md`).
    pub output: String,
    /// Populate `{{ project.* }}` facts from repository introspection when rendering.
    #[serde(default = "default_true")]
    pub auto_probe: bool,
    /// Ordered sections assembled into the compiled output.
    #[serde(default)]
    pub sections: Vec<InstructionSection>,
}

/// Activation mode for conditional modular rules (`.agents/rules/*.md`).
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleTrigger {
    /// Always injected into the active context window.
    #[default]
    AlwaysOn,
    /// Injected when viewed or edited files match any declared glob pattern.
    Glob,
    /// Activated dynamically by the model based on the rule's `description`.
    ModelDecision,
    /// Activated only when explicitly referenced by the user.
    Manual,
}

/// Conditional modular rule definition (`[[rules]]`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuleSpec {
    /// Unique rule identifier (determines `.agents/rules/<id>.md`).
    pub id: ResourceId,
    /// Source Markdown file path relative to `.lazyagents/`.
    pub source: String,
    /// Activation trigger semantics.
    #[serde(default)]
    pub trigger: RuleTrigger,
    /// Glob patterns used when `trigger = "glob"`.
    #[serde(default)]
    pub globs: Vec<String>,
    /// Short description used for `model_decision` and TUI listings.
    #[serde(default)]
    pub description: Option<String>,
    /// Whether this rule is enabled for compilation.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Skill bundle declaration (`[[skills]]`), compiled to `.agents/skills/<name>/SKILL.md`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SkillSpec {
    /// Validated lowercase kebab-case skill name (must match folder & `SKILL.md` frontmatter).
    pub name: KebabName,
    /// Source directory path relative to `.lazyagents/` (e.g., `skills/release-prep`).
    pub source: String,
    /// Whether this skill is enabled for compilation.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Model tier requested for a custom subagent.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelTier {
    /// Inherit the parent agent's model.
    #[default]
    Inherit,
    /// Lightest, fastest model tier.
    FlashLite,
    /// Fast general-purpose model tier.
    Flash,
    /// Highest-capability reasoning model tier.
    Pro,
}

/// Antigravity-specific subagent extension fields (`[agents.targets.antigravity]`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AntigravityAgentExt {
    /// Allow launching this persona directly as a primary session (`agy --agent <name>`).
    pub main_agent: bool,
    /// Allow parent agents to delegate to this persona via `invoke_subagent`.
    pub subagent: bool,
    /// Optional Antigravity `permissionMode` (e.g., `"plan"`).
    pub permission_mode: Option<String>,
    /// Optional Antigravity `commandExecutionPolicy` (e.g., `"sandbox"`, `"auto"`).
    pub command_execution_policy: Option<String>,
}

/// Per-target extension hooks for a custom subagent (`[agents.targets]`).
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SubagentTargetExt {
    /// Antigravity-specific subagent fields.
    pub antigravity: AntigravityAgentExt,
}

/// Custom subagent definition (`[[agents]]`), compiled to `.agents/agents/<name>.md`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SubagentSpec {
    /// Validated kebab-case agent identifier.
    pub name: KebabName,
    /// Source Markdown prompt body relative to `.lazyagents/`.
    pub source: String,
    /// Delegation description read by coordinator agents.
    pub description: String,
    /// Requested model tier.
    #[serde(default)]
    pub model: ModelTier,
    /// Abstract tool capabilities mapped to target-specific tool names at compile time.
    #[serde(default)]
    pub capabilities: Vec<ToolCapability>,
    /// Skills explicitly attached to this subagent.
    #[serde(default)]
    pub skills: Vec<KebabName>,
    /// MCP server names bound to this subagent.
    #[serde(default)]
    pub mcp_servers: Vec<ResourceId>,
    /// Whether this subagent is enabled for compilation.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Target-specific extension blocks.
    #[serde(default)]
    pub targets: SubagentTargetExt,
}

/// Tool matcher for `PreToolUse` and `PostToolUse` lifecycle hooks.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HookMatcher {
    /// Match by vendor-neutral abstract capability (mapped per target).
    Capability {
        /// Abstract capability mapped to target tool names (e.g., `shell` -> `run_command`).
        capability: ToolCapability,
    },
    /// Match using an explicit target-native regex (e.g., `"run_command|edit_file"` or `"*"`).
    Regex {
        /// Regular expression pattern.
        regex: String,
    },
}

/// Lifecycle hook specification (`[[hooks]]`), compiled to `.agents/hooks.json`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HookSpec {
    /// Unique named hook key in `hooks.json`.
    pub name: ResourceId,
    /// Whether this hook is active (`enabled: false` emits disabled hook entry).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Lifecycle event triggering this hook.
    pub event: HookEvent,
    /// Optional matcher required for tool-scoped events (`PreToolUse` / `PostToolUse`).
    #[serde(default)]
    pub matcher: Option<HookMatcher>,
    /// Command executed by the hook handler (e.g., `lazyagents hook eval git-guard` or `./hooks/check.sh`).
    pub command: String,
    /// Execution timeout in seconds (must be > 0).
    #[serde(default = "default_hook_timeout")]
    pub timeout_secs: u32,
}

/// User-prompted variable declaration for Antigravity MCP server configs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct McpVariable {
    /// Environment/placeholder name referenced as `${NAME}`.
    pub name: String,
    /// Human-readable form title shown in Antigravity setup UI.
    pub title: String,
    /// Concise guidance describing what credential or value to supply.
    pub description: String,
}

/// Transport mechanism and connection parameters for an MCP server.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum McpTransport {
    /// Local command spawned over stdio.
    Stdio {
        /// Executable name or path.
        command: String,
        /// Command-line arguments.
        #[serde(default)]
        args: Vec<String>,
        /// Environment variables injected into the server process (never plaintext secrets).
        #[serde(default)]
        env: BTreeMap<String, String>,
        /// Optional working directory.
        #[serde(default)]
        cwd: Option<String>,
    },
    /// Remote Streamable HTTP endpoint (MCP spec 2025-03-26+).
    StreamableHttp {
        /// Remote HTTPS endpoint URL (e.g., `https://api.githubcopilot.com/mcp/`).
        url: String,
        /// Optional HTTP headers (must use `${VAR}` placeholders for credentials).
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
    /// Legacy HTTP+SSE transport (retained in IR so Linter `LA006` catches & diagnoses it explicitly).
    LegacySse {
        /// Legacy SSE endpoint URL.
        url: String,
        /// Optional HTTP headers.
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
}

/// MCP server definition (`[[mcp_servers]]`), compiled to `.agents/mcp_config.json`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct McpServerSpec {
    /// Unique MCP server identifier.
    pub name: ResourceId,
    /// Whether this server is enabled (`disabled: true` when false).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Transport configuration (`stdio`, `streamable_http`, or `legacy_sse`).
    #[serde(flatten)]
    pub transport: McpTransport,
    /// Explicit Antigravity `${VAR}` setup variable declarations.
    #[serde(default)]
    pub variables: Vec<McpVariable>,
}

/// Single `entries` or `inherits` path entry inside Antigravity `skills.json` / `rules.json` / `plugins.json`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct IndexEntry {
    /// Workspace-relative, home-relative, or absolute path.
    pub path: String,
    /// Optional allowlist patterns relative to `path`.
    #[serde(default)]
    pub include_only: Vec<String>,
    /// Optional exclusion patterns.
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// Declarative Antigravity JSON index manifest (`skills.json`, `rules.json`, or `plugins.json`).
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IndexManifestSpec {
    /// Inherited parent JSON manifests processed in order.
    pub inherits: Vec<IndexEntry>,
    /// Explicit directory scan entries.
    pub entries: Vec<IndexEntry>,
}

/// Grouped declarative Antigravity JSON index manifests (`[manifests]`).
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IndexManifests {
    /// Optional `.agents/skills.json` index configuration.
    pub skills: IndexManifestSpec,
    /// Optional `.agents/rules.json` index configuration.
    pub rules: IndexManifestSpec,
    /// Optional `.agents/plugins.json` index configuration.
    pub plugins: IndexManifestSpec,
}

/// Cohesive project-level Antigravity plugin bundle (`[[plugins]]`), compiled to `.agents/plugins/<name>/`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PluginSpec {
    /// Unique lowercase kebab-case plugin name (and directory name).
    pub name: KebabName,
    /// Human-readable presentation title shown in UI cards.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Semantic version string (e.g., `"1.0.0"`).
    #[serde(default)]
    pub version: Option<String>,
    /// Optional author or team name emitted into `plugin.json`.
    #[serde(default)]
    pub author: Option<String>,
    /// Optional external `https://` documentation or homepage URL.
    #[serde(default)]
    pub homepage: Option<String>,
    /// Optional source repository `https://` URL.
    #[serde(default)]
    pub repository: Option<String>,
    /// Concise summary of the bundle's purpose.
    pub description: String,
    /// Optional plugin-relative logo path (`.png`, `.svg`, `.jpg`, `.jpeg`, `.webp`).
    #[serde(default)]
    pub logo: Option<String>,
    /// Optional starter prompts (up to 3 used by Antigravity UI cards).
    #[serde(default)]
    pub suggested_prompts: Vec<String>,
    /// Whether the plugin ships enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Rule IDs bundled into `<plugin>/rules/AGENTS.md`.
    #[serde(default)]
    pub include_rules: Vec<ResourceId>,
    /// Skill names bundled into `<plugin>/skills/`.
    #[serde(default)]
    pub include_skills: Vec<KebabName>,
    /// Subagent names bundled into `<plugin>/agents/`.
    #[serde(default)]
    pub include_agents: Vec<KebabName>,
    /// Hook names bundled into `<plugin>/hooks.json`.
    #[serde(default)]
    pub include_hooks: Vec<ResourceId>,
    /// MCP server names bundled into `<plugin>/mcp_config.json`.
    #[serde(default)]
    pub include_mcp: Vec<ResourceId>,
}

impl Default for AntigravityAgentExt {
    fn default() -> Self {
        Self {
            main_agent: false,
            subagent: true,
            permission_mode: None,
            command_execution_policy: None,
        }
    }
}

impl IndexManifestSpec {
    /// Returns `true` when neither `inherits` nor `entries` contains any entries.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.inherits.is_empty() && self.entries.is_empty()
    }
}

const fn default_true() -> bool {
    true
}

const fn default_hook_timeout() -> u32 {
    DEFAULT_HOOK_TIMEOUT_SECS
}
