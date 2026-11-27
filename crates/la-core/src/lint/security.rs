//! Agentic supply-chain security (`LA020`, `LA021`), Lethal Trifecta risk analysis, and Eager
//! MCP context-tax diagnostics (`LA022`) grounded in the OWASP Top 10 for Agentic Applications (2026).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::budget::BudgetReport;
use crate::id::ToolCapability;
use crate::lint::{
    Diagnostic, DiagnosticLocation, DiagnosticSeverity, LintCode, LintInputs, level_to_severity,
};
use crate::manifest::{
    McpRiskSurface, McpServerSpec, McpToolLoading, McpTransport, OrgPolicyRef, PackSource,
    ProjectManifest, SubagentSpec,
};

/// Minimum number of eager MCP servers before `LA022` advises switching to `deferred`/`code_mode`.
const EAGER_MCP_TAX_WARN_THRESHOLD: usize = 4;

/// Static Lethal Trifecta risk classification (Simon Willison / OWASP ASI01 & ASI02).
#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum LethalTrifectaRisk {
    /// Zero or one risk surface present.
    #[default]
    Low,
    /// Two risk surfaces present (missing the third exfiltration/ingestion leg).
    Moderate,
    /// All three surfaces (`untrusted_input`, `private_data`, `external_egress`) present simultaneously.
    High,
}

/// Lethal Trifecta evaluation for one named scope (project-wide MCP surface or a specific subagent).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrifectaSubjectReport {
    /// Subject identifier (e.g., `"project:mcp"` or `"agent:research-helper"`).
    pub subject: String,
    /// Distinct risk surfaces active on this subject.
    pub surfaces: Vec<McpRiskSurface>,
    /// Derived risk tier (`Low`, `Moderate`, or `High`).
    pub risk: LethalTrifectaRisk,
}

/// Aggregate Lethal Trifecta static risk report included in [`crate::LintReport`] and [`crate::AgBomReport`].
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct LethalTrifectaReport {
    /// Highest risk tier across all evaluated project and subagent subjects.
    pub overall_risk: LethalTrifectaRisk,
    /// Distinct risk surfaces observed across the project's enabled MCP servers and agents.
    pub surfaces: Vec<McpRiskSurface>,
    /// Per-subject breakdowns (project MCP layer + enabled custom subagents).
    pub subjects: Vec<TrifectaSubjectReport>,
}

/// Computes the deterministic static Lethal Trifecta report for a [`ProjectManifest`].
#[must_use]
pub fn analyze_lethal_trifecta(manifest: &ProjectManifest) -> LethalTrifectaReport {
    let mut subjects = Vec::new();
    let mut overall_surfaces = BTreeSet::new();

    let mut project_mcp_surfaces = BTreeSet::new();
    for server in &manifest.mcp_servers {
        if !server.enabled {
            continue;
        }
        for surface in effective_mcp_risk_surfaces(server) {
            project_mcp_surfaces.insert(surface);
            overall_surfaces.insert(surface);
        }
    }
    if !project_mcp_surfaces.is_empty() {
        let risk = classify_surfaces(&project_mcp_surfaces);
        subjects.push(TrifectaSubjectReport {
            subject: "project:mcp".to_owned(),
            surfaces: project_mcp_surfaces.into_iter().collect(),
            risk,
        });
    }

    for agent in &manifest.agents {
        if !agent.enabled {
            continue;
        }
        let agent_surfaces = effective_subagent_risk_surfaces(agent, &manifest.mcp_servers);
        for &surface in &agent_surfaces {
            overall_surfaces.insert(surface);
        }
        let risk = classify_surfaces(&agent_surfaces);
        subjects.push(TrifectaSubjectReport {
            subject: format!("agent:{}", agent.name),
            surfaces: agent_surfaces.into_iter().collect(),
            risk,
        });
    }

    let overall_risk = subjects
        .iter()
        .map(|s| s.risk)
        .max()
        .unwrap_or(LethalTrifectaRisk::Low);

    LethalTrifectaReport {
        overall_risk,
        surfaces: overall_surfaces.into_iter().collect(),
        subjects,
    }
}

/// Returns the effective [`McpRiskSurface`] set for an MCP server (explicit declarations take
/// precedence; when omitted, conservative built-in heuristics infer likely surfaces).
#[must_use]
pub fn effective_mcp_risk_surfaces(server: &McpServerSpec) -> Vec<McpRiskSurface> {
    if !server.risk_surfaces.is_empty() {
        let unique: BTreeSet<McpRiskSurface> = server.risk_surfaces.iter().copied().collect();
        return unique.into_iter().collect();
    }

    let mut surfaces = BTreeSet::new();
    let mut haystack = server.name.as_str().to_ascii_lowercase();
    match &server.transport {
        McpTransport::StreamableHttp { url, .. } | McpTransport::LegacySse { url, .. } => {
            haystack.push(' ');
            haystack.push_str(&url.to_ascii_lowercase());
        }
        McpTransport::Stdio { command, args, .. } => {
            haystack.push(' ');
            haystack.push_str(&command.to_ascii_lowercase());
            for arg in args {
                haystack.push(' ');
                haystack.push_str(&arg.to_ascii_lowercase());
            }
        }
    }

    let untrusted_hints = [
        "web",
        "fetch",
        "browser",
        "search",
        "playwright",
        "puppeteer",
        "github",
        "gmail",
        "email",
        "slack",
        "jira",
        "linear",
        "zendesk",
        "notion",
    ];
    let private_hints = [
        "filesystem",
        "file",
        "postgres",
        "sqlite",
        "mysql",
        "mongo",
        "redis",
        "db",
        "database",
        "github",
        "vault",
        "aws",
        "gcp",
        "cloudflare",
        "secret",
    ];
    let egress_hints = [
        "github",
        "slack",
        "gmail",
        "email",
        "webhook",
        "http",
        "cloudflare",
        "linear",
        "jira",
        "notion",
        "browser",
    ];

    if untrusted_hints.iter().any(|hint| haystack.contains(hint)) {
        surfaces.insert(McpRiskSurface::UntrustedInput);
    }
    if private_hints.iter().any(|hint| haystack.contains(hint)) {
        surfaces.insert(McpRiskSurface::PrivateData);
    }
    if egress_hints.iter().any(|hint| haystack.contains(hint)) {
        surfaces.insert(McpRiskSurface::ExternalEgress);
    }

    surfaces.into_iter().collect()
}

pub(super) fn run_security_checks(
    inputs: &LintInputs<'_>,
    budget: &BudgetReport,
    trifecta: &LethalTrifectaReport,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_prompt_injection_la020(inputs, diagnostics);
    check_supply_chain_pinning_la021(inputs, diagnostics);
    check_risk_and_context_tax_la022(inputs, budget, trifecta, diagnostics);
}

fn check_prompt_injection_la020(inputs: &LintInputs<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let Some(severity) = level_to_severity(inputs.manifest.lint.prompt_injection) else {
        return;
    };

    scan_text_for_injection(
        ".lazyagents/lazyagents.toml",
        inputs.manifest_toml,
        severity,
        diagnostics,
    );
    for (rel, content) in inputs.source_files {
        scan_text_for_injection(
            &format!(".lazyagents/{rel}"),
            content,
            severity,
            diagnostics,
        );
    }
    for file in &inputs.compile_output.files {
        if is_agent_text_asset(&file.path) {
            scan_text_for_injection(&file.path, &file.content, severity, diagnostics);
        }
    }
}

fn scan_text_for_injection(
    path: &str,
    content: &str,
    severity: DiagnosticSeverity,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut reported_unicode = false;
    let mut reported_phrase = false;
    let mut reported_pipe = false;

    for (idx, line) in content.lines().enumerate() {
        let line_no = idx.saturating_add(1);

        if !reported_unicode
            && let Some((col, codepoint, kind)) = detect_suspicious_unicode(line, idx == 0)
        {
            reported_unicode = true;
            diagnostics.push(Diagnostic {
                code: LintCode::La020,
                severity,
                location: DiagnosticLocation {
                    path: path.to_owned(),
                    line: Some(line_no),
                    column: Some(col),
                },
                message: format!(
                    "detected hidden {kind} character (U+{codepoint:04X}) in agent configuration (OWASP ASI04 Unicode smuggling vector)"
                ),
                remediation: "Remove invisible BiDi, zero-width, or Unicode Tag characters and audit the file provenance before trusting it.".to_owned(),
            });
        }

        let lower = line.to_ascii_lowercase();
        if !reported_phrase && contains_hijack_phrase(&lower) {
            reported_phrase = true;
            diagnostics.push(Diagnostic {
                code: LintCode::La020,
                severity,
                location: DiagnosticLocation::line(path, line_no),
                message: "detected high-risk prompt-injection override directive inside tracked agent configuration (OWASP ASI01/ASI04)".to_owned(),
                remediation: "Audit and remove instructions attempting to override system safety boundaries or silently exfiltrate data.".to_owned(),
            });
        }

        if !reported_pipe && contains_suspicious_pipe_or_exfil(&lower) {
            reported_pipe = true;
            diagnostics.push(Diagnostic {
                code: LintCode::La020,
                severity,
                location: DiagnosticLocation::line(path, line_no),
                message: "detected unverified remote shell pipe or sensitive credential exfiltration pattern in agent instructions/skills (OWASP ASI04)".to_owned(),
                remediation: "Replace remote `curl|sh` pipes with pinned local scripts or audited packages, and never instruct agents to transmit `~/.ssh` or `~/.aws` secrets.".to_owned(),
            });
        }
    }
}

fn check_supply_chain_pinning_la021(inputs: &LintInputs<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let Some(severity) = level_to_severity(inputs.manifest.lint.supply_chain_pinning) else {
        return;
    };

    for server in &inputs.manifest.mcp_servers {
        if !server.enabled {
            continue;
        }
        if let McpTransport::Stdio { command, args, .. } = &server.transport
            && let Some(unpinned_pkg) = find_unpinned_stdio_package(command, args)
        {
            diagnostics.push(Diagnostic {
                code: LintCode::La021,
                severity,
                location: DiagnosticLocation::file(".lazyagents/lazyagents.toml"),
                message: format!(
                    "MCP server `{}` runs unpinned package `{unpinned_pkg}` via `{command}` (OWASP ASI04 Rug Pull risk)",
                    server.name
                ),
                remediation: format!(
                    "Pin `{unpinned_pkg}` to an exact version (e.g., `{unpinned_pkg}@1.2.3` or `==1.2.3`) so upstream updates cannot silently poison tool descriptions."
                ),
            });
        }
    }

    for pack in &inputs.manifest.packs {
        if let PackSource::Git { url, rev } = &pack.source
            && !rev.as_deref().is_some_and(is_full_commit_sha)
        {
            diagnostics.push(Diagnostic {
                code: LintCode::La021,
                severity,
                location: DiagnosticLocation::file(".lazyagents/lazyagents.toml"),
                message: format!(
                    "Git pack `{}` (`{url}`) is not pinned to a full 40-character commit SHA",
                    pack.id
                ),
                remediation: "Set `rev` to a full 40-character hex commit SHA before running `lazyagents pack fetch`.".to_owned(),
            });
        }
    }

    if let Some(OrgPolicyRef::Git { url, rev }) = &inputs.manifest.scopes.org_policy
        && !rev.as_deref().is_some_and(is_full_commit_sha)
    {
        diagnostics.push(Diagnostic {
            code: LintCode::La021,
            severity,
            location: DiagnosticLocation::file(".lazyagents/lazyagents.toml"),
            message: format!(
                "organization policy Git source `{url}` is not pinned to a full 40-character commit SHA"
            ),
            remediation: "Pin `scopes.org_policy.rev` to an immutable 40-character hex commit SHA.".to_owned(),
        });
    }
}

fn check_risk_and_context_tax_la022(
    inputs: &LintInputs<'_>,
    budget: &BudgetReport,
    trifecta: &LethalTrifectaReport,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(severity) = level_to_severity(inputs.manifest.lint.risk_and_context_tax) else {
        return;
    };

    for subject in &trifecta.subjects {
        if subject.risk == LethalTrifectaRisk::High {
            diagnostics.push(Diagnostic {
                code: LintCode::La022,
                severity,
                location: DiagnosticLocation::file(".lazyagents/lazyagents.toml"),
                message: format!(
                    "Lethal Trifecta detected on `{}`: combines untrusted_input, private_data, and external_egress simultaneously",
                    subject.subject
                ),
                remediation: "Break the trifecta by splitting read-only ingestion from privileged execution, restricting `allowed_tools`, or enabling `[engineering.shell_guard]`.".to_owned(),
            });
        }
    }

    if budget.push_pull.eager_mcp_server_count >= EAGER_MCP_TAX_WARN_THRESHOLD {
        diagnostics.push(Diagnostic {
            code: LintCode::La022,
            severity,
            location: DiagnosticLocation::file(".lazyagents/lazyagents.toml"),
            message: format!(
                "{} eager MCP servers add ~{} estimated startup tool-schema tokens",
                budget.push_pull.eager_mcp_server_count,
                budget.push_pull.eager_mcp_estimated_tokens
            ),
            remediation: "Set `tool_loading = \"deferred\"` (Tool Search) or `\"code_mode\"` on secondary MCP servers, or prefer native CLI + `SKILL.md`.".to_owned(),
        });
    }

    let has_gh_cli_skills = inputs.manifest.engineering.github.enabled
        && !inputs.manifest.engineering.github.enable_skills.is_empty();
    let has_eager_github_mcp = inputs.manifest.mcp_servers.iter().any(|s| {
        s.enabled
            && s.tool_loading == McpToolLoading::Eager
            && s.name.as_str().eq_ignore_ascii_case("github")
    });
    if has_gh_cli_skills && has_eager_github_mcp {
        diagnostics.push(Diagnostic {
            code: LintCode::La022,
            severity,
            location: DiagnosticLocation::file(".lazyagents/lazyagents.toml"),
            message: "eager `github` MCP server duplicates capabilities already covered by enabled `gh` CLI GitHub skills".to_owned(),
            remediation: "Set `tool_loading = \"deferred\"` on the `github` MCP server or rely on `gh` CLI skills to save ~1,500 startup schema tokens.".to_owned(),
        });
    }
}

fn effective_subagent_risk_surfaces(
    agent: &SubagentSpec,
    mcp_servers: &[McpServerSpec],
) -> BTreeSet<McpRiskSurface> {
    let mut surfaces = BTreeSet::new();
    for cap in &agent.capabilities {
        match cap {
            ToolCapability::WebSearch | ToolCapability::Browser => {
                surfaces.insert(McpRiskSurface::UntrustedInput);
                surfaces.insert(McpRiskSurface::ExternalEgress);
            }
            ToolCapability::FileRead | ToolCapability::Search => {
                surfaces.insert(McpRiskSurface::PrivateData);
            }
            ToolCapability::Shell => {
                surfaces.insert(McpRiskSurface::PrivateData);
                surfaces.insert(McpRiskSurface::ExternalEgress);
            }
            ToolCapability::FileWrite | ToolCapability::Subagent | ToolCapability::Custom(_) => {}
        }
    }

    for mcp_id in &agent.mcp_servers {
        if let Some(server) = mcp_servers.iter().find(|s| s.name == *mcp_id && s.enabled) {
            for surface in effective_mcp_risk_surfaces(server) {
                surfaces.insert(surface);
            }
        }
    }

    surfaces
}

fn classify_surfaces(surfaces: &BTreeSet<McpRiskSurface>) -> LethalTrifectaRisk {
    match surfaces.len() {
        0 | 1 => LethalTrifectaRisk::Low,
        2 => LethalTrifectaRisk::Moderate,
        _ => LethalTrifectaRisk::High,
    }
}

fn is_agent_text_asset(path: &str) -> bool {
    path == "AGENTS.md"
        || path == "GEMINI.md"
        || path == "CLAUDE.md"
        || path.ends_with("/AGENTS.md")
        || path.ends_with("/GEMINI.md")
        || path.starts_with(".agents/")
}

fn detect_suspicious_unicode(
    line: &str,
    is_first_line: bool,
) -> Option<(usize, u32, &'static str)> {
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    for (pos, &(byte_idx, ch)) in chars.iter().enumerate() {
        let cp = u32::from(ch);
        let col = byte_idx.saturating_add(1);
        if matches!(
            cp,
            0x202A..=0x202E | 0x2066..=0x2069 | 0x061C | 0x200E | 0x200F
        ) {
            return Some((col, cp, "BiDi control"));
        }
        if matches!(cp, 0xE0000..=0xE007F) {
            return Some((col, cp, "Unicode Tag smuggling"));
        }
        if matches!(cp, 0x200B | 0x200C | 0x2060) || (cp == 0xFEFF && !(is_first_line && pos == 0))
        {
            return Some((col, cp, "zero-width invisible"));
        }
        if cp == 0x200D {
            let prev_ascii = pos
                .checked_sub(1)
                .and_then(|i| chars.get(i))
                .is_some_and(|(_, c)| c.is_ascii());
            let next_ascii = chars
                .get(pos.saturating_add(1))
                .is_some_and(|(_, c)| c.is_ascii());
            if prev_ascii || next_ascii {
                return Some((col, cp, "zero-width joiner in ASCII"));
            }
        }
    }
    None
}

fn contains_hijack_phrase(lower: &str) -> bool {
    let phrases = [
        "ignore all previous instructions",
        "ignore previous instructions",
        "disregard all previous instructions",
        "override system prompt",
        "bypass all safety checks",
    ];
    phrases.iter().any(|phrase| lower.contains(phrase))
}

fn contains_suspicious_pipe_or_exfil(lower: &str) -> bool {
    let trimmed = lower.trim_start();
    let negation_prefixes = ["never ", "do not ", "don't ", "avoid ", "forbid ", "block "];
    if negation_prefixes
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
    {
        return false;
    }

    let has_remote_fetch = (lower.contains("curl ") || lower.contains("wget "))
        && (lower.contains("http://") || lower.contains("https://"));
    let pipes_to_shell = lower.contains("| sh")
        || lower.contains("| bash")
        || lower.contains("|sh")
        || lower.contains("|bash");
    if has_remote_fetch && pipes_to_shell {
        return true;
    }

    let reads_sensitive = lower.contains("~/.ssh/id_")
        || lower.contains("$home/.ssh/id_")
        || lower.contains("~/.aws/credentials");
    let exfil_tool = lower.contains("curl ") || lower.contains("wget ") || lower.contains("nc ");
    reads_sensitive && exfil_tool
}

fn find_unpinned_stdio_package<'a>(command: &str, args: &'a [String]) -> Option<&'a str> {
    let bin = command.rsplit('/').next().unwrap_or(command);
    match bin {
        "npx" => {
            let pkg = first_positional_arg(args, &["-p", "--package"])?;
            if is_pinned_npm_spec(pkg) {
                None
            } else {
                Some(pkg)
            }
        }
        "uvx" | "pipx" => {
            let start = if bin == "pipx" && args.first().map(String::as_str) == Some("run") {
                args.get(1..).unwrap_or_default()
            } else {
                args
            };
            let pkg = first_positional_arg(start, &["--from", "--python", "--with"])?;
            if is_pinned_python_spec(pkg) {
                None
            } else {
                Some(pkg)
            }
        }
        _ => None,
    }
}

fn first_positional_arg<'a>(args: &'a [String], value_flags: &[&str]) -> Option<&'a str> {
    let mut skip_next = false;
    for arg in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if value_flags.contains(&arg.as_str()) {
            skip_next = true;
            continue;
        }
        if arg.starts_with('-') {
            continue;
        }
        return Some(arg.as_str());
    }
    None
}

fn is_pinned_npm_spec(spec: &str) -> bool {
    let after_scope = spec.strip_prefix('@').unwrap_or(spec);
    let Some((_, version)) = after_scope.split_once('@') else {
        return false;
    };
    !version.is_empty()
        && !version.eq_ignore_ascii_case("latest")
        && !version.eq_ignore_ascii_case("next")
        && version.chars().next().is_some_and(|ch| ch.is_ascii_digit())
}

fn is_pinned_python_spec(spec: &str) -> bool {
    if let Some((_, version)) = spec.split_once("==") {
        return !version.is_empty() && version.chars().next().is_some_and(|ch| ch.is_ascii_digit());
    }
    if let Some((_, version)) = spec.split_once('@') {
        return !version.is_empty()
            && !version.eq_ignore_ascii_case("latest")
            && version.chars().next().is_some_and(|ch| ch.is_ascii_digit());
    }
    false
}

fn is_full_commit_sha(rev: &str) -> bool {
    rev.len() == 40 && rev.bytes().all(|b| b.is_ascii_hexdigit())
}
