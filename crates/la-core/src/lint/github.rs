//! GitHub Actions workflow security hygiene (`LA014`–`LA016`) and repository governance
//! alignment (`LA017`) linter rules.

use std::path::Path;

use crate::lint::{
    Diagnostic, DiagnosticLocation, DiagnosticSeverity, LintCode, LintInputs, level_to_severity,
};

const UNTRUSTED_EVENT_EXPRS: [&str; 9] = [
    "github.event.issue.title",
    "github.event.issue.body",
    "github.event.pull_request.title",
    "github.event.pull_request.body",
    "github.event.pull_request.head.ref",
    "github.event.comment.body",
    "github.event.review.body",
    "github.event.head_commit.message",
    "github.head_ref",
];

pub(super) fn run_github_checks(inputs: &LintInputs<'_>, diagnostics: &mut Vec<Diagnostic>) {
    if inputs.manifest.engineering.github.sync.lint_workflows
        && let Some(severity) = level_to_severity(inputs.manifest.lint.github_actions_security)
    {
        for (path, content) in inputs.workspace_files {
            if is_github_workflow(path) {
                lint_workflow_file(path, content, severity, diagnostics);
            }
        }
    }

    check_governance_sync_la017(inputs, diagnostics);
}

fn is_github_workflow(path: &str) -> bool {
    path.starts_with(".github/workflows/")
        && Path::new(path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("yml") || ext.eq_ignore_ascii_case("yaml"))
}

fn lint_workflow_file(
    path: &str,
    content: &str,
    severity: DiagnosticSeverity,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let lines: Vec<&str> = content.lines().collect();

    check_top_level_permissions(path, &lines, severity, diagnostics);
    check_uses_sha_pinning_and_checkout(path, &lines, severity, diagnostics);
    check_pull_request_target_and_injection(path, &lines, severity, diagnostics);
}

fn check_top_level_permissions(
    path: &str,
    lines: &[&str],
    severity: DiagnosticSeverity,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let has_top_level_permissions = lines.iter().any(|line| {
        !line.starts_with(' ')
            && !line.starts_with('\t')
            && line.trim_start().starts_with("permissions:")
    });
    if !has_top_level_permissions {
        diagnostics.push(Diagnostic {
            code: LintCode::La015,
            severity,
            location: DiagnosticLocation::line(path, 1),
            message: "workflow is missing top-level `permissions: {}` least-privilege declaration"
                .to_owned(),
            remediation:
                "Add top-level `permissions: {}` and grant only minimal permissions per job."
                    .to_owned(),
        });
    }
}

fn check_uses_sha_pinning_and_checkout(
    path: &str,
    lines: &[&str],
    severity: DiagnosticSeverity,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (idx, line) in lines.iter().enumerate() {
        let line_no = idx.saturating_add(1);
        let trimmed = line.trim();
        let Some(after_uses) = extract_uses_target(trimmed) else {
            continue;
        };

        if !is_local_or_docker_action(after_uses) && !is_sha_pinned_action(after_uses) {
            diagnostics.push(Diagnostic {
                code: LintCode::La014,
                severity,
                location: DiagnosticLocation::line(path, line_no),
                message: format!(
                    "action `{after_uses}` is not pinned to a full 40-character commit SHA"
                ),
                remediation: "Pin `uses:` to `<owner>/<repo>@<40-char-sha> # vX.Y.Z`.".to_owned(),
            });
        }

        if after_uses.starts_with("actions/checkout@")
            && !checkout_sets_persist_credentials_false(lines, idx)
        {
            diagnostics.push(Diagnostic {
                code: LintCode::La015,
                severity,
                location: DiagnosticLocation::line(path, line_no),
                message: "`actions/checkout` step does not set `persist-credentials: false`"
                    .to_owned(),
                remediation: "Add `with: { persist-credentials: false }` under the checkout step."
                    .to_owned(),
            });
        }
    }
}

fn check_pull_request_target_and_injection(
    path: &str,
    lines: &[&str],
    severity: DiagnosticSeverity,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let has_pr_target = lines
        .iter()
        .any(|line| line.trim_start().starts_with("pull_request_target"));
    if has_pr_target {
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("github.event.pull_request.head") {
                diagnostics.push(Diagnostic {
                    code: LintCode::La016,
                    severity,
                    location: DiagnosticLocation::line(path, idx.saturating_add(1)),
                    message: "workflow combines `pull_request_target` with untrusted PR head ref checkout".to_owned(),
                    remediation: "Avoid checking out untrusted fork code under `pull_request_target`.".to_owned(),
                });
            }
        }
    }

    let mut in_run_block_indent: Option<usize> = None;
    for (idx, line) in lines.iter().enumerate() {
        let line_no = idx.saturating_add(1);
        let indent = leading_spaces(line);
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(block_indent) = in_run_block_indent
            && indent <= block_indent
        {
            in_run_block_indent = None;
        }

        let is_run_key = trimmed.starts_with("run:") || trimmed.starts_with("- run:");
        if is_run_key {
            let after_run = trimmed
                .split_once("run:")
                .map_or("", |(_, tail)| tail.trim());
            if after_run == "|" || after_run == "|-" || after_run == ">" || after_run == ">-" {
                in_run_block_indent = Some(indent);
            } else {
                flag_untrusted_expr(path, line_no, after_run, severity, diagnostics);
            }
        } else if in_run_block_indent.is_some() {
            flag_untrusted_expr(path, line_no, trimmed, severity, diagnostics);
        }
    }
}

fn flag_untrusted_expr(
    path: &str,
    line_no: usize,
    text: &str,
    severity: DiagnosticSeverity,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !text.contains("${{") {
        return;
    }
    for expr in UNTRUSTED_EVENT_EXPRS {
        if text.contains(expr) {
            diagnostics.push(Diagnostic {
                code: LintCode::La016,
                severity,
                location: DiagnosticLocation::line(path, line_no),
                message: format!(
                    "untrusted expression `${{{{ {expr} }}}}` interpolated directly inside `run:` script"
                ),
                remediation: "Pass untrusted event values via step `env:` variables and quote them in the shell script.".to_owned(),
            });
        }
    }
}

fn check_governance_sync_la017(inputs: &LintInputs<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let protected = &inputs.manifest.engineering.protected_paths;
    if !protected.entries.is_empty()
        && let Some(codeowners) = inputs.workspace_files.get(".github/CODEOWNERS")
    {
        for entry in &protected.entries {
            if entry.owners.is_empty() {
                continue;
            }
            let needle = entry.glob.trim_end_matches("/**").trim_start_matches('/');
            if !needle.is_empty() && !codeowners.contains(needle) {
                diagnostics.push(Diagnostic {
                    code: LintCode::La017,
                    severity: DiagnosticSeverity::Warning,
                    location: DiagnosticLocation::file(".github/CODEOWNERS"),
                    message: format!(
                        "protected path `{}` is not covered in `.github/CODEOWNERS`",
                        entry.glob
                    ),
                    remediation: "Enable `protected_paths.codeowners_sync = true` and run `lazyagents generate`.".to_owned(),
                });
            }
        }
    }

    let required_cmds: Vec<&str> = inputs
        .manifest
        .engineering
        .verify
        .commands
        .iter()
        .filter(|c| c.required)
        .map(|c| c.cmd.as_str())
        .collect();
    if !required_cmds.is_empty()
        && let Some(pr_tpl) = inputs
            .workspace_files
            .get(".github/pull_request_template.md")
    {
        for cmd in required_cmds {
            if !pr_tpl.contains(cmd) {
                diagnostics.push(Diagnostic {
                    code: LintCode::La017,
                    severity: DiagnosticSeverity::Warning,
                    location: DiagnosticLocation::file(".github/pull_request_template.md"),
                    message: format!(
                        "required verification command `{cmd}` is missing from PR template"
                    ),
                    remediation: "Enable `engineering.github.sync.pr_template = true` and run `lazyagents generate`.".to_owned(),
                });
            }
        }
    }
}

fn extract_uses_target(trimmed: &str) -> Option<&str> {
    let rest = trimmed
        .strip_prefix("uses:")
        .or_else(|| trimmed.strip_prefix("- uses:"))?;
    let without_comment = rest.split('#').next().unwrap_or(rest).trim();
    let unquoted = without_comment.trim_matches('"').trim_matches('\'');
    if unquoted.is_empty() {
        None
    } else {
        Some(unquoted)
    }
}

fn is_local_or_docker_action(target: &str) -> bool {
    target.starts_with("./") || target.starts_with("docker://")
}

fn is_sha_pinned_action(target: &str) -> bool {
    let Some((_, rev)) = target.rsplit_once('@') else {
        return false;
    };
    rev.len() == 40
        && rev
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
}

fn checkout_sets_persist_credentials_false(lines: &[&str], uses_idx: usize) -> bool {
    let uses_line = lines.get(uses_idx).copied().unwrap_or_default();
    let step_indent = leading_spaces(uses_line);

    for line in lines.iter().skip(uses_idx.saturating_add(1)) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = leading_spaces(line);
        if indent < step_indent || (indent == step_indent && trimmed.starts_with("- ")) {
            break;
        }
        if trimmed == "persist-credentials: false" {
            return true;
        }
    }
    false
}

fn leading_spaces(line: &str) -> usize {
    line.bytes().take_while(|b| *b == b' ').count()
}
