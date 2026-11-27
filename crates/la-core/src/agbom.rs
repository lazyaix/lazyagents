//! Deterministic Agent Bill of Materials (`AgBOM`) builder tracking provenance, source origins,
//! pinned revisions, SHA-256 digests, and Lethal Trifecta risk surfaces across all active assets.

use core::fmt::Write as _;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::compiler::CompileOutput;
use crate::error::CoreError;
use crate::lint::{LethalTrifectaReport, analyze_lethal_trifecta, effective_mcp_risk_surfaces};
use crate::lock::sha256_hex;
use crate::manifest::{McpRiskSurface, McpTransport, PackSource};
use crate::scope::{ConfigScope, ResolvedConfig};

/// Current schema version for serialized [`AgBomReport`] JSON documents.
pub const AGBOM_SCHEMA_VERSION: u32 = 1;

/// Category of an individual component recorded in an [`AgBomReport`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgBomComponentKind {
    /// Dynamic `AGENTS.md` / `GEMINI.md` instruction document.
    Instruction,
    /// Conditional modular rule (`.agents/rules/<id>.md`).
    Rule,
    /// Agent Skill bundle (`.agents/skills/<name>/SKILL.md`).
    Skill,
    /// Custom subagent persona (`.agents/agents/<name>.md`).
    Subagent,
    /// Lifecycle hook handler.
    Hook,
    /// Model Context Protocol server integration.
    McpServer,
    /// Cohesive plugin bundle.
    Plugin,
    /// Reusable pack dependency (`builtin`, `local`, `git`, `registry`).
    Pack,
}

/// Single deterministic asset record inside an [`AgBomReport`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgBomComponent {
    /// Stable component identifier or output path.
    pub id: String,
    /// Component category.
    pub kind: AgBomComponentKind,
    /// Whether this component is enabled in the resolved manifest.
    pub enabled: bool,
    /// Winning configuration scope that supplied this component.
    pub scope: ConfigScope,
    /// Origin descriptor (relative source path, transport URI/command, or pack URL).
    pub source_origin: String,
    /// Optional pinned commit SHA or version when sourced externally.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned_rev: Option<String>,
    /// Deterministic SHA-256 digest of the compiled output or source payload when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// Effective security risk surfaces (`untrusted_input`, `private_data`, `external_egress`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risk_surfaces: Vec<McpRiskSurface>,
}

/// Complete deterministic Agent Bill of Materials exported by `lazyagents inspect --agbom`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgBomReport {
    /// `AgBOM` schema version (currently [`AGBOM_SCHEMA_VERSION`]).
    pub schema_version: u32,
    /// Project name from `[project]`.
    pub project: String,
    /// Active configuration scopes that participated in resolution.
    pub active_scopes: Vec<ConfigScope>,
    /// Static Lethal Trifecta risk assessment across MCP servers and subagents.
    pub lethal_trifecta: LethalTrifectaReport,
    /// Ordered list of all resolved agent configuration components.
    pub components: Vec<AgBomComponent>,
}

impl AgBomReport {
    /// Serializes this `AgBOM` report as deterministic pretty JSON with a trailing LF.
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

    /// Renders a concise human-readable `AgBOM` summary table for terminal output.
    #[must_use]
    pub fn format_text(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "AgBOM v{} — Project: {} (components: {}, lethal_trifecta: {:?})",
            self.schema_version,
            self.project,
            self.components.len(),
            self.lethal_trifecta.overall_risk
        );
        for item in &self.components {
            let status = if item.enabled { "enabled" } else { "disabled" };
            let digest = item
                .sha256
                .as_deref()
                .and_then(|s| s.get(..12))
                .unwrap_or("-");
            let _ = writeln!(
                out,
                "- [{:?}] {} ({status}, scope={}, sha256={digest}, origin={})",
                item.kind, item.id, item.scope, item.source_origin
            );
        }
        out
    }
}

/// Builds a deterministic [`AgBomReport`] from a resolved manifest, compiled outputs, and
/// `.lazyagents/` source files (100% Sans-I/O).
#[must_use]
pub fn build_agbom(
    resolved: &ResolvedConfig,
    compiled: &CompileOutput,
    source_files: &BTreeMap<String, String>,
) -> AgBomReport {
    let manifest = &resolved.manifest;
    let compiled_map: BTreeMap<&str, &str> = compiled
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.content.as_str()))
        .collect();
    let default_scope = resolved
        .active_scopes
        .last()
        .copied()
        .unwrap_or(ConfigScope::Project);

    let mut components = Vec::new();
    collect_pack_and_doc_components(
        resolved,
        &compiled_map,
        source_files,
        default_scope,
        &mut components,
    );
    collect_runtime_components(resolved, &compiled_map, default_scope, &mut components);

    AgBomReport {
        schema_version: AGBOM_SCHEMA_VERSION,
        project: manifest.project.name.clone(),
        active_scopes: resolved.active_scopes.clone(),
        lethal_trifecta: analyze_lethal_trifecta(manifest),
        components,
    }
}

fn scope_for(resolved: &ResolvedConfig, key: &str, default_scope: ConfigScope) -> ConfigScope {
    resolved
        .provenance
        .get(key)
        .copied()
        .unwrap_or(default_scope)
}

fn collect_pack_and_doc_components(
    resolved: &ResolvedConfig,
    compiled_map: &BTreeMap<&str, &str>,
    source_files: &BTreeMap<String, String>,
    default_scope: ConfigScope,
    components: &mut Vec<AgBomComponent>,
) {
    let manifest = &resolved.manifest;

    for pack in &manifest.packs {
        let (source_origin, pinned_rev) = match &pack.source {
            PackSource::Builtin { name } => (format!("builtin:{name}"), None),
            PackSource::Local { path } => (format!("local:{path}"), None),
            PackSource::Git { url, rev } => (format!("git:{url}"), rev.clone()),
            PackSource::Registry { package, version } => {
                (format!("registry:{package}"), version.clone())
            }
        };
        components.push(AgBomComponent {
            id: pack.id.as_str().to_owned(),
            kind: AgBomComponentKind::Pack,
            enabled: true,
            scope: scope_for(resolved, &format!("pack:{}", pack.id), default_scope),
            source_origin,
            pinned_rev,
            sha256: None,
            risk_surfaces: Vec::new(),
        });
    }

    for inst in &manifest.instructions {
        let sha256 = compiled_map
            .get(inst.output.as_str())
            .map(|body| sha256_hex(body.as_bytes()));
        components.push(AgBomComponent {
            id: inst.output.clone(),
            kind: AgBomComponentKind::Instruction,
            enabled: true,
            scope: scope_for(
                resolved,
                &format!("instruction:{}", inst.output),
                default_scope,
            ),
            source_origin: format!("compiled:{}", inst.output),
            pinned_rev: None,
            sha256,
            risk_surfaces: Vec::new(),
        });
    }

    for rule in &manifest.rules {
        let compiled_path = format!(".agents/rules/{}.md", rule.id);
        let sha256 = compiled_map
            .get(compiled_path.as_str())
            .copied()
            .or_else(|| source_files.get(&rule.source).map(String::as_str))
            .map(|body| sha256_hex(body.as_bytes()));
        components.push(AgBomComponent {
            id: rule.id.as_str().to_owned(),
            kind: AgBomComponentKind::Rule,
            enabled: rule.enabled,
            scope: scope_for(resolved, &format!("rule:{}", rule.id), default_scope),
            source_origin: format!(".lazyagents/{}", rule.source),
            pinned_rev: None,
            sha256,
            risk_surfaces: Vec::new(),
        });
    }

    for skill in &manifest.skills {
        let compiled_path = format!(".agents/skills/{}/SKILL.md", skill.name);
        let source_key = format!("{}/SKILL.md", skill.source.trim_end_matches('/'));
        let sha256 = compiled_map
            .get(compiled_path.as_str())
            .copied()
            .or_else(|| source_files.get(&source_key).map(String::as_str))
            .map(|body| sha256_hex(body.as_bytes()));
        components.push(AgBomComponent {
            id: skill.name.as_str().to_owned(),
            kind: AgBomComponentKind::Skill,
            enabled: skill.enabled,
            scope: scope_for(resolved, &format!("skill:{}", skill.name), default_scope),
            source_origin: format!(".lazyagents/{}", skill.source),
            pinned_rev: None,
            sha256,
            risk_surfaces: Vec::new(),
        });
    }

    for agent in &manifest.agents {
        let compiled_path = format!(".agents/agents/{}.md", agent.name);
        let sha256 = compiled_map
            .get(compiled_path.as_str())
            .copied()
            .or_else(|| source_files.get(&agent.source).map(String::as_str))
            .map(|body| sha256_hex(body.as_bytes()));
        components.push(AgBomComponent {
            id: agent.name.as_str().to_owned(),
            kind: AgBomComponentKind::Subagent,
            enabled: agent.enabled,
            scope: scope_for(resolved, &format!("agent:{}", agent.name), default_scope),
            source_origin: format!(".lazyagents/{}", agent.source),
            pinned_rev: None,
            sha256,
            risk_surfaces: Vec::new(),
        });
    }
}

fn collect_runtime_components(
    resolved: &ResolvedConfig,
    compiled_map: &BTreeMap<&str, &str>,
    default_scope: ConfigScope,
    components: &mut Vec<AgBomComponent>,
) {
    let manifest = &resolved.manifest;

    for hook in &manifest.hooks {
        components.push(AgBomComponent {
            id: hook.name.as_str().to_owned(),
            kind: AgBomComponentKind::Hook,
            enabled: hook.enabled,
            scope: scope_for(resolved, &format!("hook:{}", hook.name), default_scope),
            source_origin: hook.command.clone(),
            pinned_rev: None,
            sha256: Some(sha256_hex(hook.command.as_bytes())),
            risk_surfaces: Vec::new(),
        });
    }

    for server in &manifest.mcp_servers {
        let source_origin = match &server.transport {
            McpTransport::StreamableHttp { url, .. } | McpTransport::LegacySse { url, .. } => {
                url.clone()
            }
            McpTransport::Stdio { command, args, .. } => {
                if args.is_empty() {
                    command.clone()
                } else {
                    format!("{command} {}", args.join(" "))
                }
            }
        };
        components.push(AgBomComponent {
            id: server.name.as_str().to_owned(),
            kind: AgBomComponentKind::McpServer,
            enabled: server.enabled,
            scope: scope_for(resolved, &format!("mcp:{}", server.name), default_scope),
            source_origin: source_origin.clone(),
            pinned_rev: None,
            sha256: Some(sha256_hex(source_origin.as_bytes())),
            risk_surfaces: effective_mcp_risk_surfaces(server),
        });
    }

    for plugin in &manifest.plugins {
        let manifest_path = format!(".agents/plugins/{}/plugin.json", plugin.name);
        let sha256 = compiled_map
            .get(manifest_path.as_str())
            .map(|body| sha256_hex(body.as_bytes()));
        components.push(AgBomComponent {
            id: plugin.name.as_str().to_owned(),
            kind: AgBomComponentKind::Plugin,
            enabled: plugin.enabled,
            scope: scope_for(resolved, &format!("plugin:{}", plugin.name), default_scope),
            source_origin: format!(".agents/plugins/{}", plugin.name),
            pinned_rev: plugin.version.clone(),
            sha256,
            risk_surfaces: Vec::new(),
        });
    }
}
