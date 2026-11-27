//! Google Antigravity V1 compiler translating canonical IR and `[engineering]` policies
//! into all 8 repository-checked-in Antigravity customization assets.

use core::fmt::Write as _;
use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::catalog::{builtin_github_mcp_preset, builtin_section_template, builtin_skill_template};
use crate::compiler::{
    CompatibilityDiagnostic, CompatibilitySeverity, CompileInputs, CompileOutput, GeneratedFile,
};
use crate::engineering::StopHookMode;
use crate::error::CoreError;
use crate::id::{HookEvent, PolicyDecision, ResourceId, TargetKind, ToolCapability};
use crate::manifest::{
    HookMatcher, HookSpec, IndexManifestSpec, InstructionFileKind, InstructionSection, LintLevel,
    McpServerSpec, McpTransport, McpVariable, ModelTier, PluginSpec, ProjectManifest, RuleSpec,
    RuleTrigger, SubagentSpec,
};
use crate::template::{TemplateContext, TemplateEngine, normalize_lf};

/// Google Antigravity target compiler (V1 primary target).
#[derive(Clone, Copy, Debug, Default)]
pub struct AntigravityCompiler;

impl AntigravityCompiler {
    /// Compiles all enabled Antigravity project assets (`AGENTS.md`, `.agents/rules/*.md`,
    /// `.agents/skills/**`, `.agents/agents/*.md`, `.agents/hooks.json`, `.agents/mcp_config.json`,
    /// `.agents/{skills,rules,plugins}.json`, and `.agents/plugins/<name>/**`).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::TemplateRender`] or [`CoreError::Compile`] when templates or
    /// referenced source files cannot be resolved or serialized.
    pub fn compile(
        manifest: &ProjectManifest,
        inputs: &CompileInputs<'_>,
    ) -> Result<CompileOutput, CoreError> {
        let mut files = Vec::new();
        let mut diagnostics = Vec::new();
        let ctx = TemplateContext {
            project: &inputs.facts,
            engineering: &manifest.engineering,
        };

        compile_instructions(manifest, inputs, &ctx, &mut files)?;
        compile_rules(manifest, inputs, &mut files)?;
        compile_skills(manifest, inputs, &mut files)?;
        compile_subagents(manifest, inputs, &mut files)?;

        let effective_hooks = collect_effective_hooks(manifest);
        if !effective_hooks.is_empty() {
            let json = render_hooks_json(&effective_hooks)?;
            files.push(GeneratedFile::whole_file(".agents/hooks.json", json));
        }

        let effective_mcp = collect_effective_mcp(manifest);
        if !effective_mcp.is_empty() {
            let json = render_mcp_config_json(&effective_mcp, &mut diagnostics)?;
            files.push(GeneratedFile::whole_file(".agents/mcp_config.json", json));
        }

        compile_index_manifests(manifest, &mut files)?;
        compile_plugins(
            manifest,
            inputs,
            &effective_hooks,
            &effective_mcp,
            &mut files,
            &mut diagnostics,
        )?;

        Ok(CompileOutput { files, diagnostics })
    }
}

/// Normalizes a list of glob patterns into Antigravity's single comma-separated `globs` string:
/// expands simple `{a,b}` brace lists and prefixes workspace-relative paths with `**/`.
#[must_use]
pub fn normalize_antigravity_globs(globs: &[String]) -> String {
    let mut expanded = Vec::new();
    for raw in globs {
        for part in expand_braces(raw.trim()) {
            let normalized = if part.is_empty() {
                continue;
            } else if part.starts_with("**/") || part.starts_with('/') || part.starts_with("*.") {
                part
            } else {
                format!("**/{part}")
            };
            if !expanded.contains(&normalized) {
                expanded.push(normalized);
            }
        }
    }
    expanded.join(", ")
}

fn expand_braces(pattern: &str) -> Vec<String> {
    let Some(open) = pattern.find('{') else {
        return vec![pattern.to_owned()];
    };
    let rest = pattern.get(open.saturating_add(1)..).unwrap_or_default();
    let Some(rel_close) = rest.find('}') else {
        return vec![pattern.to_owned()];
    };
    let prefix = pattern.get(..open).unwrap_or_default();
    let body = rest.get(..rel_close).unwrap_or_default();
    let suffix = rest.get(rel_close.saturating_add(1)..).unwrap_or_default();

    let mut results = Vec::new();
    for option in body.split(',') {
        let combined = format!("{prefix}{}{suffix}", option.trim());
        results.extend(expand_braces(&combined));
    }
    results
}

fn compile_instructions(
    manifest: &ProjectManifest,
    inputs: &CompileInputs<'_>,
    ctx: &TemplateContext<'_>,
    out: &mut Vec<GeneratedFile>,
) -> Result<(), CoreError> {
    for inst in &manifest.instructions {
        let mut rendered_blocks = Vec::new();
        for section in &inst.sections {
            if let Some(block) = render_instruction_section(section, inputs, ctx)? {
                let trimmed = block.trim();
                if !trimmed.is_empty() {
                    rendered_blocks.push(trimmed.to_owned());
                }
            }
        }
        let mut body = rendered_blocks.join("\n\n");
        if !body.is_empty() && !body.ends_with('\n') {
            body.push('\n');
        }
        let path = resolve_instruction_output_path(&inst.output, manifest);
        out.push(GeneratedFile::whole_file(path, body));
    }
    Ok(())
}

fn resolve_instruction_output_path(raw: &str, manifest: &ProjectManifest) -> String {
    if raw == "AGENTS.md"
        && manifest.targets.antigravity.instruction_file == InstructionFileKind::GeminiMd
    {
        "GEMINI.md".to_owned()
    } else {
        raw.to_owned()
    }
}

fn render_instruction_section(
    section: &InstructionSection,
    inputs: &CompileInputs<'_>,
    ctx: &TemplateContext<'_>,
) -> Result<Option<String>, CoreError> {
    let when_expr = match section {
        InstructionSection::Preset { when, .. }
        | InstructionSection::File { when, .. }
        | InstructionSection::Inline { when, .. } => when.as_deref(),
    };
    if let Some(expr) = when_expr
        && !TemplateEngine::eval_condition(expr, ctx)?
    {
        return Ok(None);
    }

    match section {
        InstructionSection::Preset { preset, .. } => {
            let tpl = builtin_section_template(preset).ok_or_else(|| CoreError::Compile {
                message: format!("unknown built-in instruction preset `{preset}`"),
            })?;
            TemplateEngine::render(preset, tpl, ctx).map(Some)
        }
        InstructionSection::File { file, .. } => {
            let src = inputs
                .source_files
                .get(file)
                .ok_or_else(|| CoreError::Compile {
                    message: format!("missing instruction section source `{file}`"),
                })?;
            TemplateEngine::render(file, src, ctx).map(Some)
        }
        InstructionSection::Inline { inline, .. } => {
            TemplateEngine::render("inline-section", inline, ctx).map(Some)
        }
    }
}

fn compile_rules(
    manifest: &ProjectManifest,
    inputs: &CompileInputs<'_>,
    out: &mut Vec<GeneratedFile>,
) -> Result<(), CoreError> {
    for rule in &manifest.rules {
        if !rule.enabled {
            continue;
        }
        let raw_body = inputs
            .source_files
            .get(&rule.source)
            .ok_or_else(|| CoreError::Compile {
                message: format!(
                    "missing rule source `{}` for rule `{}`",
                    rule.source, rule.id
                ),
            })?;
        let rendered = render_rule_markdown(rule, raw_body);
        out.push(GeneratedFile::whole_file(
            format!(".agents/rules/{}.md", rule.id),
            rendered,
        ));
    }

    for layer in &manifest.engineering.architecture.layers {
        let globs = normalize_antigravity_globs(core::slice::from_ref(&layer.glob));
        let content = format!(
            "---\ntrigger: glob\nglobs: {globs}\ndescription: Architecture boundary for {}\n---\n\n# {} Layer Boundary\n\n{}\n",
            layer.name, layer.name, layer.rule
        );
        out.push(GeneratedFile::whole_file(
            format!(".agents/rules/arch-{}.md", layer.name),
            content,
        ));
    }

    Ok(())
}

fn render_rule_markdown(rule: &RuleSpec, body: &str) -> String {
    let trigger = match rule.trigger {
        RuleTrigger::AlwaysOn => "always_on",
        RuleTrigger::Glob => "glob",
        RuleTrigger::ModelDecision => "model_decision",
        RuleTrigger::Manual => "manual",
    };
    let mut frontmatter = format!("---\ntrigger: {trigger}\n");
    if rule.trigger == RuleTrigger::Glob && !rule.globs.is_empty() {
        let _ = writeln!(
            frontmatter,
            "globs: {}",
            normalize_antigravity_globs(&rule.globs)
        );
    }
    if let Some(desc) = &rule.description {
        let clean = desc.trim();
        if !clean.is_empty() {
            let _ = writeln!(frontmatter, "description: {clean}");
        }
    }
    frontmatter.push_str("---\n\n");
    let normalized = normalize_lf(body);
    let trimmed = normalized.trim_start_matches('\n');
    let mut out = format!("{frontmatter}{trimmed}");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn compile_skills(
    manifest: &ProjectManifest,
    inputs: &CompileInputs<'_>,
    out: &mut Vec<GeneratedFile>,
) -> Result<(), CoreError> {
    let mut emitted_skills = BTreeSet::new();

    for skill in &manifest.skills {
        if !skill.enabled {
            continue;
        }
        let prefix = format!("{}/", skill.source.trim_end_matches('/'));
        let mut found_any = false;
        for (src_path, content) in &inputs.source_files {
            if let Some(rel) = src_path.strip_prefix(&prefix) {
                found_any = true;
                out.push(GeneratedFile::whole_file(
                    format!(".agents/skills/{}/{rel}", skill.name),
                    normalize_lf(content),
                ));
            }
        }
        if !found_any {
            if let Some(builtin) = builtin_skill_template(skill.name.as_str()) {
                out.push(GeneratedFile::whole_file(
                    format!(".agents/skills/{}/SKILL.md", skill.name),
                    normalize_lf(builtin.skill_md),
                ));
            } else {
                return Err(CoreError::Compile {
                    message: format!(
                        "missing skill files under `{}` for skill `{}`",
                        skill.source, skill.name
                    ),
                });
            }
        }
        emitted_skills.insert(skill.name.as_str().to_owned());
    }

    if manifest.engineering.git.worktree.enabled
        && !emitted_skills.contains("worktree-isolation")
        && let Some(builtin) = builtin_skill_template("worktree-isolation")
    {
        out.push(GeneratedFile::whole_file(
            ".agents/skills/worktree-isolation/SKILL.md",
            normalize_lf(builtin.skill_md),
        ));
        emitted_skills.insert("worktree-isolation".to_owned());
    }

    if manifest.engineering.github.enabled {
        for skill_name in &manifest.engineering.github.enable_skills {
            if emitted_skills.contains(skill_name.as_str()) {
                continue;
            }
            if let Some(builtin) = builtin_skill_template(skill_name.as_str()) {
                out.push(GeneratedFile::whole_file(
                    format!(".agents/skills/{}/SKILL.md", skill_name.as_str()),
                    normalize_lf(builtin.skill_md),
                ));
                emitted_skills.insert(skill_name.as_str().to_owned());
            }
        }
    }

    Ok(())
}

fn compile_subagents(
    manifest: &ProjectManifest,
    inputs: &CompileInputs<'_>,
    out: &mut Vec<GeneratedFile>,
) -> Result<(), CoreError> {
    for agent in &manifest.agents {
        if !agent.enabled {
            continue;
        }
        let body = inputs
            .source_files
            .get(&agent.source)
            .ok_or_else(|| CoreError::Compile {
                message: format!(
                    "missing subagent prompt `{}` for agent `{}`",
                    agent.source, agent.name
                ),
            })?;
        let rendered = render_subagent_markdown(agent, body);
        out.push(GeneratedFile::whole_file(
            format!(".agents/agents/{}.md", agent.name),
            rendered,
        ));
    }
    Ok(())
}

fn render_subagent_markdown(agent: &SubagentSpec, body: &str) -> String {
    let model = match agent.model {
        ModelTier::Inherit => "inherit",
        ModelTier::FlashLite => "flash_lite",
        ModelTier::Flash => "flash",
        ModelTier::Pro => "pro",
    };
    let ext = &agent.targets.antigravity;
    let mut fm = format!(
        "---\nname: {}\ndescription: {}\nmodel: {model}\nmainAgent: {}\nsubagent: {}\n",
        agent.name,
        agent.description.trim(),
        ext.main_agent,
        ext.subagent
    );
    if let Some(perm) = &ext.permission_mode {
        let _ = writeln!(fm, "permissionMode: {perm}");
    }
    if let Some(policy) = &ext.command_execution_policy {
        let _ = writeln!(fm, "commandExecutionPolicy: {policy}");
    }
    let tools = map_antigravity_tools(&agent.capabilities);
    if !tools.is_empty() {
        fm.push_str("tools:\n");
        for tool in tools {
            let _ = writeln!(fm, "  - {tool}");
        }
    }
    if !agent.skills.is_empty() {
        fm.push_str("skills:\n");
        for skill in &agent.skills {
            let _ = writeln!(fm, "  - {skill}");
        }
    }
    if !agent.mcp_servers.is_empty() {
        fm.push_str("mcpServers:\n");
        for server in &agent.mcp_servers {
            let _ = writeln!(fm, "  - {server}");
        }
    }
    fm.push_str("---\n\n");
    let normalized = normalize_lf(body);
    let mut out = format!("{fm}{}", normalized.trim_start_matches('\n'));
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn map_antigravity_tools(capabilities: &[ToolCapability]) -> Vec<String> {
    let mut tools = Vec::new();
    for cap in capabilities {
        let mapped: &[&str] = match cap {
            ToolCapability::FileRead => &["read_file"],
            ToolCapability::FileWrite => &["edit_file", "write_file"],
            ToolCapability::Shell => &["run_command", "manage_task"],
            ToolCapability::Search => &["grep_search", "find_by_name"],
            ToolCapability::WebSearch => &["search_web"],
            ToolCapability::Browser => &["browser_subagent"],
            ToolCapability::Subagent => &["invoke_subagent"],
            ToolCapability::Custom(custom) => {
                if !tools.contains(custom) {
                    tools.push(custom.clone());
                }
                continue;
            }
        };
        for name in mapped {
            let owned = (*name).to_owned();
            if !tools.contains(&owned) {
                tools.push(owned);
            }
        }
    }
    tools
}

fn collect_effective_hooks(manifest: &ProjectManifest) -> Vec<HookSpec> {
    let mut hooks = Vec::new();
    let existing_names: BTreeSet<&str> = manifest.hooks.iter().map(|h| h.name.as_str()).collect();

    let git = &manifest.engineering.git.guardrails;
    let git_guard_active = git.block_push_default_branch != PolicyDecision::Allow
        || git.block_force_push != PolicyDecision::Allow
        || git.block_destructive_reset != PolicyDecision::Allow
        || git.guard_broad_staging != PolicyDecision::Allow;

    if git_guard_active && !existing_names.contains("lazyagents-git-guard") {
        hooks.push(HookSpec {
            name: ResourceId::from_static_valid("lazyagents-git-guard"),
            enabled: true,
            event: HookEvent::PreToolUse,
            matcher: Some(HookMatcher::Capability {
                capability: ToolCapability::Shell,
            }),
            command: "lazyagents hook eval git-guard".to_owned(),
            timeout_secs: 15,
        });
    }

    if !manifest.engineering.protected_paths.entries.is_empty()
        && !existing_names.contains("lazyagents-protected-paths")
    {
        hooks.push(HookSpec {
            name: ResourceId::from_static_valid("lazyagents-protected-paths"),
            enabled: true,
            event: HookEvent::PreToolUse,
            matcher: Some(HookMatcher::Capability {
                capability: ToolCapability::FileWrite,
            }),
            command: "lazyagents hook eval protected-paths".to_owned(),
            timeout_secs: 15,
        });
    }

    if manifest.lint.secrets != LintLevel::Off
        && !existing_names.contains("lazyagents-secret-guard")
    {
        hooks.push(HookSpec {
            name: ResourceId::from_static_valid("lazyagents-secret-guard"),
            enabled: true,
            event: HookEvent::PreToolUse,
            matcher: Some(HookMatcher::Capability {
                capability: ToolCapability::FileWrite,
            }),
            command: "lazyagents hook eval secret-guard".to_owned(),
            timeout_secs: 15,
        });
    }

    if manifest.engineering.verify.stop_hook_mode != StopHookMode::Off
        && !manifest.engineering.verify.commands.is_empty()
        && !existing_names.contains("lazyagents-verify-gate")
    {
        hooks.push(HookSpec {
            name: ResourceId::from_static_valid("lazyagents-verify-gate"),
            enabled: true,
            event: HookEvent::Stop,
            matcher: None,
            command: "lazyagents hook eval verify-gate".to_owned(),
            timeout_secs: 15,
        });
    }

    hooks.extend(manifest.hooks.iter().cloned());
    hooks
}

#[derive(Serialize)]
struct WireHookHandler<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    command: &'a str,
    timeout: u32,
}

#[derive(Serialize)]
struct WireToolMatcherGroup<'a> {
    matcher: String,
    hooks: Vec<WireHookHandler<'a>>,
}

fn render_hooks_json(hooks: &[HookSpec]) -> Result<String, CoreError> {
    let mut root = serde_json::Map::new();
    for hook in hooks {
        let mut entry = serde_json::Map::new();
        if !hook.enabled {
            entry.insert("enabled".to_owned(), serde_json::Value::Bool(false));
        }
        let handler = WireHookHandler {
            kind: "command",
            command: &hook.command,
            timeout: hook.timeout_secs,
        };
        let event_key = hook.event.antigravity_key().to_owned();
        if hook.event.uses_tool_matcher() {
            let matcher = resolve_antigravity_matcher(hook.matcher.as_ref());
            let group = WireToolMatcherGroup {
                matcher,
                hooks: vec![handler],
            };
            let val = serde_json::to_value(vec![group]).map_err(|err| CoreError::Compile {
                message: err.to_string(),
            })?;
            entry.insert(event_key, val);
        } else {
            let val = serde_json::to_value(vec![handler]).map_err(|err| CoreError::Compile {
                message: err.to_string(),
            })?;
            entry.insert(event_key, val);
        }
        root.insert(
            hook.name.as_str().to_owned(),
            serde_json::Value::Object(entry),
        );
    }

    let mut out =
        serde_json::to_string_pretty(&serde_json::Value::Object(root)).map_err(|err| {
            CoreError::Compile {
                message: err.to_string(),
            }
        })?;
    out.push('\n');
    Ok(out)
}

fn resolve_antigravity_matcher(matcher: Option<&HookMatcher>) -> String {
    match matcher {
        None => "*".to_owned(),
        Some(HookMatcher::Regex { regex }) => regex.clone(),
        Some(HookMatcher::Capability { capability }) => match capability {
            ToolCapability::Shell => "run_command".to_owned(),
            ToolCapability::FileRead => "read_file".to_owned(),
            ToolCapability::FileWrite => "edit_file|write_file".to_owned(),
            ToolCapability::Search => "grep_search|find_by_name".to_owned(),
            ToolCapability::WebSearch => "search_web".to_owned(),
            ToolCapability::Browser => "browser_.*".to_owned(),
            ToolCapability::Subagent => "invoke_subagent".to_owned(),
            ToolCapability::Custom(custom) => custom.clone(),
        },
    }
}

fn collect_effective_mcp(manifest: &ProjectManifest) -> Vec<McpServerSpec> {
    let mut servers = manifest.mcp_servers.clone();
    let has_github = servers.iter().any(|s| s.name.as_str() == "github");
    if manifest.engineering.github.enabled
        && manifest.engineering.github.enable_mcp_preset
        && !has_github
    {
        servers.push(builtin_github_mcp_preset());
    }
    servers
}

#[derive(Serialize)]
struct WireMcpConfig<'a> {
    #[serde(rename = "mcpServers")]
    mcp_servers: BTreeMap<String, WireMcpServer<'a>>,
}

#[derive(Serialize)]
struct WireMcpServer<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    args: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    env: Option<&'a BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cwd: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: Option<&'a BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    disabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    variables: Option<&'a [McpVariable]>,
}

fn render_mcp_config_json(
    servers: &[McpServerSpec],
    diagnostics: &mut Vec<CompatibilityDiagnostic>,
) -> Result<String, CoreError> {
    let mut map = BTreeMap::new();
    for server in servers {
        let disabled = if server.enabled { None } else { Some(true) };
        let variables = if server.variables.is_empty() {
            None
        } else {
            Some(server.variables.as_slice())
        };
        let wire = match &server.transport {
            McpTransport::Stdio {
                command,
                args,
                env,
                cwd,
            } => WireMcpServer {
                command: Some(command.as_str()),
                args: (!args.is_empty()).then_some(args.as_slice()),
                env: (!env.is_empty()).then_some(env),
                cwd: cwd.as_deref(),
                url: None,
                headers: None,
                disabled,
                variables,
            },
            McpTransport::StreamableHttp { url, headers } => WireMcpServer {
                command: None,
                args: None,
                env: None,
                cwd: None,
                url: Some(url.as_str()),
                headers: (!headers.is_empty()).then_some(headers),
                disabled,
                variables,
            },
            McpTransport::LegacySse { url, headers } => {
                diagnostics.push(CompatibilityDiagnostic {
                    target: TargetKind::Antigravity,
                    severity: CompatibilitySeverity::Warning,
                    subject: format!("mcp:{}", server.name),
                    message: format!(
                        "Antigravity does not support legacy SSE transport (`{url}`); switch to Streamable HTTP or bridge via `npx -y mcp-remote {url}`"
                    ),
                });
                WireMcpServer {
                    command: None,
                    args: None,
                    env: None,
                    cwd: None,
                    url: Some(url.as_str()),
                    headers: (!headers.is_empty()).then_some(headers),
                    disabled,
                    variables,
                }
            }
        };
        map.insert(server.name.as_str().to_owned(), wire);
    }

    let cfg = WireMcpConfig { mcp_servers: map };
    let mut out = serde_json::to_string_pretty(&cfg).map_err(|err| CoreError::Compile {
        message: err.to_string(),
    })?;
    out.push('\n');
    Ok(out)
}

fn compile_index_manifests(
    manifest: &ProjectManifest,
    out: &mut Vec<GeneratedFile>,
) -> Result<(), CoreError> {
    let specs: [(&str, &IndexManifestSpec); 3] = [
        (".agents/skills.json", &manifest.manifests.skills),
        (".agents/rules.json", &manifest.manifests.rules),
        (".agents/plugins.json", &manifest.manifests.plugins),
    ];
    for (path, spec) in specs {
        if spec.is_empty() {
            continue;
        }
        let mut json = serde_json::to_string_pretty(spec).map_err(|err| CoreError::Compile {
            message: err.to_string(),
        })?;
        json.push('\n');
        out.push(GeneratedFile::whole_file(path, json));
    }
    Ok(())
}

#[derive(Serialize)]
struct WirePluginManifest<'a> {
    name: &'a str,
    #[serde(rename = "displayName", skip_serializing_if = "Option::is_none")]
    display_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<&'a str>,
    description: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    logo: Option<&'a str>,
    #[serde(rename = "suggestedPrompts", skip_serializing_if = "Option::is_none")]
    suggested_prompts: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    disabled: Option<bool>,
}

fn compile_plugins(
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
        emit_plugin_hooks(plugin, effective_hooks, &base, out)?;
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
    let json = render_hooks_json(&selected)?;
    out.push(GeneratedFile::whole_file(
        format!("{base}/hooks.json"),
        json,
    ));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_antigravity_globs_with_brace_expansion_and_prefix() {
        let globs = vec![
            "crates/**/*.rs".to_owned(),
            "src/**/*.{ts,tsx}".to_owned(),
            "**/already/**".to_owned(),
        ];
        assert_eq!(
            normalize_antigravity_globs(&globs),
            "**/crates/**/*.rs, **/src/**/*.ts, **/src/**/*.tsx, **/already/**"
        );
    }
}
