//! Verification gates, supply-chain security baselines, architectural layer boundaries,
//! and release automation policies under `[engineering]`.

use serde::{Deserialize, Serialize};

use crate::id::ResourceId;

/// Behavior of the built-in `verify-gate` `Stop` hook when tracked files changed.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StopHookMode {
    /// Do not emit a `verify-gate` `Stop` hook.
    Off,
    /// Inject a concise reminder listing required verification commands once per turn.
    #[default]
    Remind,
    /// Ask the agent to continue until verification commands have been run.
    Enforce,
}

/// Single verification gate command declared under `[engineering.verify]`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VerifyCommand {
    /// Stable command identifier (e.g., `fmt`, `lint`, `test`, `doc`).
    pub id: ResourceId,
    /// Human-readable check name (displayed in PR templates and TUI).
    pub label: String,
    /// Exact shell command to run locally before pushing.
    pub cmd: String,
    /// Whether this check is mandatory before opening or updating a PR.
    #[serde(default = "default_true")]
    pub required: bool,
}

/// Declarative `[engineering.verify]` local quality-gate contract.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VerifyPolicy {
    /// Behavior of the built-in `verify-gate` Antigravity `Stop` hook.
    pub stop_hook_mode: StopHookMode,
    /// Ordered local verification commands compiled into `AGENTS.md` and PR templates.
    pub commands: Vec<VerifyCommand>,
}

/// Declarative `[engineering.supply_chain]` dependency and token hygiene baseline.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SupplyChainPolicy {
    /// SPDX license identifiers allowed in the dependency graph.
    pub allowed_licenses: Vec<String>,
    /// Explicitly ban Copyleft licenses (`MPL`/`LGPL`/`GPL`/`AGPL`).
    pub forbid_copyleft: bool,
    /// Minimum publish cooldown window (in days) for dependency and action updates.
    pub publish_cooldown_days: u16,
    /// Enforce variable-length opaque handling and redaction for stateless `ghs_*` GitHub tokens.
    pub stateless_github_tokens: bool,
}

/// Single architectural module or crate boundary compiled into a `glob`-triggered rule.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureLayer {
    /// Logical layer or crate name (e.g., `la-core`).
    pub name: ResourceId,
    /// Workspace-relative glob pattern covered by this boundary (e.g., `crates/la-core/**`).
    pub glob: String,
    /// Concise architectural invariant enforced for files matching `glob`.
    pub rule: String,
}

/// Declarative `[engineering.architecture]` design references and layer boundaries.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ArchitecturePolicy {
    /// Repository-relative paths to authoritative architecture/spec documents.
    pub docs: Vec<String>,
    /// Path-scoped layer boundaries compiled into `AGENTS.md` and `.agents/rules/*.md`.
    pub layers: Vec<ArchitectureLayer>,
}

/// Release automation engine declared under `[engineering.release]`.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReleaseTool {
    /// Manual git tags and release notes.
    #[default]
    Manual,
    /// Rust workspace releases via `release-plz`.
    ReleasePlz,
    /// JavaScript/TypeScript monorepo releases via Changesets.
    Changesets,
}

/// Declarative `[engineering.release]` versioning and release contract.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReleasePolicy {
    /// Automated or manual release driver.
    pub tool: ReleaseTool,
    /// Git tag format template (e.g., `v{{ version }}`).
    pub tag_pattern: String,
    /// Expect GitHub Immutable Releases for published tags and artifacts.
    pub immutable_releases: bool,
}

impl Default for SupplyChainPolicy {
    fn default() -> Self {
        Self {
            allowed_licenses: vec![
                "Apache-2.0".to_owned(),
                "MIT".to_owned(),
                "BSD-2-Clause".to_owned(),
                "BSD-3-Clause".to_owned(),
                "ISC".to_owned(),
                "Zlib".to_owned(),
                "0BSD".to_owned(),
                "CC0-1.0".to_owned(),
            ],
            forbid_copyleft: true,
            publish_cooldown_days: 7,
            stateless_github_tokens: true,
        }
    }
}

impl Default for ReleasePolicy {
    fn default() -> Self {
        Self {
            tool: ReleaseTool::Manual,
            tag_pattern: "v{{ version }}".to_owned(),
            immutable_releases: true,
        }
    }
}

const fn default_true() -> bool {
    true
}
