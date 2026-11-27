//! Antigravity Markdown asset compilers (`AGENTS.md` / `GEMINI.md`, conditional rules,
//! skill bundles, and custom subagents).

use core::fmt::Write as _;
use std::collections::BTreeSet;

use crate::catalog::builtin_skill_template;
use crate::compiler::shared::render_instruction_section;
use crate::compiler::{CompileInputs, GeneratedFile};
use crate::error::CoreError;
use crate::id::ToolCapability;
use crate::manifest::{
    InstructionFileKind, ModelTier, ProjectManifest, RuleSpec, RuleTrigger, SubagentSpec,
};
use crate::template::{TemplateContext, normalize_lf};

use super::normalize_antigravity_globs;

pub(super) fn compile_instructions(
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

pub(super) fn compile_rules(
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

pub(super) fn compile_skills(
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

pub(super) fn compile_subagents(
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

pub(super) fn render_subagent_markdown(agent: &SubagentSpec, body: &str) -> String {
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

fn resolve_instruction_output_path(raw: &str, manifest: &ProjectManifest) -> String {
    if raw == "AGENTS.md"
        && manifest.targets.antigravity.instruction_file == InstructionFileKind::GeminiMd
    {
        "GEMINI.md".to_owned()
    } else {
        raw.to_owned()
    }
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
