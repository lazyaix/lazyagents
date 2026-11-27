//! Empirical context-efficiency (`LA018`) and skill-quality (`LA019`) lint checks grounded in
//! *Evaluating AGENTS.md* (`arXiv:2602.11988`), *`SkillsBench`* (`arXiv:2602.12670`), and the
//! `agentskills.io` progressive-disclosure specification.

use crate::budget::BudgetReport;
use crate::lint::antigravity::{extract_yaml_frontmatter, parse_frontmatter_scalar};
use crate::lint::{Diagnostic, DiagnosticLocation, LintCode, LintInputs, level_to_severity};

/// Maximum description length permitted by the `agentskills.io` `SKILL.md` specification.
const MAX_SKILL_DESCRIPTION_CHARS: usize = 1_024;

/// Minimum description length needed for reliable progressive-disclosure routing.
const MIN_SKILL_DESCRIPTION_CHARS: usize = 20;

/// Minimum consecutive ASCII directory-tree lines considered a stale-overview anti-pattern.
const MIN_STATIC_TREE_LINES: usize = 12;

/// Number of header lines scanned for KV-cache-busting dynamic clock/timestamp placeholders.
const KV_CACHE_HEADER_LINES: usize = 25;

pub(super) fn run_empirical_checks(
    inputs: &LintInputs<'_>,
    budget: &BudgetReport,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_context_lean_la018(inputs, budget, diagnostics);
    check_skill_quality_la019(inputs, diagnostics);
}

fn check_context_lean_la018(
    inputs: &LintInputs<'_>,
    budget: &BudgetReport,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(severity) = level_to_severity(inputs.manifest.lint.context_lean) else {
        return;
    };

    for file in &budget.files {
        if (file.path == "AGENTS.md" || file.path == "GEMINI.md")
            && file.expanded_bytes > budget.lean_advisory.recommended_root_bytes
        {
            diagnostics.push(Diagnostic {
                code: LintCode::La018,
                severity,
                location: DiagnosticLocation::file(&file.path),
                message: format!(
                    "root instruction `{}` is {} bytes (recommended <= {} bytes; empirical studies show bloated context files increase inference cost by >20%)",
                    file.path, file.expanded_bytes, budget.lean_advisory.recommended_root_bytes
                ),
                remediation: "Keep root `AGENTS.md` to non-obvious commands, hard boundaries, and doc pointers; offload domain details into `glob` rules or `SKILL.md`.".to_owned(),
            });
        }
    }

    if budget.lean_advisory.always_on_exceeded {
        diagnostics.push(Diagnostic {
            code: LintCode::La018,
            severity,
            location: DiagnosticLocation::file("AGENTS.md"),
            message: format!(
                "always-on context estimated at {} tokens (recommended <= {} tokens for lean inference)",
                budget.always_on_tokens, budget.lean_advisory.recommended_always_on_tokens
            ),
            remediation: "Demote directory-specific guidance to `trigger = \"glob\"` rules and multi-step procedures to on-demand Skills.".to_owned(),
        });
    }

    for file in &inputs.compile_output.files {
        if !(file.path == "AGENTS.md"
            || file.path == "GEMINI.md"
            || file.path.ends_with("/AGENTS.md")
            || file.path.ends_with("/GEMINI.md"))
        {
            continue;
        }
        if let Some(tree_start) = detect_static_directory_tree(&file.content) {
            diagnostics.push(Diagnostic {
                code: LintCode::La018,
                severity,
                location: DiagnosticLocation::line(&file.path, tree_start),
                message: format!(
                    "`{}` embeds a static directory tree (>= {MIN_STATIC_TREE_LINES} lines), which goes stale quickly and does not improve file localization",
                    file.path
                ),
                remediation: "Remove static repository trees and let agents inspect directories dynamically; list only non-obvious entrypoints or boundaries.".to_owned(),
            });
        }
    }

    for (rel, content) in inputs.source_files {
        if !rel.starts_with("sections/") {
            continue;
        }
        if let Some(line_no) = detect_kv_cache_busting_header(content) {
            diagnostics.push(Diagnostic {
                code: LintCode::La018,
                severity,
                location: DiagnosticLocation::line(format!(".lazyagents/{rel}"), line_no),
                message: "dynamic timestamp/clock placeholder near top of always-on instruction section can invalidate prompt KV-cache prefixes".to_owned(),
                remediation: "Keep always-on `AGENTS.md` prefixes strictly deterministic and static; inject session-dynamic facts via `SessionStart` hooks instead.".to_owned(),
            });
        }
    }
}

fn check_skill_quality_la019(inputs: &LintInputs<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let Some(severity) = level_to_severity(inputs.manifest.lint.skill_quality) else {
        return;
    };
    let max_lines = inputs.manifest.lint.recommended_skill_max_lines;

    for file in &inputs.compile_output.files {
        if !is_compiled_skill_md(&file.path) {
            continue;
        }

        if let Some(fm) = extract_yaml_frontmatter(&file.content)
            && let Some(desc) = parse_frontmatter_scalar(fm, "description")
        {
            let trimmed = desc.trim();
            let char_len = trimmed.chars().count();
            if char_len > MAX_SKILL_DESCRIPTION_CHARS {
                diagnostics.push(Diagnostic {
                    code: LintCode::La019,
                    severity,
                    location: DiagnosticLocation::line(&file.path, 1),
                    message: format!(
                        "SKILL.md `description` is {char_len} chars (agentskills.io cap is {MAX_SKILL_DESCRIPTION_CHARS} chars)"
                    ),
                    remediation: "Tighten `description` to a concise third-person summary stating what the skill does and when to trigger it.".to_owned(),
                });
            } else if !trimmed.is_empty() && char_len < MIN_SKILL_DESCRIPTION_CHARS {
                diagnostics.push(Diagnostic {
                    code: LintCode::La019,
                    severity,
                    location: DiagnosticLocation::line(&file.path, 1),
                    message: format!(
                        "SKILL.md `description` is only {char_len} chars — too terse for reliable progressive-disclosure activation"
                    ),
                    remediation: "State both what the skill does and concrete trigger keywords/scenarios (aim for 40-250 chars).".to_owned(),
                });
            }

            if uses_first_person_voice(trimmed) {
                diagnostics.push(Diagnostic {
                    code: LintCode::La019,
                    severity,
                    location: DiagnosticLocation::line(&file.path, 1),
                    message: "SKILL.md `description` uses first-person pronouns (`I`/`We`/`My`), which clashes with system-prompt discovery lists".to_owned(),
                    remediation: "Rewrite `description` in third person (e.g., \"Reviews Rust pull requests for...\").".to_owned(),
                });
            }
        }

        let body = strip_frontmatter_body(&file.content);
        let body_lines = body.lines().count();
        if max_lines > 0 && body_lines > max_lines {
            diagnostics.push(Diagnostic {
                code: LintCode::La019,
                severity,
                location: DiagnosticLocation::file(&file.path),
                message: format!(
                    "SKILL.md body has {body_lines} lines (recommended <= {max_lines} lines per SkillsBench modular-skill findings)"
                ),
                remediation: "Split deep reference material into sibling `references/` files and deterministic procedures into `scripts/`.".to_owned(),
            });
        }
    }
}

fn is_compiled_skill_md(path: &str) -> bool {
    path.ends_with("/SKILL.md")
        && (path.starts_with(".agents/skills/") || path.starts_with(".agents/plugins/"))
}

fn strip_frontmatter_body(content: &str) -> &str {
    if let Some(rest) = content.strip_prefix("---\n")
        && let Some(end) = rest.find("\n---")
    {
        return rest
            .get(end.saturating_add(4)..)
            .unwrap_or_default()
            .trim_start_matches('\n');
    }
    content
}

fn detect_static_directory_tree(content: &str) -> Option<usize> {
    let mut streak = 0usize;
    let mut streak_start = 1usize;
    for (idx, line) in content.lines().enumerate() {
        let line_no = idx.saturating_add(1);
        let is_tree_line = line.contains("├──")
            || line.contains("└──")
            || line.contains("│   ")
            || line.trim_start().starts_with("|-- ")
            || line.trim_start().starts_with("+-- ");
        if is_tree_line {
            if streak == 0 {
                streak_start = line_no;
            }
            streak = streak.saturating_add(1);
            if streak >= MIN_STATIC_TREE_LINES {
                return Some(streak_start);
            }
        } else {
            streak = 0;
        }
    }
    None
}

fn detect_kv_cache_busting_header(content: &str) -> Option<usize> {
    let needles = [
        "{{ now(",
        "{{now(",
        "current_timestamp",
        "current_date",
        "generated_at",
    ];
    for (idx, line) in content.lines().take(KV_CACHE_HEADER_LINES).enumerate() {
        let lower = line.to_ascii_lowercase();
        if needles.iter().any(|needle| lower.contains(needle)) {
            return Some(idx.saturating_add(1));
        }
    }
    None
}

fn uses_first_person_voice(desc: &str) -> bool {
    let starts = ["I ", "I'", "We ", "We'", "My ", "Our "];
    if starts.iter().any(|prefix| desc.starts_with(prefix)) {
        return true;
    }
    let padded = format!(" {desc} ");
    let mid = [" I ", " I'", " we ", " we'", " my ", " our "];
    mid.iter().any(|token| padded.contains(token))
}
