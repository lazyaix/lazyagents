//! Multi-target deterministic compiler orchestrating Google Antigravity (V1 active),
//! opt-in `.github/*` governance compilation, and reserved cross-agent target adapters.

mod antigravity;
mod github;
mod reserved;
mod shared;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::id::TargetKind;
use crate::lock::{ManagedRegionStyle, OwnershipMode};
use crate::manifest::ProjectManifest;
use crate::probe::ProjectFacts;

pub use self::antigravity::{AntigravityCompiler, normalize_antigravity_globs};
pub use self::github::GitHubGovernanceCompiler;
pub use self::reserved::compile_reserved_targets;

/// Severity of a cross-agent compilation compatibility diagnostic (`LA012`).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilitySeverity {
    /// Non-fatal degradation warning (output emitted on a best-effort basis or skipped).
    Warning,
    /// Unsupported feature or target that cannot be compiled yet.
    Unsupported,
}

/// Structured diagnostic emitted when a canonical IR feature cannot map losslessly to a target.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompatibilityDiagnostic {
    /// Target that emitted this compatibility notice.
    pub target: TargetKind,
    /// Severity level.
    pub severity: CompatibilitySeverity,
    /// Canonical resource or feature name (e.g., `"target:claude_code"` or `"mcp:legacy"`).
    pub subject: String,
    /// Concise explanation of what could not be mapped losslessly.
    pub message: String,
}

/// One deterministically compiled output file ready for diffing and atomic writing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedFile {
    /// Workspace-relative POSIX path (e.g., `"AGENTS.md"`, `".agents/hooks.json"`).
    pub path: String,
    /// Whole-file or fenced managed-region ownership mode.
    pub ownership: OwnershipMode,
    /// Normalized LF file body (or inner region body for `ManagedRegion`).
    pub content: String,
}

/// Pure Sans-I/O compilation input bundle (loaded from `.lazyagents/` by `la-runtime` or tests).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompileInputs<'a> {
    /// Probed repository facts used for `{{ project.* }}` rendering.
    pub facts: ProjectFacts,
    /// Source file contents keyed by `.lazyagents/`-relative POSIX path
    /// (e.g., `"sections/arch.md"`, `"rules/rust-style.md"`, `"skills/release-prep/SKILL.md"`).
    pub source_files: BTreeMap<String, String>,
    /// Optional explicit target filter (`None` compiles all enabled targets in the manifest).
    pub target_filter: Option<&'a [TargetKind]>,
}

/// Complete deterministic compilation output across all active targets and opt-in governance files.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompileOutput {
    /// Deterministically sorted output files (deduplicated by path).
    pub files: Vec<GeneratedFile>,
    /// Cross-agent compatibility diagnostics emitted during compilation.
    pub diagnostics: Vec<CompatibilityDiagnostic>,
}

impl GeneratedFile {
    /// Creates a whole-file managed output entry.
    #[must_use]
    pub fn whole_file(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            ownership: OwnershipMode::WholeFile,
            content: content.into(),
        }
    }

    /// Creates a fenced managed-region output entry.
    #[must_use]
    pub fn managed_region(
        path: impl Into<String>,
        style: ManagedRegionStyle,
        body: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            ownership: OwnershipMode::ManagedRegion { style },
            content: body.into(),
        }
    }
}

/// Compiles a resolved [`ProjectManifest`] and [`CompileInputs`] into deterministic target
/// artifacts and compatibility diagnostics.
///
/// # Errors
///
/// Returns [`CoreError::TemplateRender`] or [`CoreError::Compile`] if template rendering or
/// required source-file lookups fail.
pub fn compile_all(
    manifest: &ProjectManifest,
    inputs: &CompileInputs<'_>,
) -> Result<CompileOutput, CoreError> {
    manifest.validate_invariants()?;

    let mut file_map: BTreeMap<String, GeneratedFile> = BTreeMap::new();
    let mut diagnostics = Vec::new();

    let should_compile = |target: TargetKind| -> bool {
        inputs
            .target_filter
            .is_none_or(|filter| filter.contains(&target))
    };

    if manifest.targets.antigravity.enabled && should_compile(TargetKind::Antigravity) {
        let agy = AntigravityCompiler::compile(manifest, inputs)?;
        for file in agy.files {
            file_map.insert(file.path.clone(), file);
        }
        diagnostics.extend(agy.diagnostics);
    }

    let gh = GitHubGovernanceCompiler::compile(manifest, inputs, &file_map)?;
    for file in gh.files {
        file_map.insert(file.path.clone(), file);
    }
    diagnostics.extend(gh.diagnostics);

    diagnostics.extend(compile_reserved_targets(manifest, inputs.target_filter));

    Ok(CompileOutput {
        files: file_map.into_values().collect(),
        diagnostics,
    })
}
