//! Full manifest TOML roundtrip and secret-scanning black-box tests.

use la_core::engineering::{
    AiDisclosureMode, BranchingModel, ConventionalCommitMode, GitHubScaffold, MergeStrategy,
    ReleaseTool, StopHookMode,
};
use la_core::manifest::{
    GitOutputMode, HookMatcher, InstructionFileKind, InstructionSection, LintLevel, McpTransport,
    ModelTier, PackSource, RuleTrigger, TemplateLang,
};
use la_core::{
    HookEvent, PolicyDecision, ProjectManifest, SecretKind, ToolCapability, scan_secrets,
};

const FULL_MANIFEST_TOML: &str = r###"
schema_version = 1

[project]
name = "lazyagents"
git_mode = "commit"
template_lang = "en"

[scopes]
inherit_user = false

[targets.antigravity]
enabled = true
instruction_file = "AGENTS.md"

[targets.claude_code]
enabled = false

[targets.codex]
enabled = false

[targets.github_copilot]
enabled = false

[engineering.git]
default_branch = "main"
branching_model = "trunk-based"
merge_strategy = "squash-only"
branch_prefixes = ["feat/", "fix/", "docs/", "agent/"]
conventional_commits = "pr-title"
stacked_prs = true
ai_disclosure = "pr-checkbox"
recommend_ssh_signing = true
local_git_hooks = false

[engineering.git.worktree]
enabled = true
pattern = "../{repo}-wt-{task}"

[engineering.git.guardrails]
block_push_default_branch = "deny"
block_force_push = "deny"
block_destructive_reset = "deny"
guard_broad_staging = "ask"

[engineering.protected_paths]
default_decision = "force_ask"
codeowners_sync = true
entries = [
  { glob = ".lazyagents/**", owners = ["@handoffx"] },
  { glob = ".github/**", owners = ["@handoffx"], decision = "deny" },
]

[engineering.verify]
stop_hook_mode = "remind"
commands = [
  { id = "fmt", label = "Format", cmd = "cargo fmt --all -- --check", required = true },
  { id = "test", label = "Test", cmd = "cargo test --workspace --all-features", required = true },
]

[engineering.github]
enabled = true
repo = "lazyaix/lazyagents"
enable_mcp_preset = true
enable_skills = ["pr-author", "ci-remediation", "stacked-pr"]
scaffolds = ["pr-title", "security-zizmor", "dependabot"]

[engineering.github.sync]
ci_workflow = true
pr_template = true
lint_workflows = true

[engineering.github.expected_rulesets]
protect_default_branch = true
require_linear_history = true
require_pr_before_merge = true
required_checks = ["PR Title", "Format", "Clippy", "Test", "Cargo Deny", "Zizmor"]

[engineering.github.expected_rulesets.release_tags]
pattern = "v*"
immutable_releases = true

[engineering.supply_chain]
allowed_licenses = ["Apache-2.0", "MIT"]
forbid_copyleft = true
publish_cooldown_days = 7
stateless_github_tokens = true

[engineering.architecture]
docs = ["docs/architecture.md", "docs/rust-standards.md"]

[[engineering.architecture.layers]]
name = "la-core"
glob = "crates/la-core/**"
rule = "Pure Sans-I/O domain crate."

[engineering.release]
tool = "release-plz"
tag_pattern = "v{{ version }}"
immutable_releases = true

[[packs]]
id = "rust-stack"
source = { kind = "builtin", name = "lang/rust" }

[[packs]]
id = "team-local"
source = { kind = "local", path = "../shared-pack" }
include_only = ["skills/pr-review"]

[[instructions]]
output = "AGENTS.md"
auto_probe = true
sections = [
  { preset = "builtin:engineering/git-workflow" },
  { file = "sections/custom.md", when = "project.has_rust" },
  { inline = "## Notes\nKeep diffs minimal." },
]

[[rules]]
id = "rust-safety"
source = "rules/rust-style.md"
trigger = "glob"
globs = ["crates/**/*.rs"]
description = "Rust safety conventions."

[[skills]]
name = "release-prep"
source = "skills/release-prep"

[manifests.skills]
entries = [{ path = "tools/extra-skills", exclude = ["wip-*"] }]

[[agents]]
name = "security-auditor"
source = "agents/security-auditor.md"
description = "Read-only auditor."
model = "pro"
capabilities = ["file_read", "search", "web_search"]
skills = ["release-prep"]
mcp_servers = ["github"]

[agents.targets.antigravity]
main_agent = false
subagent = true
permission_mode = "plan"
command_execution_policy = "sandbox"

[[hooks]]
name = "git-guard"
enabled = true
event = "pre_tool_use"
matcher = { capability = "shell" }
command = "lazyagents hook eval git-guard"
timeout_secs = 15

[[mcp_servers]]
name = "github"
transport = "streamable_http"
url = "https://api.githubcopilot.com/mcp/"

[mcp_servers.headers]
Authorization = "Bearer ${GITHUB_MCP_TOKEN}"

[[mcp_servers.variables]]
name = "GITHUB_MCP_TOKEN"
title = "GitHub MCP Token"
description = "Token for GitHub MCP access"

[[mcp_servers]]
name = "sqlite-local"
transport = "stdio"
command = "sqlite-mcp-server"
args = ["./dev.db"]

[[plugins]]
name = "repo-toolkit"
display_name = "Repo Toolkit"
version = "1.0.0"
description = "Bundled repository toolkit."
include_rules = ["rust-safety"]
include_skills = ["release-prep"]
include_agents = ["security-auditor"]
include_hooks = ["git-guard"]
include_mcp = ["github"]

[lint]
rule_max_bytes = 24000
always_on_max_tokens = 20000
secrets = "error"
legacy_sse_mcp = "error"
skill_frontmatter = "error"
github_actions_security = "error"
compatibility_loss = "warn"
"###;

#[test]
fn full_manifest_parses_roundtrips_and_passes_secret_scan() {
    assert_eq!(scan_secrets(FULL_MANIFEST_TOML), Vec::new());

    let parsed = ProjectManifest::from_toml_str(FULL_MANIFEST_TOML).unwrap();
    assert_eq!(parsed.project.name, "lazyagents");
    assert_eq!(parsed.project.git_mode, GitOutputMode::Commit);
    assert_eq!(parsed.project.template_lang, TemplateLang::En);
    assert_eq!(
        parsed.targets.antigravity.instruction_file,
        InstructionFileKind::AgentsMd
    );

    assert_eq!(
        parsed.engineering.git.branching_model,
        BranchingModel::TrunkBased
    );
    assert_eq!(
        parsed.engineering.git.merge_strategy,
        MergeStrategy::SquashOnly
    );
    assert_eq!(
        parsed.engineering.git.conventional_commits,
        ConventionalCommitMode::PrTitle
    );
    assert_eq!(
        parsed.engineering.git.ai_disclosure,
        AiDisclosureMode::PrCheckbox
    );
    assert_eq!(
        parsed.engineering.protected_paths.default_decision,
        PolicyDecision::ForceAsk
    );
    assert_eq!(
        parsed.engineering.verify.stop_hook_mode,
        StopHookMode::Remind
    );
    assert_eq!(parsed.engineering.release.tool, ReleaseTool::ReleasePlz);
    assert_eq!(
        parsed.engineering.github.scaffolds,
        vec![
            GitHubScaffold::PrTitle,
            GitHubScaffold::SecurityZizmor,
            GitHubScaffold::Dependabot,
        ]
    );

    assert!(matches!(
        &parsed.packs[0].source,
        PackSource::Builtin { name } if name == "lang/rust"
    ));
    assert!(matches!(
        &parsed.instructions[0].sections[0],
        InstructionSection::Preset { preset, .. } if preset == "builtin:engineering/git-workflow"
    ));
    assert_eq!(parsed.rules[0].trigger, RuleTrigger::Glob);
    assert_eq!(parsed.agents[0].model, ModelTier::Pro);
    assert_eq!(
        parsed.agents[0].capabilities,
        vec![
            ToolCapability::FileRead,
            ToolCapability::Search,
            ToolCapability::WebSearch,
        ]
    );
    assert_eq!(parsed.hooks[0].event, HookEvent::PreToolUse);
    assert_eq!(
        parsed.hooks[0].matcher,
        Some(HookMatcher::Capability {
            capability: ToolCapability::Shell,
        })
    );
    assert!(matches!(
        &parsed.mcp_servers[0].transport,
        McpTransport::StreamableHttp { url, .. } if url == "https://api.githubcopilot.com/mcp/"
    ));
    assert_eq!(parsed.lint.compatibility_loss, LintLevel::Warn);

    let reserialized = parsed.to_toml_pretty().unwrap();
    let reparsed = ProjectManifest::from_toml_str(&reserialized).unwrap();
    assert_eq!(parsed, reparsed);
}

#[test]
fn secret_scanner_flags_leaked_token_inside_manifest_header() {
    let leaked_token = format!("ghs_{}.{}", "X".repeat(25), "Y".repeat(25));
    let polluted = FULL_MANIFEST_TOML.replace("${GITHUB_MCP_TOKEN}", &leaked_token);
    let findings = scan_secrets(&polluted);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].kind, SecretKind::GitHubStatelessToken);
}
