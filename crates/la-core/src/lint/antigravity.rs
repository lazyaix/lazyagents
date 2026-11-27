//! Antigravity compliance, context budget, secret hygiene, and drift lint checks (`LA001`–`LA013`).

use std::collections::BTreeSet;
use std::path::Path;

use regex::Regex;

use crate::budget::{BudgetReport, BudgetStatus};
use crate::id::{KebabName, ResourceId};
use crate::lint::{
    Diagnostic, DiagnosticLocation, DiagnosticSeverity, LintCode, LintInputs, level_to_severity,
};
use crate::manifest::{HookMatcher, McpTransport, RuleTrigger};
use crate::secret::scan_secrets;

const BUILTIN_ANTIGRAVITY_AGENTS: [&str; 5] =
    ["self", "browser", "research", "deepagent", "review-agent"];

const ALLOWED_LOGO_EXTS: [&str; 5] = [".png", ".svg", ".jpg", ".jpeg", ".webp"];

pub(super) fn run_antigravity_checks(
    inputs: &LintInputs<'_>,
    budget: &BudgetReport,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_secrets_la001(inputs, diagnostics);
    check_budgets_la002_la003(budget, diagnostics);
    check_skills_la004(inputs, diagnostics);
    check_rules_la005(inputs, diagnostics);
    check_mcp_la006_la008(inputs, diagnostics);
    check_hooks_la009(inputs, diagnostics);
    check_plugins_la010(inputs, diagnostics);
    check_collisions_la011(inputs, diagnostics);
    check_portability_la012(inputs, diagnostics);
    check_drift_la013(inputs, diagnostics);
}

fn check_secrets_la001(inputs: &LintInputs<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let Some(severity) = level_to_severity(inputs.manifest.lint.secrets) else {
        return;
    };

    emit_secret_findings(
        ".lazyagents/lazyagents.toml",
        inputs.manifest_toml,
        severity,
        diagnostics,
    );
    for (rel, content) in inputs.source_files {
        emit_secret_findings(
            &format!(".lazyagents/{rel}"),
            content,
            severity,
            diagnostics,
        );
    }
    for file in &inputs.compile_output.files {
        emit_secret_findings(&file.path, &file.content, severity, diagnostics);
    }
}

fn emit_secret_findings(
    path: &str,
    content: &str,
    severity: DiagnosticSeverity,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for finding in scan_secrets(content) {
        diagnostics.push(Diagnostic {
            code: LintCode::La001,
            severity,
            location: DiagnosticLocation {
                path: path.to_owned(),
                line: Some(finding.line),
                column: Some(finding.column),
            },
            message: format!(
                "detected plaintext {:?} ({}) in tracked configuration",
                finding.kind, finding.preview
            ),
            remediation: "Replace the literal credential with a `${VAR}` placeholder and Antigravity `variables` entry, or move personal overrides to gitignored `.lazyagents/local.toml`.".to_owned(),
        });
    }
}

fn check_budgets_la002_la003(budget: &BudgetReport, diagnostics: &mut Vec<Diagnostic>) {
    for file in &budget.files {
        if file.status == BudgetStatus::Exceeded {
            diagnostics.push(Diagnostic {
                code: LintCode::La002,
                severity: DiagnosticSeverity::Error,
                location: DiagnosticLocation::file(&file.path),
                message: format!(
                    "expanded size ({} bytes) exceeds Antigravity's {}-byte per-file rule cap",
                    file.expanded_bytes, file.max_bytes
                ),
                remediation: "Split detailed guidelines into `glob`-triggered `.agents/rules/*.md` files or on-demand Skills to prevent silent line truncation.".to_owned(),
            });
        }
    }

    match budget.always_on_status {
        BudgetStatus::Warning => {
            diagnostics.push(Diagnostic {
                code: LintCode::La003,
                severity: DiagnosticSeverity::Warning,
                location: DiagnosticLocation::file("AGENTS.md"),
                message: format!(
                    "always-on rules estimated at {}/{} tokens (>= 80% of Antigravity defaultRulesBudget)",
                    budget.always_on_tokens, budget.max_always_on_tokens
                ),
                remediation: "Consider moving specialized rules to `glob` or `model_decision` triggers before hitting the 20,000-token demotion cap.".to_owned(),
            });
        }
        BudgetStatus::Exceeded => {
            diagnostics.push(Diagnostic {
                code: LintCode::La003,
                severity: DiagnosticSeverity::Error,
                location: DiagnosticLocation::file("AGENTS.md"),
                message: format!(
                    "always-on rules estimated at {}/{} tokens, exceeding Antigravity's defaultRulesBudget",
                    budget.always_on_tokens, budget.max_always_on_tokens
                ),
                remediation: "Over-budget rules are demoted to file path pointers; offload non-global content into Skills or `glob`-scoped rules.".to_owned(),
            });
        }
        BudgetStatus::Ok => {}
    }
}

fn check_skills_la004(inputs: &LintInputs<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let Some(severity) = level_to_severity(inputs.manifest.lint.skill_frontmatter) else {
        return;
    };

    for file in &inputs.compile_output.files {
        let Some(rest) = file.path.strip_prefix(".agents/skills/") else {
            continue;
        };
        let Some(dir_name) = rest.strip_suffix("/SKILL.md") else {
            continue;
        };
        if dir_name.contains('/') {
            continue;
        }
        validate_skill_md(&file.path, dir_name, &file.content, severity, diagnostics);
    }
}

fn validate_skill_md(
    path: &str,
    dir_name: &str,
    content: &str,
    severity: DiagnosticSeverity,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(fm) = extract_yaml_frontmatter(content) else {
        diagnostics.push(Diagnostic {
            code: LintCode::La004,
            severity,
            location: DiagnosticLocation::line(path, 1),
            message: "SKILL.md is missing required YAML frontmatter (`---`)".to_owned(),
            remediation: format!(
                "Add YAML frontmatter with `name: {dir_name}` and a non-empty `description`."
            ),
        });
        return;
    };

    let name = parse_frontmatter_scalar(fm, "name");
    let description = parse_frontmatter_scalar(fm, "description");

    match name {
        None => diagnostics.push(Diagnostic {
            code: LintCode::La004,
            severity,
            location: DiagnosticLocation::line(path, 1),
            message: "SKILL.md frontmatter is missing required `name` field".to_owned(),
            remediation: format!("Set `name: {dir_name}` so it matches the skill folder name."),
        }),
        Some(ref value) if value != dir_name => diagnostics.push(Diagnostic {
            code: LintCode::La004,
            severity,
            location: DiagnosticLocation::line(path, 1),
            message: format!(
                "SKILL.md frontmatter `name: {value}` does not match folder `{dir_name}`"
            ),
            remediation: format!("Rename `name` in frontmatter to `{dir_name}` so Antigravity does not drop this skill."),
        }),
        Some(ref value) if KebabName::parse(value).is_err() => diagnostics.push(Diagnostic {
            code: LintCode::La004,
            severity,
            location: DiagnosticLocation::line(path, 1),
            message: format!("SKILL.md `name: {value}` is not lowercase kebab-case"),
            remediation: "Use lowercase ASCII letters, digits, and single hyphens.".to_owned(),
        }),
        Some(_) => {}
    }

    if description.is_none_or(|d| d.trim().is_empty()) {
        diagnostics.push(Diagnostic {
            code: LintCode::La004,
            severity,
            location: DiagnosticLocation::line(path, 1),
            message: "SKILL.md frontmatter requires a non-empty `description`".to_owned(),
            remediation:
                "Describe clearly what the skill does and when the agent should activate it."
                    .to_owned(),
        });
    }
}

fn check_rules_la005(inputs: &LintInputs<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for rule in &inputs.manifest.rules {
        if rule.trigger == RuleTrigger::Glob && rule.globs.is_empty() {
            diagnostics.push(Diagnostic {
                code: LintCode::La005,
                severity: DiagnosticSeverity::Error,
                location: DiagnosticLocation::file(".lazyagents/lazyagents.toml"),
                message: format!(
                    "rule `{}` uses `trigger = \"glob\"` with an empty `globs` list",
                    rule.id
                ),
                remediation: "Provide at least one file glob (e.g., `globs = [\"crates/**/*.rs\"]`) or switch trigger to `always_on`/`model_decision`.".to_owned(),
            });
        }
        if rule.trigger == RuleTrigger::ModelDecision
            && rule
                .description
                .as_deref()
                .is_none_or(|desc| desc.trim().is_empty())
        {
            diagnostics.push(Diagnostic {
                code: LintCode::La005,
                severity: DiagnosticSeverity::Warning,
                location: DiagnosticLocation::file(".lazyagents/lazyagents.toml"),
                message: format!(
                    "rule `{}` uses `trigger = \"model_decision\"` without a `description`",
                    rule.id
                ),
                remediation:
                    "Add a clear `description` so Antigravity knows when to load this rule."
                        .to_owned(),
            });
        }
    }

    for (path, content) in inputs.workspace_files {
        let is_rule_md = path.starts_with(".agents/rules/")
            && Path::new(path)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("md"));
        if !is_rule_md {
            continue;
        }
        if let Some(fm) = extract_yaml_frontmatter(content)
            && fm.lines().any(|l| {
                l.trim_start().starts_with("globs:")
                    && (l.contains('{') || l.trim_end().ends_with("globs:"))
            })
        {
            diagnostics.push(Diagnostic {
                code: LintCode::La005,
                severity: DiagnosticSeverity::Error,
                location: DiagnosticLocation::line(path, 1),
                message: "Antigravity rule `globs` must be a flat comma-separated string with braces expanded".to_owned(),
                remediation: "Run `lazyagents generate` to normalize rule frontmatter globs.".to_owned(),
            });
        }
    }
}

fn check_mcp_la006_la008(inputs: &LintInputs<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let sse_severity = level_to_severity(inputs.manifest.lint.legacy_sse_mcp);
    let plugin_mcp_names: BTreeSet<&str> = inputs
        .manifest
        .plugins
        .iter()
        .flat_map(|p| p.include_mcp.iter().map(ResourceId::as_str))
        .collect();

    for server in &inputs.manifest.mcp_servers {
        let declared_vars: BTreeSet<&str> =
            server.variables.iter().map(|v| v.name.as_str()).collect();

        match &server.transport {
            McpTransport::LegacySse { url, headers } => {
                if let Some(severity) = sse_severity {
                    diagnostics.push(Diagnostic {
                        code: LintCode::La006,
                        severity,
                        location: DiagnosticLocation::file(".lazyagents/lazyagents.toml"),
                        message: format!(
                            "MCP server `{}` uses legacy HTTP+SSE transport (`{url}`), which Antigravity does not support",
                            server.name
                        ),
                        remediation: format!(
                            "Point `url` to a Streamable HTTP endpoint or run stdio bridge `npx -y mcp-remote {url}`."
                        ),
                    });
                }
                check_undeclared_vars(
                    server.name.as_str(),
                    headers.values().map(String::as_str),
                    &declared_vars,
                    diagnostics,
                );
            }
            McpTransport::StreamableHttp { url, headers } => {
                if url.ends_with("/sse")
                    && let Some(severity) = sse_severity
                {
                    diagnostics.push(Diagnostic {
                        code: LintCode::La006,
                        severity,
                        location: DiagnosticLocation::file(".lazyagents/lazyagents.toml"),
                        message: format!(
                            "MCP server `{}` URL `{url}` ends with `/sse` (legacy SSE is unsupported on Antigravity)",
                            server.name
                        ),
                        remediation: "Use the server's Streamable HTTP endpoint (typically `/mcp`) or bridge via `npx -y mcp-remote`.".to_owned(),
                    });
                }
                check_undeclared_vars(
                    server.name.as_str(),
                    core::iter::once(url.as_str()).chain(headers.values().map(String::as_str)),
                    &declared_vars,
                    diagnostics,
                );
            }
            McpTransport::Stdio {
                command,
                args,
                env,
                cwd,
            } => {
                for (key, val) in env {
                    let redundant = val == &format!("${{{key}}}") || val == &format!("${key}");
                    if redundant {
                        diagnostics.push(Diagnostic {
                            code: LintCode::La007,
                            severity: DiagnosticSeverity::Warning,
                            location: DiagnosticLocation::file(".lazyagents/lazyagents.toml"),
                            message: format!(
                                "MCP server `{}` has redundant stdio env pass-through `{key} = \"{val}\"`",
                                server.name
                            ),
                            remediation: "Remove redundant pass-through entries; stdio servers inherit OS environment variables directly.".to_owned(),
                        });
                    }
                }
                check_undeclared_vars(
                    server.name.as_str(),
                    core::iter::once(command.as_str())
                        .chain(args.iter().map(String::as_str))
                        .chain(env.values().map(String::as_str)),
                    &declared_vars,
                    diagnostics,
                );
                if plugin_mcp_names.contains(server.name.as_str())
                    && let Some(dir) = cwd
                    && !is_absolute_path(dir)
                {
                    diagnostics.push(Diagnostic {
                        code: LintCode::La008,
                        severity: DiagnosticSeverity::Error,
                        location: DiagnosticLocation::file(".lazyagents/lazyagents.toml"),
                        message: format!(
                            "plugin-bundled MCP server `{}` uses relative `cwd = \"{dir}\"`",
                            server.name
                        ),
                        remediation: "Use an absolute `cwd` or omit `cwd`; relative paths in plugin MCP configs resolve against the plugin directory.".to_owned(),
                    });
                }
            }
        }
    }
}

fn check_undeclared_vars<'a>(
    server_name: &str,
    values: impl Iterator<Item = &'a str>,
    declared: &BTreeSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut warned = BTreeSet::new();
    for val in values {
        for var in extract_brace_placeholders(val) {
            if !declared.contains(var) && warned.insert(var.to_owned()) {
                diagnostics.push(Diagnostic {
                    code: LintCode::La007,
                    severity: DiagnosticSeverity::Warning,
                    location: DiagnosticLocation::file(".lazyagents/lazyagents.toml"),
                    message: format!(
                        "MCP server `{server_name}` references `${{{var}}}` without a matching `variables` declaration"
                    ),
                    remediation: format!(
                        "Declare `[[mcp_servers.variables]] name = \"{var}\"` because Antigravity only substitutes declared setup variables."
                    ),
                });
            }
        }
    }
}

fn check_hooks_la009(inputs: &LintInputs<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for hook in &inputs.manifest.hooks {
        if let Some(HookMatcher::Regex { regex }) = &hook.matcher
            && regex != "*"
            && Regex::new(regex).is_err()
        {
            diagnostics.push(Diagnostic {
                code: LintCode::La009,
                severity: DiagnosticSeverity::Error,
                location: DiagnosticLocation::file(".lazyagents/lazyagents.toml"),
                message: format!("hook `{}` has invalid matcher regex `{regex}`", hook.name),
                remediation: "Provide a valid regular expression (e.g., `run_command|edit_file`) or use `{ capability = \"shell\" }`.".to_owned(),
            });
        }

        if let Some(rel_script) = hook.command.trim().strip_prefix("./") {
            let script_path = rel_script.split_whitespace().next().unwrap_or_default();
            let exists = inputs.source_files.contains_key(script_path)
                || inputs.workspace_files.contains_key(script_path)
                || inputs
                    .workspace_files
                    .contains_key(&format!(".agents/{script_path}"));
            if !script_path.is_empty() && !exists {
                diagnostics.push(Diagnostic {
                    code: LintCode::La009,
                    severity: DiagnosticSeverity::Error,
                    location: DiagnosticLocation::file(".lazyagents/lazyagents.toml"),
                    message: format!(
                        "hook `{}` references missing local script `./{script_path}`",
                        hook.name
                    ),
                    remediation: format!(
                        "Create `.lazyagents/{script_path}` or switch to built-in `lazyagents hook eval <guard>`."
                    ),
                });
            }
        }
    }
}

fn check_plugins_la010(inputs: &LintInputs<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for plugin in &inputs.manifest.plugins {
        if let Some(logo) = &plugin.logo {
            let bad_origin = logo.starts_with('/')
                || logo.starts_with("http://")
                || logo.starts_with("https://");
            let lower = logo.to_ascii_lowercase();
            let bad_ext = !ALLOWED_LOGO_EXTS.iter().any(|ext| lower.ends_with(ext));
            if bad_origin || bad_ext {
                diagnostics.push(Diagnostic {
                    code: LintCode::La010,
                    severity: DiagnosticSeverity::Error,
                    location: DiagnosticLocation::file(".lazyagents/lazyagents.toml"),
                    message: format!(
                        "plugin `{}` has invalid logo path `{logo}` (must be plugin-relative .png/.svg/.jpg/.jpeg/.webp)",
                        plugin.name
                    ),
                    remediation: "Use a relative asset path such as `assets/logo.svg`.".to_owned(),
                });
            }
        }
        if let Some(homepage) = &plugin.homepage
            && !homepage.starts_with("https://")
        {
            diagnostics.push(Diagnostic {
                code: LintCode::La010,
                severity: DiagnosticSeverity::Error,
                location: DiagnosticLocation::file(".lazyagents/lazyagents.toml"),
                message: format!(
                    "plugin `{}` `homepage` URL `{homepage}` must use `https://`",
                    plugin.name
                ),
                remediation: "Use an `https://` URL for plugin `homepage` metadata.".to_owned(),
            });
        }
        if let Some(repository) = &plugin.repository
            && !repository.starts_with("https://")
        {
            diagnostics.push(Diagnostic {
                code: LintCode::La010,
                severity: DiagnosticSeverity::Error,
                location: DiagnosticLocation::file(".lazyagents/lazyagents.toml"),
                message: format!(
                    "plugin `{}` `repository` URL `{repository}` must use `https://`",
                    plugin.name
                ),
                remediation: "Use an `https://` URL for plugin `repository` metadata.".to_owned(),
            });
        }
        if plugin.suggested_prompts.len() > 3 {
            diagnostics.push(Diagnostic {
                code: LintCode::La010,
                severity: DiagnosticSeverity::Warning,
                location: DiagnosticLocation::file(".lazyagents/lazyagents.toml"),
                message: format!(
                    "plugin `{}` defines {} `suggested_prompts`; Antigravity ignores prompts beyond 3",
                    plugin.name,
                    plugin.suggested_prompts.len()
                ),
                remediation: "Trim `suggested_prompts` to at most 3 concise starter prompts.".to_owned(),
            });
        }
    }

    for path in inputs.workspace_files.keys() {
        if path.starts_with(".agents/plugins/") && path.ends_with("/hooks/hooks.json") {
            diagnostics.push(Diagnostic {
                code: LintCode::La010,
                severity: DiagnosticSeverity::Error,
                location: DiagnosticLocation::file(path),
                message: "native Antigravity plugins ignore `hooks/hooks.json`; place `hooks.json` directly at the plugin root".to_owned(),
                remediation: "Move `hooks/hooks.json` to `<plugin_dir>/hooks.json`.".to_owned(),
            });
        }
    }
}

fn check_collisions_la011(inputs: &LintInputs<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for agent in &inputs.manifest.agents {
        if BUILTIN_ANTIGRAVITY_AGENTS.contains(&agent.name.as_str()) {
            diagnostics.push(Diagnostic {
                code: LintCode::La011,
                severity: DiagnosticSeverity::Error,
                location: DiagnosticLocation::file(".lazyagents/lazyagents.toml"),
                message: format!(
                    "subagent name `{}` collides with a built-in Antigravity agent",
                    agent.name
                ),
                remediation: format!(
                    "Prefix the subagent name (e.g., `project-{}`) to avoid shadowing built-ins.",
                    agent.name
                ),
            });
        }
    }
}

fn check_portability_la012(inputs: &LintInputs<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let Some(severity) = level_to_severity(inputs.manifest.lint.compatibility_loss) else {
        return;
    };
    for compat in &inputs.compile_output.diagnostics {
        diagnostics.push(Diagnostic {
            code: LintCode::La012,
            severity,
            location: DiagnosticLocation::file(".lazyagents/lazyagents.toml"),
            message: format!("[{}] {}", compat.target, compat.message),
            remediation:
                "Disable unsupported targets or configure a target-specific override block."
                    .to_owned(),
        });
    }
}

fn check_drift_la013(inputs: &LintInputs<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let Some(plan) = inputs.write_plan else {
        return;
    };
    for file in &plan.files {
        if file.drifted {
            diagnostics.push(Diagnostic {
                code: LintCode::La013,
                severity: DiagnosticSeverity::Error,
                location: DiagnosticLocation::file(&file.path),
                message: format!(
                    "managed output `{}` drifted from `.lazyagents/` ({:?})",
                    file.path, file.action
                ),
                remediation: "Run `lazyagents generate` to sync outputs, or `lazyagents import` if intentional hand edits should be brought into `.lazyagents/`.".to_owned(),
            });
        }
    }
}

fn extract_yaml_frontmatter(content: &str) -> Option<&str> {
    let rest = content.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    rest.get(..end)
}

fn parse_frontmatter_scalar(fm: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}:");
    let lines: Vec<&str> = fm.lines().collect();
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if let Some(after) = trimmed.strip_prefix(&prefix) {
            let value = after.trim().trim_matches('"').trim_matches('\'');
            if value == ">" || value == ">-" || value == "|" || value == "|-" {
                let mut block = Vec::new();
                for next in lines.iter().skip(idx.saturating_add(1)) {
                    if next.starts_with(' ') || next.starts_with('\t') {
                        block.push(next.trim());
                    } else {
                        break;
                    }
                }
                return Some(block.join(" "));
            }
            return Some(value.to_owned());
        }
    }
    None
}

fn extract_brace_placeholders(input: &str) -> Vec<&str> {
    let mut vars = Vec::new();
    let mut rest = input;
    while let Some(pos) = rest.find("${") {
        let after = rest.get(pos.saturating_add(2)..).unwrap_or_default();
        if let Some(end) = after.find('}') {
            let name = after.get(..end).unwrap_or_default();
            if !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                vars.push(name);
            }
            rest = after.get(end.saturating_add(1)..).unwrap_or_default();
        } else {
            break;
        }
    }
    vars
}

fn is_absolute_path(path: &str) -> bool {
    path.starts_with('/')
        || path.starts_with("~/")
        || path.as_bytes().get(1).is_some_and(|b| *b == b':')
}
