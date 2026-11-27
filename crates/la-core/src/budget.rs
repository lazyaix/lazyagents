//! Antigravity context budget telemetry computing per-file UTF-8 byte sizes
//! (after expanding `@[label](path)` includes) and aggregate always-on token budgets.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::compiler::GeneratedFile;

/// Warning threshold percentage (80%) for Antigravity's aggregate always-on token budget.
pub const BUDGET_WARN_PERCENT: usize = 80;

/// Maximum recursive expansion depth for `@[label](path)` includes.
const MAX_INCLUDE_DEPTH: usize = 8;

/// Budget status classification for a file or aggregate rule set.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetStatus {
    /// Safely below warning threshold.
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

/// Complete context budget report consumed by `LA002`/`LA003`, CLI `inspect`, and the TUI Radar.
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
    /// Actionable offload hints when files approach or exceed budgets.
    pub offload_suggestions: Vec<String>,
}

/// Analyzes Antigravity context budgets across compiled output files and available workspace files.
#[must_use]
pub fn analyze_budgets(
    generated: &[GeneratedFile],
    workspace_files: &BTreeMap<String, String>,
    rule_max_bytes: usize,
    always_on_max_tokens: usize,
) -> BudgetReport {
    let mut all_files = workspace_files.clone();
    for file in generated {
        all_files.insert(file.path.clone(), file.content.clone());
    }

    let mut file_budgets = Vec::new();
    let mut always_on_tokens = 0usize;
    let mut offload_suggestions = Vec::new();

    for file in generated {
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

    let always_on_status = classify_usage(always_on_tokens, always_on_max_tokens);
    if always_on_status != BudgetStatus::Ok {
        offload_suggestions.push(format!(
            "Always-on rules total ~{always_on_tokens}/{always_on_max_tokens} tokens — demote non-global rules to `glob` or `model_decision` triggers"
        ));
    }

    BudgetReport {
        files: file_budgets,
        always_on_tokens,
        max_always_on_tokens: always_on_max_tokens,
        always_on_status,
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
        assert_ne!(report.offload_suggestions, Vec::<String>::new());
    }

    #[test]
    fn glob_rules_do_not_inflate_always_on_token_budget() {
        let generated = vec![
            GeneratedFile::whole_file("AGENTS.md", "Small root guide.\n"),
            GeneratedFile::whole_file(
                ".agents/rules/rust.md",
                format!(
                    "---\ntrigger: glob\nglobs: **/*.rs\n---\n\n{}",
                    "a".repeat(12_000)
                ),
            ),
        ];
        let report = analyze_budgets(&generated, &BTreeMap::new(), 24_000, 20_000);
        assert_eq!(report.files.len(), 2);
        assert!(report.files[0].counts_as_always_on);
        assert!(!report.files[1].counts_as_always_on);
        assert!(report.always_on_tokens < 50);
        assert_eq!(report.always_on_status, BudgetStatus::Ok);
    }

    #[test]
    fn cjk_token_estimator_weights_multibyte_chars_conservatively() {
        let tokens = estimate_tokens("禁止直接推送到主分支");
        assert!(tokens >= 15);
    }
}
