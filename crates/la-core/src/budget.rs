//! Double-tier context budget telemetry computing:
//! 1. Platform hard caps (per-file UTF-8 byte cap after `@[label](path)` include expansion and
//!    aggregate always-on token cap),
//! 2. Empirical lean efficiency targets grounded in 2026 context-engineering studies, and
//! 3. Push vs. Pull progressive-disclosure ratios and Eager MCP tool-schema context tax.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::compiler::GeneratedFile;
use crate::manifest::{
    DEFAULT_ALWAYS_ON_MAX_TOKENS, DEFAULT_RECOMMENDED_ALWAYS_ON_TOKENS,
    DEFAULT_RECOMMENDED_ROOT_BYTES, McpServerSpec, McpToolLoading, ProjectManifest,
};

/// Warning threshold percentage (80%) for Antigravity's aggregate always-on token budget.
pub const BUDGET_WARN_PERCENT: usize = 80;

/// Conservative deterministic startup token estimate for one unbounded eager MCP server.
pub const DEFAULT_EAGER_MCP_TOKENS_PER_SERVER: usize = 1_500;

/// Conservative deterministic token estimate per explicitly allowlisted eager MCP tool.
const EAGER_MCP_TOKENS_PER_ALLOWED_TOOL: usize = 120;

/// Minimum startup token floor for an eager MCP server even with a tight tool allowlist.
const MIN_EAGER_MCP_ALLOWLIST_TOKENS: usize = 200;

/// Maximum recursive expansion depth for `@[label](path)` includes.
const MAX_INCLUDE_DEPTH: usize = 8;

/// Budget status classification for a file or aggregate rule set.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetStatus {
    /// Safely below warning threshold.
    #[default]
    Ok,
    /// Approaching the cap (>= 80% of budget).
    Warning,
    /// Exceeded the hard limit (> 100% of budget).
    Exceeded,
}

/// Budget metrics for one instruction or rule file (`AGENTS.md`, `GEMINI.md`, `.agents/rules/*.md`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileBudget {
    /// Workspace-relative POSIX path.
    pub path: String,
    /// Raw UTF-8 byte length before expanding `@[label](path)` includes.
    pub raw_bytes: usize,
    /// Expanded UTF-8 byte length after resolving `@[label](path)` includes.
    pub expanded_bytes: usize,
    /// Configured per-file byte cap (Antigravity default: 24,000 bytes).
    pub max_bytes: usize,
    /// Deterministic estimated token count for this file.
    pub estimated_tokens: usize,
    /// Whether this file counts toward Antigravity's always-on aggregate rule budget.
    pub counts_as_always_on: bool,
    /// Status against `max_bytes`.
    pub status: BudgetStatus,
}

/// Push (always-on startup cost) vs. Pull (on-demand progressive disclosure) telemetry.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PushPullMetrics {
    /// Always-on instruction and `always_on` rule tokens injected on every turn.
    pub always_on_tokens: usize,
    /// Tokens offloaded into conditional (`glob`, `model_decision`, `manual`) rules.
    pub conditional_rule_tokens: usize,
    /// Always-on startup discovery footprint of enabled `SKILL.md` (`name` + `description`).
    pub skill_discovery_tokens: usize,
    /// On-demand Pull tokens residing in full `SKILL.md` bodies.
    pub skill_body_tokens: usize,
    /// Number of enabled MCP servers loading schemas eagerly at startup.
    pub eager_mcp_server_count: usize,
    /// Number of enabled MCP servers using `deferred` Tool Search or `code_mode`.
    pub deferred_mcp_server_count: usize,
    /// Estimated startup token tax contributed by eager MCP server tool schemas.
    pub eager_mcp_estimated_tokens: usize,
    /// Percentage (`0..=100`) of total knowledge tokens kept off the always-on hot path.
    pub pull_ratio_percent: u8,
}

/// Empirical lean-context advisory telemetry grounded in `Evaluating AGENTS.md` (`LA018`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LeanAdvisory {
    /// Configured empirical root `AGENTS.md` byte target (default: 8,192 bytes).
    pub recommended_root_bytes: usize,
    /// Configured empirical always-on token target (default: 2,000 tokens).
    pub recommended_always_on_tokens: usize,
    /// Whether any root instruction file (`AGENTS.md` / `GEMINI.md`) exceeded the lean byte target.
    pub root_bytes_exceeded: bool,
    /// Whether aggregate always-on tokens exceeded the lean token target.
    pub always_on_exceeded: bool,
}

/// Complete context budget report consumed by `LA002`/`LA003`/`LA018`, CLI `inspect`, and the TUI Radar.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BudgetReport {
    /// Per-file metrics ordered by path.
    pub files: Vec<FileBudget>,
    /// Total estimated tokens across all `always_on` instruction and rule files.
    pub always_on_tokens: usize,
    /// Configured aggregate always-on token budget (Antigravity default: 20,000 tokens).
    pub max_always_on_tokens: usize,
    /// Aggregate budget status (`Ok`, `Warning` >= 80%, `Exceeded` > 100%).
    pub always_on_status: BudgetStatus,
    /// Push vs. Pull progressive-disclosure and Eager MCP schema-tax breakdown.
    pub push_pull: PushPullMetrics,
    /// Empirical lean-context advisory summary.
    pub lean_advisory: LeanAdvisory,
    /// Actionable offload hints when files approach or exceed budgets.
    pub offload_suggestions: Vec<String>,
}

impl Default for LeanAdvisory {
    fn default() -> Self {
        Self {
            recommended_root_bytes: DEFAULT_RECOMMENDED_ROOT_BYTES,
            recommended_always_on_tokens: DEFAULT_RECOMMENDED_ALWAYS_ON_TOKENS,
            root_bytes_exceeded: false,
            always_on_exceeded: false,
        }
    }
}

impl Default for BudgetReport {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            always_on_tokens: 0,
            max_always_on_tokens: DEFAULT_ALWAYS_ON_MAX_TOKENS,
            always_on_status: BudgetStatus::Ok,
            push_pull: PushPullMetrics {
                pull_ratio_percent: 100,
                ..PushPullMetrics::default()
            },
            lean_advisory: LeanAdvisory::default(),
            offload_suggestions: Vec::new(),
        }
    }
}

/// Analyzes Antigravity context budgets across compiled output files and available workspace files
/// using default empirical thresholds and zero MCP servers.
#[must_use]
pub fn analyze_budgets(
    generated: &[GeneratedFile],
    workspace_files: &BTreeMap<String, String>,
    rule_max_bytes: usize,
    always_on_max_tokens: usize,
) -> BudgetReport {
    analyze_budgets_full(
        generated,
        workspace_files,
        rule_max_bytes,
        always_on_max_tokens,
        DEFAULT_RECOMMENDED_ROOT_BYTES,
        DEFAULT_RECOMMENDED_ALWAYS_ON_TOKENS,
        &[],
    )
}

/// Analyzes full double-tier context budgets, Push vs. Pull metrics, and Eager MCP schema tax
/// using thresholds and server declarations from [`ProjectManifest`].
#[must_use]
pub fn analyze_budgets_with_manifest(
    generated: &[GeneratedFile],
    workspace_files: &BTreeMap<String, String>,
    manifest: &ProjectManifest,
) -> BudgetReport {
    analyze_budgets_full(
        generated,
        workspace_files,
        manifest.lint.rule_max_bytes,
        manifest.lint.always_on_max_tokens,
        manifest.lint.recommended_root_bytes,
        manifest.lint.recommended_always_on_tokens,
        &manifest.mcp_servers,
    )
}

fn analyze_budgets_full(
    generated: &[GeneratedFile],
    workspace_files: &BTreeMap<String, String>,
    rule_max_bytes: usize,
    always_on_max_tokens: usize,
    recommended_root_bytes: usize,
    recommended_always_on_tokens: usize,
    mcp_servers: &[McpServerSpec],
) -> BudgetReport {
    let mut all_files = workspace_files.clone();
    for file in generated {
        all_files.insert(file.path.clone(), file.content.clone());
    }

    let mut file_budgets = Vec::new();
    let mut always_on_tokens = 0usize;
    let mut conditional_rule_tokens = 0usize;
    let mut skill_discovery_tokens = 0usize;
    let mut skill_body_tokens = 0usize;
    let mut root_bytes_exceeded = false;
    let mut offload_suggestions = Vec::new();

    for file in generated {
        if is_skill_md_path(&file.path) {
            let (disc, body) = estimate_skill_tokens(&file.content);
            skill_discovery_tokens = skill_discovery_tokens.saturating_add(disc);
            skill_body_tokens = skill_body_tokens.saturating_add(body);
            continue;
        }
        if !is_rule_or_instruction_path(&file.path) {
            continue;
        }

        let mut visited = BTreeSet::new();
        visited.insert(file.path.clone());
        let expanded = expand_at_includes(&file.content, &all_files, &mut visited, 0);
        let raw_bytes = file.content.len();
        let expanded_bytes = expanded.len();
        let estimated_tokens = estimate_tokens(&expanded);
        let counts_as_always_on = is_always_on_rule(&file.path, &file.content);
        let status = classify_usage(expanded_bytes, rule_max_bytes);

        if counts_as_always_on {
            always_on_tokens = always_on_tokens.saturating_add(estimated_tokens);
        } else {
            conditional_rule_tokens = conditional_rule_tokens.saturating_add(estimated_tokens);
        }

        if is_root_instruction_path(&file.path) && expanded_bytes > recommended_root_bytes {
            root_bytes_exceeded = true;
            offload_suggestions.push(format!(
                "`{}` is {expanded_bytes} bytes (recommended <= {recommended_root_bytes} bytes) — trim generic overviews and offload domain procedures into `glob` rules or `SKILL.md`",
                file.path
            ));
        }

        if status != BudgetStatus::Ok {
            offload_suggestions.push(format!(
                "`{}` uses {expanded_bytes}/{rule_max_bytes} bytes — move specialized guidance into a `glob`-triggered `.agents/rules/*.md` or on-demand Skill",
                file.path
            ));
        }

        file_budgets.push(FileBudget {
            path: file.path.clone(),
            raw_bytes,
            expanded_bytes,
            max_bytes: rule_max_bytes,
            estimated_tokens,
            counts_as_always_on,
            status,
        });
    }

    let (eager_mcp_server_count, deferred_mcp_server_count, eager_mcp_estimated_tokens) =
        estimate_mcp_context_tax(mcp_servers);

    if eager_mcp_server_count >= 4 {
        offload_suggestions.push(format!(
            "{eager_mcp_server_count} eager MCP servers add ~{eager_mcp_estimated_tokens} startup schema tokens — mark secondary servers `tool_loading = \"deferred\"` or `\"code_mode\"`"
        ));
    }

    let always_on_status = classify_usage(always_on_tokens, always_on_max_tokens);
    if always_on_status != BudgetStatus::Ok {
        offload_suggestions.push(format!(
            "Always-on rules total ~{always_on_tokens}/{always_on_max_tokens} tokens — demote non-global rules to `glob` or `model_decision` triggers"
        ));
    }

    let always_on_exceeded = always_on_tokens > recommended_always_on_tokens;
    let push_total = always_on_tokens
        .saturating_add(skill_discovery_tokens)
        .saturating_add(eager_mcp_estimated_tokens);
    let pull_total = conditional_rule_tokens.saturating_add(skill_body_tokens);
    let pull_ratio_percent = compute_pull_ratio_percent(push_total, pull_total);

    BudgetReport {
        files: file_budgets,
        always_on_tokens,
        max_always_on_tokens: always_on_max_tokens,
        always_on_status,
        push_pull: PushPullMetrics {
            always_on_tokens,
            conditional_rule_tokens,
            skill_discovery_tokens,
            skill_body_tokens,
            eager_mcp_server_count,
            deferred_mcp_server_count,
            eager_mcp_estimated_tokens,
            pull_ratio_percent,
        },
        lean_advisory: LeanAdvisory {
            recommended_root_bytes,
            recommended_always_on_tokens,
            root_bytes_exceeded,
            always_on_exceeded,
        },
        offload_suggestions,
    }
}

/// Computes a conservative, CJK-safe deterministic token estimate for Markdown rule content.
///
/// Counts non-ASCII scalar values (e.g., CJK ideographs) at ~1.5 tokens each and ASCII bytes
/// at ~0.25 tokens each (ceiling rounded), preventing Chinese/Japanese/Korean rules from
/// under-reporting context pressure.
#[must_use]
pub fn estimate_tokens(text: &str) -> usize {
    let mut ascii_bytes = 0usize;
    let mut non_ascii_chars = 0usize;
    for ch in text.chars() {
        if ch.is_ascii() {
            ascii_bytes = ascii_bytes.saturating_add(1);
        } else {
            non_ascii_chars = non_ascii_chars.saturating_add(1);
        }
    }
    let ascii_tokens = ascii_bytes.saturating_add(3) / 4;
    let non_ascii_tokens = non_ascii_chars.saturating_mul(3).saturating_add(1) / 2;
    ascii_tokens.saturating_add(non_ascii_tokens)
}

fn estimate_mcp_context_tax(servers: &[McpServerSpec]) -> (usize, usize, usize) {
    let mut eager_count = 0usize;
    let mut deferred_count = 0usize;
    let mut eager_tokens = 0usize;

    for server in servers {
        if !server.enabled {
            continue;
        }
        match server.tool_loading {
            McpToolLoading::Eager => {
                eager_count = eager_count.saturating_add(1);
                let per_server = if server.allowed_tools.is_empty() {
                    DEFAULT_EAGER_MCP_TOKENS_PER_SERVER
                } else {
                    server
                        .allowed_tools
                        .len()
                        .saturating_mul(EAGER_MCP_TOKENS_PER_ALLOWED_TOOL)
                        .max(MIN_EAGER_MCP_ALLOWLIST_TOKENS)
                };
                eager_tokens = eager_tokens.saturating_add(per_server);
            }
            McpToolLoading::Deferred | McpToolLoading::CodeMode => {
                deferred_count = deferred_count.saturating_add(1);
            }
        }
    }

    (eager_count, deferred_count, eager_tokens)
}

fn estimate_skill_tokens(content: &str) -> (usize, usize) {
    if let Some(rest) = content.strip_prefix("---\n")
        && let Some(end) = rest.find("\n---")
    {
        let fm = rest.get(..end).unwrap_or_default();
        let body = rest.get(end.saturating_add(4)..).unwrap_or_default();
        return (estimate_tokens(fm), estimate_tokens(body));
    }
    (0, estimate_tokens(content))
}

fn compute_pull_ratio_percent(push_total: usize, pull_total: usize) -> u8 {
    let total = push_total.saturating_add(pull_total);
    let pct = pull_total
        .saturating_mul(100)
        .checked_div(total)
        .unwrap_or(100);
    u8::try_from(pct.min(100)).unwrap_or(100)
}

fn is_root_instruction_path(path: &str) -> bool {
    path == "AGENTS.md" || path == "GEMINI.md"
}

fn is_skill_md_path(path: &str) -> bool {
    path.ends_with("/SKILL.md")
        && (path.starts_with(".agents/skills/") || path.starts_with(".agents/plugins/"))
}

fn is_rule_or_instruction_path(path: &str) -> bool {
    path == "AGENTS.md"
        || path == "GEMINI.md"
        || path.ends_with("/AGENTS.md")
        || path.ends_with("/GEMINI.md")
        || (path.starts_with(".agents/rules/")
            && Path::new(path)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("md")))
}

fn is_always_on_rule(path: &str, content: &str) -> bool {
    if path == "AGENTS.md"
        || path == "GEMINI.md"
        || path.ends_with("/AGENTS.md")
        || path.ends_with("/GEMINI.md")
    {
        return true;
    }
    extract_frontmatter(content).is_none_or(|fm| {
        fm.lines().any(|line| {
            let trimmed = line.trim();
            trimmed == "trigger: always_on" || trimmed == "trigger: \"always_on\""
        })
    })
}

fn extract_frontmatter(content: &str) -> Option<&str> {
    let rest = content.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    rest.get(..end)
}

fn expand_at_includes(
    content: &str,
    files: &BTreeMap<String, String>,
    visited: &mut BTreeSet<String>,
    depth: usize,
) -> String {
    if depth >= MAX_INCLUDE_DEPTH {
        return content.to_owned();
    }
    let mut output = String::with_capacity(content.len());
    let mut rest = content;

    while let Some(pos) = rest.find("@[") {
        let Some((before, after)) = rest.split_at_checked(pos) else {
            break;
        };
        output.push_str(before);

        if let Some((target_path, consumed_len)) = parse_at_include(after) {
            let normalized = target_path.trim_start_matches("./");
            if let Some(included) = files.get(normalized)
                && visited.insert(normalized.to_owned())
            {
                let nested = expand_at_includes(included, files, visited, depth.saturating_add(1));
                output.push_str(&nested);
                visited.remove(normalized);
            } else {
                output.push_str(after.get(..consumed_len).unwrap_or_default());
            }
            rest = after.get(consumed_len..).unwrap_or_default();
        } else {
            output.push_str("@[");
            rest = after.get(2..).unwrap_or_default();
        }
    }
    output.push_str(rest);
    output
}

fn parse_at_include(slice: &str) -> Option<(&str, usize)> {
    let after_open = slice.strip_prefix("@[")?;
    let label_end = after_open.find("](")?;
    let path_start = 2usize.saturating_add(label_end).saturating_add(2);
    let after_paren = slice.get(path_start..)?;
    let path_end = after_paren.find(')')?;
    let path = after_paren.get(..path_end)?;
    if path.is_empty() || path.contains('\n') {
        return None;
    }
    let consumed = path_start.saturating_add(path_end).saturating_add(1);
    Some((path, consumed))
}

const fn classify_usage(used: usize, limit: usize) -> BudgetStatus {
    if limit == 0 {
        return if used == 0 {
            BudgetStatus::Ok
        } else {
            BudgetStatus::Exceeded
        };
    }
    if used > limit {
        BudgetStatus::Exceeded
    } else if used.saturating_mul(100) >= limit.saturating_mul(BUDGET_WARN_PERCENT) {
        BudgetStatus::Warning
    } else {
        BudgetStatus::Ok
    }
}

#[cfg(test)]
mod tests {
    use crate::id::ResourceId;
    use crate::manifest::{McpAuthMode, McpTransport};

    use super::*;

    #[test]
    fn expands_at_includes_and_flags_over_24kb_file() {
        let mut ws = BTreeMap::new();
        ws.insert("docs/heavy.md".to_owned(), "x".repeat(25_000));

        let generated = vec![GeneratedFile::whole_file(
            "AGENTS.md",
            "# Rules\n@[heavy](docs/heavy.md)\n",
        )];
        let report = analyze_budgets(&generated, &ws, 24_000, 20_000);
        assert_eq!(report.files.len(), 1);
        assert!(report.files[0].expanded_bytes > 24_000);
        assert_eq!(report.files[0].status, BudgetStatus::Exceeded);
        assert!(report.lean_advisory.root_bytes_exceeded);
        assert_ne!(report.offload_suggestions, Vec::<String>::new());
    }

    #[test]
    fn glob_rules_and_skills_raise_pull_ratio_while_eager_mcp_adds_push_tax() {
        let mut manifest = ProjectManifest::default();
        manifest.mcp_servers.push(McpServerSpec {
            name: ResourceId::parse("eager-srv").unwrap(),
            enabled: true,
            transport: McpTransport::StreamableHttp {
                url: "https://mcp.example.com/mcp".to_owned(),
                auth: McpAuthMode::OAuth,
                headers: BTreeMap::new(),
            },
            variables: Vec::new(),
            tool_loading: McpToolLoading::Eager,
            allowed_tools: Vec::new(),
            denied_tools: Vec::new(),
            risk_surfaces: Vec::new(),
        });
        manifest.mcp_servers.push(McpServerSpec {
            name: ResourceId::parse("deferred-srv").unwrap(),
            enabled: true,
            transport: McpTransport::StreamableHttp {
                url: "https://mcp2.example.com/mcp".to_owned(),
                auth: McpAuthMode::OAuth,
                headers: BTreeMap::new(),
            },
            variables: Vec::new(),
            tool_loading: McpToolLoading::Deferred,
            allowed_tools: Vec::new(),
            denied_tools: Vec::new(),
            risk_surfaces: Vec::new(),
        });

        let generated = vec![
            GeneratedFile::whole_file("AGENTS.md", "Small root guide.\n"),
            GeneratedFile::whole_file(
                ".agents/rules/rust.md",
                format!(
                    "---\ntrigger: glob\nglobs: **/*.rs\n---\n\n{}",
                    "a".repeat(12_000)
                ),
            ),
            GeneratedFile::whole_file(
                ".agents/skills/pr-author/SKILL.md",
                format!(
                    "---\nname: pr-author\ndescription: Drafts pull requests with verification.\n---\n\n{}",
                    "b".repeat(4_000)
                ),
            ),
        ];

        let report = analyze_budgets_with_manifest(&generated, &BTreeMap::new(), &manifest);
        assert_eq!(report.files.len(), 2);
        assert!(report.files[0].counts_as_always_on);
        assert!(!report.files[1].counts_as_always_on);
        assert!(report.always_on_tokens < 50);
        assert_eq!(report.always_on_status, BudgetStatus::Ok);
        assert!(report.push_pull.conditional_rule_tokens >= 3_000);
        assert!(report.push_pull.skill_discovery_tokens > 0);
        assert!(report.push_pull.skill_body_tokens >= 1_000);
        assert_eq!(report.push_pull.eager_mcp_server_count, 1);
        assert_eq!(report.push_pull.deferred_mcp_server_count, 1);
        assert_eq!(
            report.push_pull.eager_mcp_estimated_tokens,
            DEFAULT_EAGER_MCP_TOKENS_PER_SERVER
        );
        assert!(report.push_pull.pull_ratio_percent >= 65);
    }

    #[test]
    fn cjk_token_estimator_weights_multibyte_chars_conservatively() {
        let tokens = estimate_tokens("禁止直接推送到主分支");
        assert!(tokens >= 15);
    }
}
