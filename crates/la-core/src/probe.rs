//! Pure Sans-I/O project introspection facts fed into dynamic `AGENTS.md` templates
//! and target compilers.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Primary programming language detected in a repository.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectedLanguage {
    /// Rust (`Cargo.toml`).
    Rust,
    /// TypeScript / JavaScript (`package.json`, `tsconfig.json`).
    TypeScript,
    /// Python (`pyproject.toml`).
    Python,
    /// Go (`go.mod`).
    Go,
}

/// Probed repository command hints discovered from build manifests or task runners.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CommandHints {
    /// Formatter check command (e.g., `cargo fmt --all -- --check`).
    pub fmt: Option<String>,
    /// Static linter command (e.g., `cargo clippy --workspace --all-targets -- -D warnings`).
    pub lint: Option<String>,
    /// Test suite command (e.g., `cargo test --workspace --all-features`).
    pub test: Option<String>,
    /// Documentation build command.
    pub doc: Option<String>,
    /// Build / compile command.
    pub build: Option<String>,
}

/// Summary of one GitHub Actions workflow discovered under `.github/workflows/`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CiWorkflowFact {
    /// Workflow file name (e.g., `ci.yml`).
    pub file: String,
    /// Top-level workflow `name:` field if present.
    pub name: Option<String>,
    /// Job display names discovered in the workflow.
    #[serde(default)]
    pub jobs: Vec<String>,
}

/// Read-only repository metadata facts produced by `la-runtime::prober` (or fixtures in tests)
/// and exposed as `{{ project.* }}` during deterministic template rendering.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectFacts {
    /// Repository or workspace name.
    pub name: String,
    /// Detected languages ordered by prevalence.
    pub languages: Vec<DetectedLanguage>,
    /// Workspace member crate or package paths.
    pub workspace_members: Vec<String>,
    /// Detected Rust toolchain or MSRV string (e.g., `"1.100.0"`).
    pub toolchain_version: Option<String>,
    /// Detected Rust edition or language standard (e.g., `"2024"`).
    pub language_edition: Option<String>,
    /// Detected Git remote repository slug (`owner/name`) if available.
    pub git_remote_repo: Option<String>,
    /// Detected Git default branch name.
    pub git_default_branch: Option<String>,
    /// Discovered GitHub Actions workflows.
    pub ci_workflows: Vec<CiWorkflowFact>,
    /// Probed verification and build command hints.
    pub commands: CommandHints,
    /// Flattened boolean convenience flags exposed to templates (`has_rust`, `has_github_actions`, etc.).
    #[serde(flatten)]
    pub flags: BTreeMap<String, bool>,
}

impl ProjectFacts {
    /// Recomputes template convenience flags (`has_rust`, `has_typescript`, `has_python`,
    /// `has_go`, and `has_github_actions`) from `languages` and `ci_workflows`.
    pub fn refresh_flags(&mut self) {
        self.flags.insert(
            "has_rust".to_owned(),
            self.has_language(DetectedLanguage::Rust),
        );
        self.flags.insert(
            "has_typescript".to_owned(),
            self.has_language(DetectedLanguage::TypeScript),
        );
        self.flags.insert(
            "has_python".to_owned(),
            self.has_language(DetectedLanguage::Python),
        );
        self.flags
            .insert("has_go".to_owned(), self.has_language(DetectedLanguage::Go));
        self.flags
            .insert("has_github_actions".to_owned(), self.has_github_actions());
    }

    /// Returns `true` when `lang` was detected in this repository.
    #[must_use]
    pub fn has_language(&self, lang: DetectedLanguage) -> bool {
        self.languages.contains(&lang)
    }

    /// Returns `true` when at least one GitHub Actions workflow was discovered.
    #[must_use]
    pub const fn has_github_actions(&self) -> bool {
        !self.ci_workflows.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_flags_sets_language_and_ci_indicators() {
        let mut facts = ProjectFacts {
            name: "demo".to_owned(),
            languages: vec![DetectedLanguage::Rust, DetectedLanguage::TypeScript],
            ci_workflows: vec![CiWorkflowFact {
                file: "ci.yml".to_owned(),
                name: Some("CI".to_owned()),
                jobs: vec!["Test".to_owned()],
            }],
            ..ProjectFacts::default()
        };
        facts.refresh_flags();
        assert_eq!(facts.flags.get("has_rust"), Some(&true));
        assert_eq!(facts.flags.get("has_typescript"), Some(&true));
        assert_eq!(facts.flags.get("has_python"), Some(&false));
        assert_eq!(facts.flags.get("has_go"), Some(&false));
        assert_eq!(facts.flags.get("has_github_actions"), Some(&true));
    }
}
