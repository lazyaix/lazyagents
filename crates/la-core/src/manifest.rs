//! Canonical `.lazyagents/lazyagents.toml` manifest model, TOML parser, serializer,
//! and structural validation.

mod resources;
mod settings;

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::engineering::EngineeringPolicy;
use crate::error::CoreError;

pub use self::resources::{
    AntigravityAgentExt, DEFAULT_HOOK_TIMEOUT_SECS, HookMatcher, HookSpec, IndexEntry,
    IndexManifestSpec, IndexManifests, InstructionSection, InstructionSpec, McpServerSpec,
    McpTransport, McpVariable, ModelTier, PackRef, PackSource, PluginSpec, RuleSpec, RuleTrigger,
    SkillSpec, SubagentSpec, SubagentTargetExt,
};
pub use self::settings::{
    AntigravityTargetConfig, DEFAULT_ALWAYS_ON_MAX_TOKENS, DEFAULT_RULE_MAX_BYTES, GitOutputMode,
    InstructionFileKind, LintConfig, LintLevel, OrgPolicyRef, ProjectMeta, ReservedTargetConfig,
    ScopeSettings, TargetConfigs, TemplateLang,
};

/// Current canonical manifest schema version supported by `la-core`.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Complete canonical project manifest parsed from `.lazyagents/lazyagents.toml`
/// (or a layered scope override such as `.lazyagents/local.toml`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectManifest {
    /// Manifest format version (must equal [`CURRENT_SCHEMA_VERSION`]).
    pub schema_version: u32,
    /// Project identity, Git output strategy, and default template language.
    pub project: ProjectMeta,
    /// Scope inheritance hooks (`User` and `Organization` reserved).
    pub scopes: ScopeSettings,
    /// Enabled compilation targets (`Antigravity` active in V1; others reserved).
    pub targets: TargetConfigs,
    /// Multi-surface engineering governance policy (`[engineering]`).
    pub engineering: EngineeringPolicy,
    /// Reusable pack references (`[[packs]]`).
    pub packs: Vec<PackRef>,
    /// Dynamic `AGENTS.md` composition plans (`[[instructions]]`).
    pub instructions: Vec<InstructionSpec>,
    /// Conditional modular rules (`[[rules]]`).
    pub rules: Vec<RuleSpec>,
    /// Skill bundles (`[[skills]]`).
    pub skills: Vec<SkillSpec>,
    /// Declarative Antigravity JSON index manifests (`[manifests]`).
    pub manifests: IndexManifests,
    /// Custom subagents (`[[agents]]`).
    pub agents: Vec<SubagentSpec>,
    /// Lifecycle hooks (`[[hooks]]`).
    pub hooks: Vec<HookSpec>,
    /// MCP servers (`[[mcp_servers]]`).
    pub mcp_servers: Vec<McpServerSpec>,
    /// Cohesive project-level Antigravity plugins (`[[plugins]]`).
    pub plugins: Vec<PluginSpec>,
    /// Compliance linter thresholds and rule severities (`[lint]`).
    pub lint: LintConfig,
}

impl Default for ProjectManifest {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            project: ProjectMeta::default(),
            scopes: ScopeSettings::default(),
            targets: TargetConfigs::default(),
            engineering: EngineeringPolicy::default(),
            packs: Vec::new(),
            instructions: Vec::new(),
            rules: Vec::new(),
            skills: Vec::new(),
            manifests: IndexManifests::default(),
            agents: Vec::new(),
            hooks: Vec::new(),
            mcp_servers: Vec::new(),
            plugins: Vec::new(),
            lint: LintConfig::default(),
        }
    }
}

impl ProjectManifest {
    /// Parses a TOML string into a validated [`ProjectManifest`].
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::ManifestParse`] on invalid TOML or field types,
    /// [`CoreError::UnsupportedSchemaVersion`] if `schema_version != 1`, or
    /// [`CoreError::InvalidManifest`] when structural invariants fail.
    pub fn from_toml_str(input: &str) -> Result<Self, CoreError> {
        let manifest: Self = toml::from_str(input).map_err(|err| CoreError::ManifestParse {
            message: err.to_string(),
        })?;
        manifest.validate_invariants()?;
        Ok(manifest)
    }

    /// Serializes this manifest to a deterministically formatted TOML string.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::ManifestSerialize`] if TOML encoding fails.
    pub fn to_toml_pretty(&self) -> Result<String, CoreError> {
        toml::to_string_pretty(self).map_err(|err| CoreError::ManifestSerialize {
            message: err.to_string(),
        })
    }

    /// Validates core structural invariants that cannot be enforced by individual field types alone.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::UnsupportedSchemaVersion`] or [`CoreError::InvalidManifest`].
    pub fn validate_invariants(&self) -> Result<(), CoreError> {
        if self.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(CoreError::UnsupportedSchemaVersion {
                found: self.schema_version,
                expected: CURRENT_SCHEMA_VERSION,
            });
        }
        if self.project.name.trim().is_empty() {
            return Err(CoreError::InvalidManifest {
                message: "project.name must not be blank".to_owned(),
            });
        }
        check_unique(self.packs.iter().map(|p| p.id.as_str()), "pack")?;
        check_unique(self.rules.iter().map(|r| r.id.as_str()), "rule")?;
        check_unique(self.skills.iter().map(|s| s.name.as_str()), "skill")?;
        check_unique(self.agents.iter().map(|a| a.name.as_str()), "agent")?;
        check_unique(self.hooks.iter().map(|h| h.name.as_str()), "hook")?;
        check_unique(
            self.mcp_servers.iter().map(|m| m.name.as_str()),
            "mcp_server",
        )?;
        check_unique(self.plugins.iter().map(|p| p.name.as_str()), "plugin")?;

        self.validate_hooks_and_instructions()
    }

    fn validate_hooks_and_instructions(&self) -> Result<(), CoreError> {
        let mut seen_outputs = BTreeSet::new();
        for inst in &self.instructions {
            if inst.output.trim().is_empty() {
                return Err(CoreError::InvalidManifest {
                    message: "instruction output path must not be blank".to_owned(),
                });
            }
            if !seen_outputs.insert(inst.output.as_str()) {
                return Err(CoreError::InvalidManifest {
                    message: format!("duplicate instruction output `{}`", inst.output),
                });
            }
        }
        for hook in &self.hooks {
            if hook.timeout_secs == 0 {
                return Err(CoreError::InvalidManifest {
                    message: format!("hook `{}` timeout_secs must be greater than 0", hook.name),
                });
            }
            if hook.command.trim().is_empty() {
                return Err(CoreError::InvalidManifest {
                    message: format!("hook `{}` command must not be blank", hook.name),
                });
            }
        }
        Ok(())
    }
}

fn check_unique<'a>(
    ids: impl Iterator<Item = &'a str>,
    kind: &'static str,
) -> Result<(), CoreError> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(CoreError::InvalidManifest {
                message: format!("duplicate {kind} identifier `{id}`"),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use core::assert_matches;

    use super::*;

    #[test]
    fn minimal_manifest_parses_with_sensible_defaults() {
        let raw = r#"
            schema_version = 1

            [project]
            name = "demo-repo"
        "#;
        let manifest = ProjectManifest::from_toml_str(raw).unwrap();
        assert_eq!(manifest.project.name, "demo-repo");
        assert!(manifest.targets.antigravity.enabled);
        assert!(!manifest.targets.claude_code.enabled);
        assert_eq!(manifest.lint.rule_max_bytes, DEFAULT_RULE_MAX_BYTES);
        assert_eq!(
            manifest.lint.always_on_max_tokens,
            DEFAULT_ALWAYS_ON_MAX_TOKENS
        );
    }

    #[test]
    fn rejects_unsupported_schema_version_and_duplicates() {
        let bad_ver = "schema_version = 99\n[project]\nname = \"x\"\n";
        assert_matches!(
            ProjectManifest::from_toml_str(bad_ver),
            Err(CoreError::UnsupportedSchemaVersion {
                found: 99,
                expected: 1
            })
        );

        let dup_skill = r#"
            schema_version = 1
            [project]
            name = "demo"
            [[skills]]
            name = "pr-review"
            source = "skills/pr-review"
            [[skills]]
            name = "pr-review"
            source = "skills/pr-review-2"
        "#;
        assert_matches!(
            ProjectManifest::from_toml_str(dup_skill),
            Err(CoreError::InvalidManifest { .. })
        );
    }

    #[test]
    fn rejects_zero_hook_timeout_and_invalid_kebab_skill_name() {
        let zero_timeout = r#"
            schema_version = 1
            [project]
            name = "demo"
            [[hooks]]
            name = "guard"
            event = "pre_tool_use"
            command = "lazyagents hook eval git-guard"
            timeout_secs = 0
        "#;
        assert_matches!(
            ProjectManifest::from_toml_str(zero_timeout),
            Err(CoreError::InvalidManifest { .. })
        );

        let bad_skill = r#"
            schema_version = 1
            [project]
            name = "demo"
            [[skills]]
            name = "Not_Kebab"
            source = "skills/bad"
        "#;
        assert_matches!(
            ProjectManifest::from_toml_str(bad_skill),
            Err(CoreError::ManifestParse { .. })
        );
    }
}
