//! Antigravity plugin bundle compiler emitting `.agents/plugins/<name>/plugin.json`,
//! consolidated `rules/AGENTS.md`, `skills/`, `agents/`, root `hooks.json`, and `mcp_config.json`.

use serde::Serialize;

use crate::catalog::builtin_skill_template;
use crate::compiler::{CompatibilityDiagnostic, CompileInputs, GeneratedFile};
use crate::error::CoreError;
use crate::manifest::{HookSpec, McpServerSpec, PluginSpec, ProjectManifest};
use crate::template::normalize_lf;

use super::docs::render_subagent_markdown;
use super::runtime_assets::{render_hooks_json, render_mcp_config_json};

#[derive(Serialize)]
struct WirePluginManifest<'a> {
    name: &'a str,
    #[serde(rename = "displayName", skip_serializing_if = "Option::is_none")]
    display_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    homepage: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repository: Option<&'a str>,
    description: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    logo: Option<&'a str>,
    #[serde(rename = "suggestedPrompts", skip_serializing_if = "Option::is_none")]
    suggested_prompts: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    disabled: Option<bool>,
}

pub(super) fn compile_plugins(
    manifest: &ProjectManifest,
    inputs: &CompileInputs<'_>,
    effective_hooks: &[HookSpec],
    effective_mcp: &[McpServerSpec],
    out: &mut Vec<GeneratedFile>,
    diagnostics: &mut Vec<CompatibilityDiagnostic>,
) -> Result<(), CoreError> {
    for plugin in &manifest.plugins {
        let base = format!(".agents/plugins/{}", plugin.name);
        let wire_manifest = WirePluginManifest {
            name: plugin.name.as_str(),
            display_name: plugin.display_name.as_deref(),
            version: plugin.version.as_deref(),
            author: plugin.author.as_deref(),
            homepage: plugin.homepage.as_deref(),
            repository: plugin.repository.as_deref(),
            description: &plugin.description,
            logo: plugin.logo.as_deref(),
            suggested_prompts: (!plugin.suggested_prompts.is_empty())
                .then_some(plugin.suggested_prompts.as_slice()),
            disabled: (!plugin.enabled).then_some(true),
        };
        let mut manifest_json =
            serde_json::to_string_pretty(&wire_manifest).map_err(|err| CoreError::Compile {
                message: err.to_string(),
            })?;
        manifest_json.push('\n');
        out.push(GeneratedFile::whole_file(
            format!("{base}/plugin.json"),
            manifest_json,
        ));

        emit_plugin_rules(plugin, manifest, inputs, &base, out)?;
        emit_plugin_skills(plugin, manifest, inputs, &base, out)?;
        emit_plugin_agents(plugin, manifest, inputs, &base, out)?;
        emit_plugin_hooks(plugin, effective_hooks, &base, out, diagnostics)?;
        emit_plugin_mcp(plugin, effective_mcp, &base, out, diagnostics)?;
    }
    Ok(())
}

fn emit_plugin_rules(
    plugin: &PluginSpec,
    manifest: &ProjectManifest,
    inputs: &CompileInputs<'_>,
    base: &str,
    out: &mut Vec<GeneratedFile>,
) -> Result<(), CoreError> {
    if plugin.include_rules.is_empty() {
        return Ok(());
    }
    let mut parts = Vec::new();
    for rule_id in &plugin.include_rules {
        let rule = manifest
            .rules
            .iter()
            .find(|r| r.id == *rule_id)
            .ok_or_else(|| CoreError::Compile {
                message: format!(
                    "plugin `{}` references unknown rule `{rule_id}`",
                    plugin.name
                ),
            })?;
        let body = inputs
            .source_files
            .get(&rule.source)
            .ok_or_else(|| CoreError::Compile {
                message: format!("missing rule source `{}`", rule.source),
            })?;
        parts.push(normalize_lf(body).trim().to_owned());
    }
    let consolidated = format!("{}\n", parts.join("\n\n"));
    out.push(GeneratedFile::whole_file(
        format!("{base}/rules/AGENTS.md"),
        consolidated,
    ));
    Ok(())
}

fn emit_plugin_skills(
    plugin: &PluginSpec,
    manifest: &ProjectManifest,
    inputs: &CompileInputs<'_>,
    base: &str,
    out: &mut Vec<GeneratedFile>,
) -> Result<(), CoreError> {
    for skill_name in &plugin.include_skills {
        if let Some(skill) = manifest.skills.iter().find(|s| s.name == *skill_name) {
            let prefix = format!("{}/", skill.source.trim_end_matches('/'));
            for (src_path, content) in &inputs.source_files {
                if let Some(rel) = src_path.strip_prefix(&prefix) {
                    out.push(GeneratedFile::whole_file(
                        format!("{base}/skills/{}/{rel}", skill.name),
                        normalize_lf(content),
                    ));
                }
            }
        } else if let Some(builtin) = builtin_skill_template(skill_name.as_str()) {
            out.push(GeneratedFile::whole_file(
                format!("{base}/skills/{}/SKILL.md", skill_name.as_str()),
                normalize_lf(builtin.skill_md),
            ));
        } else {
            return Err(CoreError::Compile {
                message: format!(
                    "plugin `{}` references unknown skill `{skill_name}`",
                    plugin.name
                ),
            });
        }
    }
    Ok(())
}

fn emit_plugin_agents(
    plugin: &PluginSpec,
    manifest: &ProjectManifest,
    inputs: &CompileInputs<'_>,
    base: &str,
    out: &mut Vec<GeneratedFile>,
) -> Result<(), CoreError> {
    for agent_name in &plugin.include_agents {
        let agent = manifest
            .agents
            .iter()
            .find(|a| a.name == *agent_name)
            .ok_or_else(|| CoreError::Compile {
                message: format!(
                    "plugin `{}` references unknown subagent `{agent_name}`",
                    plugin.name
                ),
            })?;
        let body = inputs
            .source_files
            .get(&agent.source)
            .ok_or_else(|| CoreError::Compile {
                message: format!("missing subagent source `{}`", agent.source),
            })?;
        out.push(GeneratedFile::whole_file(
            format!("{base}/agents/{}.md", agent.name),
            render_subagent_markdown(agent, body),
        ));
    }
    Ok(())
}

fn emit_plugin_hooks(
    plugin: &PluginSpec,
    effective_hooks: &[HookSpec],
    base: &str,
    out: &mut Vec<GeneratedFile>,
    diagnostics: &mut Vec<CompatibilityDiagnostic>,
) -> Result<(), CoreError> {
    if plugin.include_hooks.is_empty() {
        return Ok(());
    }
    let mut selected = Vec::new();
    for hook_id in &plugin.include_hooks {
        let hook = effective_hooks
            .iter()
            .find(|h| h.name == *hook_id)
            .ok_or_else(|| CoreError::Compile {
                message: format!(
                    "plugin `{}` references unknown hook `{hook_id}`",
                    plugin.name
                ),
            })?;
        selected.push(hook.clone());
    }
    if let Some(json) = render_hooks_json(&selected, diagnostics)? {
        out.push(GeneratedFile::whole_file(
            format!("{base}/hooks.json"),
            json,
        ));
    }
    Ok(())
}

fn emit_plugin_mcp(
    plugin: &PluginSpec,
    effective_mcp: &[McpServerSpec],
    base: &str,
    out: &mut Vec<GeneratedFile>,
    diagnostics: &mut Vec<CompatibilityDiagnostic>,
) -> Result<(), CoreError> {
    if plugin.include_mcp.is_empty() {
        return Ok(());
    }
    let mut selected = Vec::new();
    for server_id in &plugin.include_mcp {
        let server = effective_mcp
            .iter()
            .find(|m| m.name == *server_id)
            .ok_or_else(|| CoreError::Compile {
                message: format!(
                    "plugin `{}` references unknown MCP server `{server_id}`",
                    plugin.name
                ),
            })?;
        selected.push(server.clone());
    }
    let json = render_mcp_config_json(&selected, diagnostics)?;
    out.push(GeneratedFile::whole_file(
        format!("{base}/mcp_config.json"),
        json,
    ));
    Ok(())
}
