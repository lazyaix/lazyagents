//! Structured compliance, empirical context-lean, agentic supply-chain security, budget, drift,
//! and GitHub Actions linter (`LA001`–`LA022`).

mod antigravity;
mod empirical;
mod github;
mod security;

use core::fmt::{self, Write as _};
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::budget::{BudgetReport, analyze_budgets_with_manifest};
use crate::compiler::CompileOutput;
use crate::diff::WritePlan;
use crate::error::CoreError;
use crate::manifest::{LintLevel, ProjectManifest};

pub use self::security::{
    LethalTrifectaReport, LethalTrifectaRisk, TrifectaSubjectReport, analyze_lethal_trifecta,
    effective_mcp_risk_surfaces,
};

/// Canonical `lazyagents` lint rule identifier (`LA001`–`LA022`).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum LintCode {
    /// Plaintext secret or stateless GitHub token detected in tracked files.
    #[serde(rename = "LA001")]
    La001,
    /// `AGENTS.md` or rule file exceeds Antigravity's per-file byte cap (24,000 bytes).
    #[serde(rename = "LA002")]
    La002,
    /// Aggregate always-on rules approach (>= 80%) or exceed the 20,000-token budget.
    #[serde(rename = "LA003")]
    La003,
    /// `SKILL.md` missing YAML frontmatter, invalid `name != dir`, or empty `description`.
    #[serde(rename = "LA004")]
    La004,
    /// Conditional rule trigger or Antigravity `globs` formatting issue.
    #[serde(rename = "LA005")]
    La005,
    /// Unsupported legacy HTTP+SSE MCP transport on Antigravity target.
    #[serde(rename = "LA006")]
    La006,
    /// Undeclared `${VAR}` placeholder or redundant stdio pass-through in Antigravity MCP config.
    #[serde(rename = "LA007")]
    La007,
    /// Relative `cwd` in plugin-bundled MCP server configuration.
    #[serde(rename = "LA008")]
    La008,
    /// Invalid hook `matcher` regex, zero timeout, or missing referenced custom script.
    #[serde(rename = "LA009")]
    La009,
    /// Plugin manifest `logo`, `suggested_prompts`, or misplaced `hooks/hooks.json` issue.
    #[serde(rename = "LA010")]
    La010,
    /// Name collision with Antigravity built-in subagents or duplicate MCP server names.
    #[serde(rename = "LA011")]
    La011,
    /// Non-lossless cross-agent target mapping diagnostic.
    #[serde(rename = "LA012")]
    La012,
    /// Generated files or managed blocks on disk drifted from deterministic compilation / `lock.json`.
    #[serde(rename = "LA013")]
    La013,
    /// Unpinned `uses:` action in `.github/workflows/*.yml` (must be 40-char commit SHA).
    #[serde(rename = "LA014")]
    La014,
    /// Missing top-level `permissions: {}` or `persist-credentials: false` on `actions/checkout`.
    #[serde(rename = "LA015")]
    La015,
    /// Unsafe `pull_request_target` checkout or untrusted `${{ github.event.* }}` in `run:`.
    #[serde(rename = "LA016")]
    La016,
    /// Governance mismatch between `[engineering]` and `.github/CODEOWNERS` or PR template.
    #[serde(rename = "LA017")]
    La017,
    /// Empirical `AGENTS.md` lean-context advisory (bloated root, static tree, or KV-cache busting).
    #[serde(rename = "LA018")]
    La018,
    /// Empirical `SKILL.md` quality advisory (oversized body, description cap/terseness, first-person voice).
    #[serde(rename = "LA019")]
    La019,
    /// OWASP ASI01/ASI04 hidden Unicode/BiDi/Tag smuggling or high-risk prompt-injection pattern.
    #[serde(rename = "LA020")]
    La020,
    /// OWASP ASI04 anti-Rug-Pull unpinned `npx`/`uvx` MCP package or non-SHA Git pack reference.
    #[serde(rename = "LA021")]
    La021,
    /// Lethal Trifecta high-risk combination or Eager MCP tool-schema context-tax advisory.
    #[serde(rename = "LA022")]
    La022,
}

/// Diagnostic severity level emitted in [`LintReport`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    /// Non-blocking informational note.
    Info,
    /// Warning (causes failure under `--strict`).
    Warning,
    /// Hard error (causes exit code `1` in `lazyagents check`).
    Error,
}

/// Source location associated with a lint diagnostic.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticLocation {
    /// Workspace-relative POSIX file path.
    pub path: String,
    /// Optional 1-based line number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// Optional 1-based byte column.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
}

/// One structured diagnostic finding produced by [`lint_project`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Rule identifier (`LA001`–`LA017`).
    pub code: LintCode,
    /// Effective severity after applying `[lint]` configuration.
    pub severity: DiagnosticSeverity,
    /// File and optional line/column where the issue occurred.
    pub location: DiagnosticLocation,
    /// Concise human-readable description of the violation.
    pub message: String,
    /// Actionable remediation hint.
    pub remediation: String,
}

/// Pure Sans-I/O input bundle for [`lint_project`].
#[derive(Clone, Debug)]
pub struct LintInputs<'a> {
    /// Raw manifest TOML text (scanned for plaintext secrets).
    pub manifest_toml: &'a str,
    /// Parsed & scope-resolved canonical manifest.
    pub manifest: &'a ProjectManifest,
    /// Source files under `.lazyagents/` keyed by `.lazyagents/`-relative POSIX path.
    pub source_files: &'a BTreeMap<String, String>,
    /// Compiled target outputs and compatibility diagnostics.
    pub compile_output: &'a CompileOutput,
    /// Optional write plan (if provided, drifted files emit `LA013`).
    pub write_plan: Option<&'a WritePlan>,
    /// Workspace files on disk keyed by repo-relative POSIX path (used for includes, `.github/workflows/*.yml`, `CODEOWNERS`, PR template).
    pub workspace_files: &'a BTreeMap<String, String>,
}

/// Complete compliance report combining diagnostics, double-tier budgets, and security radar.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LintReport {
    /// Ordered list of all emitted diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// Double-tier context budget and Push/Pull efficiency telemetry.
    pub budget: BudgetReport,
    /// Static Lethal Trifecta risk analysis across MCP servers and subagents.
    pub lethal_trifecta: LethalTrifectaReport,
}

impl LintCode {
    /// Returns the canonical string code (e.g., `"LA001"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::La001 => "LA001",
            Self::La002 => "LA002",
            Self::La003 => "LA003",
            Self::La004 => "LA004",
            Self::La005 => "LA005",
            Self::La006 => "LA006",
            Self::La007 => "LA007",
            Self::La008 => "LA008",
            Self::La009 => "LA009",
            Self::La010 => "LA010",
            Self::La011 => "LA011",
            Self::La012 => "LA012",
            Self::La013 => "LA013",
            Self::La014 => "LA014",
            Self::La015 => "LA015",
            Self::La016 => "LA016",
            Self::La017 => "LA017",
            Self::La018 => "LA018",
            Self::La019 => "LA019",
            Self::La020 => "LA020",
            Self::La021 => "LA021",
            Self::La022 => "LA022",
        }
    }
}

impl fmt::Display for LintCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl DiagnosticLocation {
    /// Creates a file-level diagnostic location with no line/column.
    #[must_use]
    pub fn file(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            line: None,
            column: None,
        }
    }

    /// Creates a line-level diagnostic location.
    #[must_use]
    pub fn line(path: impl Into<String>, line: usize) -> Self {
        Self {
            path: path.into(),
            line: Some(line),
            column: None,
        }
    }
}

impl LintReport {
    /// Returns `true` when the report should fail `lazyagents check` (exit code `1`).
    #[must_use]
    pub fn has_errors(&self, strict: bool) -> bool {
        self.diagnostics.iter().any(|diag| match diag.severity {
            DiagnosticSeverity::Error => true,
            DiagnosticSeverity::Warning => strict,
            DiagnosticSeverity::Info => false,
        })
    }

    /// Renders diagnostics as concise human-readable terminal text.
    #[must_use]
    pub fn format_text(&self) -> String {
        if self.diagnostics.is_empty() {
            return format!(
                "OK: 0 lint diagnostics (always-on budget: {}/{} tokens)\n",
                self.budget.always_on_tokens, self.budget.max_always_on_tokens
            );
        }
        let mut out = String::new();
        for diag in &self.diagnostics {
            let sev = match diag.severity {
                DiagnosticSeverity::Error => "error",
                DiagnosticSeverity::Warning => "warn",
                DiagnosticSeverity::Info => "info",
            };
            let loc = diag.location.line.map_or_else(
                || diag.location.path.clone(),
                |line| format!("{}:{line}", diag.location.path),
            );
            let _ = writeln!(out, "{sev}[{}] {loc}: {}", diag.code, diag.message);
            if !diag.remediation.is_empty() {
                let _ = writeln!(out, "  help: {}", diag.remediation);
            }
        }
        out
    }

    /// Renders diagnostics as GitHub Actions workflow command annotations (`--format github`).
    #[must_use]
    pub fn format_github_annotations(&self) -> String {
        let mut out = String::new();
        for diag in &self.diagnostics {
            let level = match diag.severity {
                DiagnosticSeverity::Error => "error",
                DiagnosticSeverity::Warning => "warning",
                DiagnosticSeverity::Info => "notice",
            };
            let mut props = format!("file={},title={}", diag.location.path, diag.code);
            if let Some(line) = diag.location.line {
                let _ = write!(props, ",line={line}");
            }
            if let Some(col) = diag.location.column {
                let _ = write!(props, ",col={col}");
            }
            let message = diag.message.replace('\n', "%0A");
            let _ = writeln!(out, "::{level} {props}::{message}");
        }
        out
    }

    /// Serializes the full report as pretty JSON (`--format json`).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Compile`] if JSON serialization fails.
    pub fn to_json_pretty(&self) -> Result<String, CoreError> {
        let mut json = serde_json::to_string_pretty(self).map_err(|err| CoreError::Compile {
            message: err.to_string(),
        })?;
        json.push('\n');
        Ok(json)
    }
}

/// Runs the full `LA001`–`LA022` compliance, empirical budget, agentic supply-chain security,
/// secret, drift, and GitHub Actions linter.
#[must_use]
pub fn lint_project(inputs: &LintInputs<'_>) -> LintReport {
    let budget = analyze_budgets_with_manifest(
        &inputs.compile_output.files,
        inputs.workspace_files,
        inputs.manifest,
    );
    let lethal_trifecta = analyze_lethal_trifecta(inputs.manifest);
    let mut diagnostics = Vec::new();

    antigravity::run_antigravity_checks(inputs, &budget, &mut diagnostics);
    github::run_github_checks(inputs, &mut diagnostics);
    empirical::run_empirical_checks(inputs, &budget, &mut diagnostics);
    security::run_security_checks(inputs, &budget, &lethal_trifecta, &mut diagnostics);

    LintReport {
        diagnostics,
        budget,
        lethal_trifecta,
    }
}

pub(super) const fn level_to_severity(level: LintLevel) -> Option<DiagnosticSeverity> {
    match level {
        LintLevel::Error => Some(DiagnosticSeverity::Error),
        LintLevel::Warn => Some(DiagnosticSeverity::Warning),
        LintLevel::Off => None,
    }
}
