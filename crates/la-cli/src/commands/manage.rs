//! Script- and agent-friendly resource CRUD commands: `add`, `remove`, `enable`, `disable`.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;

use la_core::manifest::{
    HookMatcher, HookSpec, InstructionSection, InstructionSpec, McpAuthMode, McpServerSpec,
    McpToolLoading, McpTransport, ModelTier, PluginSpec, RuleSpec, SkillSpec, SubagentSpec,
    SubagentTargetExt,
};
use la_core::{HookEvent, KebabName, ProjectManifest, ResourceId, ToolCapability};
use la_runtime::{LOCAL_MANIFEST_REL_PATH, MANIFEST_REL_PATH, save_manifest, write_atomic};

use crate::cli::{AddArgs, RemoveArgs, ResourceKind, ToggleArgs};
use crate::commands::CommandOutcome;

pub(super) fn cmd_add(root: &Path, args: &AddArgs, out: &mut impl Write) -> CommandOutcome {
    let mut manifest = match load_single_manifest(root, args.local) {
        Ok(m) => m,
        Err(msg) => return CommandOutcome::RuntimeError(msg),
    };

    let res = match args.kind {
        ResourceKind::Section => add_section(root, &mut manifest, args),
        ResourceKind::Rule => add_rule(root, &mut manifest, args),
        ResourceKind::Skill => add_skill(root, &mut manifest, args),
        ResourceKind::Agent => add_agent(root, &mut manifest, args),
        ResourceKind::Hook => add_hook(&mut manifest, args),
        ResourceKind::Mcp => add_mcp(&mut manifest, args),
        ResourceKind::Plugin => add_plugin(&mut manifest, args),
        ResourceKind::Target => Err(
            "targets cannot be created with `add`; use `lazyagents enable target <name>`"
                .to_owned(),
        ),
    };

    if let Err(msg) = res {
        return CommandOutcome::RuntimeError(msg);
    }
    if let Err(err) = save_manifest(root, &manifest, args.local) {
        return CommandOutcome::RuntimeError(err.to_string());
    }

    let _ = writeln!(out, "Added {:?} `{}`.", args.kind, args.name);
    CommandOutcome::Pass
}

pub(super) fn cmd_remove(root: &Path, args: &RemoveArgs, out: &mut impl Write) -> CommandOutcome {
    let mut manifest = match load_single_manifest(root, args.local) {
        Ok(m) => m,
        Err(msg) => return CommandOutcome::RuntimeError(msg),
    };

    let (removed, purge_rel): (bool, Option<String>) = match args.kind {
        ResourceKind::Section => {
            let mut found = false;
            let file_rel = format!("sections/{}.md", args.name);
            for inst in &mut manifest.instructions {
                let before = inst.sections.len();
                inst.sections.retain(|s| match s {
                    InstructionSection::Preset { preset, .. } => preset != &args.name,
                    InstructionSection::File { file, .. } => {
                        file != &args.name && file != &file_rel
                    }
                    InstructionSection::Inline { .. } => true,
                });
                if inst.sections.len() < before {
                    found = true;
                }
            }
            (found, Some(file_rel))
        }
        ResourceKind::Rule => {
            let src = manifest
                .rules
                .iter()
                .find(|r| r.id.as_str() == args.name)
                .map(|r| r.source.clone());
            let before = manifest.rules.len();
            manifest.rules.retain(|r| r.id.as_str() != args.name);
            (manifest.rules.len() < before, src)
        }
        ResourceKind::Skill => {
            let src = manifest
                .skills
                .iter()
                .find(|s| s.name.as_str() == args.name)
                .map(|s| s.source.clone());
            let before = manifest.skills.len();
            manifest.skills.retain(|s| s.name.as_str() != args.name);
            (manifest.skills.len() < before, src)
        }
        ResourceKind::Agent => {
            let src = manifest
                .agents
                .iter()
                .find(|a| a.name.as_str() == args.name)
                .map(|a| a.source.clone());
            let before = manifest.agents.len();
            manifest.agents.retain(|a| a.name.as_str() != args.name);
            (manifest.agents.len() < before, src)
        }
        ResourceKind::Hook => {
            let before = manifest.hooks.len();
            manifest.hooks.retain(|h| h.name.as_str() != args.name);
            (manifest.hooks.len() < before, None)
        }
        ResourceKind::Mcp => {
            let before = manifest.mcp_servers.len();
            manifest
                .mcp_servers
                .retain(|m| m.name.as_str() != args.name);
            (manifest.mcp_servers.len() < before, None)
        }
        ResourceKind::Plugin => {
            let before = manifest.plugins.len();
            manifest.plugins.retain(|p| p.name.as_str() != args.name);
            (manifest.plugins.len() < before, None)
        }
        ResourceKind::Target => {
            return CommandOutcome::RuntimeError(
                "targets cannot be removed; use `lazyagents disable target <name>`".to_owned(),
            );
        }
    };

    if !removed {
        return CommandOutcome::RuntimeError(format!(
            "no {:?} named `{}` found in manifest",
            args.kind, args.name
        ));
    }

    if args.purge_files
        && let Some(rel) = purge_rel
    {
        let full = root.join(".lazyagents").join(&rel);
        if full.is_dir() {
            let _ = fs::remove_dir_all(&full);
        } else if full.is_file() {
            let _ = fs::remove_file(&full);
        }
    }

    if let Err(err) = save_manifest(root, &manifest, args.local) {
        return CommandOutcome::RuntimeError(err.to_string());
    }
    let _ = writeln!(out, "Removed {:?} `{}`.", args.kind, args.name);
    CommandOutcome::Pass
}

pub(super) fn cmd_toggle(
    root: &Path,
    args: &ToggleArgs,
    enabled: bool,
    out: &mut impl Write,
) -> CommandOutcome {
    let mut manifest = match load_single_manifest(root, args.local) {
        Ok(m) => m,
        Err(msg) => return CommandOutcome::RuntimeError(msg),
    };

    let matched = match args.kind {
        ResourceKind::Rule => toggle_in(
            &mut manifest.rules,
            |r| r.id.as_str() == args.name,
            |r| r.enabled = enabled,
        ),
        ResourceKind::Skill => toggle_in(
            &mut manifest.skills,
            |s| s.name.as_str() == args.name,
            |s| s.enabled = enabled,
        ),
        ResourceKind::Agent => toggle_in(
            &mut manifest.agents,
            |a| a.name.as_str() == args.name,
            |a| a.enabled = enabled,
        ),
        ResourceKind::Hook => toggle_in(
            &mut manifest.hooks,
            |h| h.name.as_str() == args.name,
            |h| h.enabled = enabled,
        ),
        ResourceKind::Mcp => toggle_in(
            &mut manifest.mcp_servers,
            |m| m.name.as_str() == args.name,
            |m| m.enabled = enabled,
        ),
        ResourceKind::Plugin => toggle_in(
            &mut manifest.plugins,
            |p| p.name.as_str() == args.name,
            |p| p.enabled = enabled,
        ),
        ResourceKind::Target => match args.name.as_str() {
            "antigravity" => {
                manifest.targets.antigravity.enabled = enabled;
                true
            }
            "claude_code" | "claude-code" => {
                manifest.targets.claude_code.enabled = enabled;
                true
            }
            "codex" => {
                manifest.targets.codex.enabled = enabled;
                true
            }
            "cursor" => {
                manifest.targets.cursor.enabled = enabled;
                true
            }
            "github_copilot" | "github-copilot" => {
                manifest.targets.github_copilot.enabled = enabled;
                true
            }
            _ => false,
        },
        ResourceKind::Section => {
            return CommandOutcome::RuntimeError(
                "sections do not have an enabled flag; use `add section` or `remove section`"
                    .to_owned(),
            );
        }
    };

    if !matched {
        return CommandOutcome::RuntimeError(format!(
            "no {:?} named `{}` found in manifest",
            args.kind, args.name
        ));
    }
    if let Err(err) = save_manifest(root, &manifest, args.local) {
        return CommandOutcome::RuntimeError(err.to_string());
    }

    let state = if enabled { "Enabled" } else { "Disabled" };
    let _ = writeln!(out, "{state} {:?} `{}`.", args.kind, args.name);
    CommandOutcome::Pass
}

fn load_single_manifest(root: &Path, local: bool) -> Result<ProjectManifest, String> {
    let rel = if local {
        LOCAL_MANIFEST_REL_PATH
    } else {
        MANIFEST_REL_PATH
    };
    let path = root.join(rel);
    if !path.is_file() {
        if local {
            return Ok(ProjectManifest::default());
        }
        return Err(format!("`{rel}` not found; run `lazyagents init` first"));
    }
    let raw = fs::read_to_string(&path).map_err(|err| format!("failed to read `{rel}`: {err}"))?;
    ProjectManifest::from_toml_str(&raw).map_err(|err| err.to_string())
}

fn add_section(root: &Path, manifest: &mut ProjectManifest, args: &AddArgs) -> Result<(), String> {
    let section = if let Some(preset) = &args.preset {
        InstructionSection::Preset {
            preset: preset.clone(),
            when: None,
        }
    } else {
        let id = ResourceId::parse(&args.name).map_err(|err| err.to_string())?;
        let rel = format!("sections/{id}.md");
        let full = root.join(".lazyagents").join(&rel);
        if !full.is_file() {
            write_atomic(&full, &format!("## {id}\n\nDocument project rules here.\n"))
                .map_err(|err| err.to_string())?;
        }
        InstructionSection::File {
            file: rel,
            when: None,
        }
    };
    if let Some(first) = manifest.instructions.first_mut() {
        first.sections.push(section);
    } else {
        manifest.instructions.push(InstructionSpec {
            output: "AGENTS.md".to_owned(),
            auto_probe: true,
            sections: vec![section],
        });
    }
    Ok(())
}

fn add_rule(root: &Path, manifest: &mut ProjectManifest, args: &AddArgs) -> Result<(), String> {
    let id = ResourceId::parse(&args.name).map_err(|err| err.to_string())?;
    let rel = format!("rules/{id}.md");
    let full = root.join(".lazyagents").join(&rel);
    if !full.is_file() {
        write_atomic(
            &full,
            &format!("# {id}\n\nDefine scoped rule guidance here.\n"),
        )
        .map_err(|err| err.to_string())?;
    }
    let globs = if args.globs.is_empty() && args.trigger == crate::cli::CliRuleTrigger::Glob {
        vec!["**/*".to_owned()]
    } else {
        args.globs.clone()
    };
    manifest.rules.push(RuleSpec {
        id,
        source: rel,
        trigger: args.trigger.into(),
        globs,
        description: args.description.clone(),
        enabled: true,
    });
    Ok(())
}

fn add_skill(root: &Path, manifest: &mut ProjectManifest, args: &AddArgs) -> Result<(), String> {
    let name = KebabName::parse(&args.name).map_err(|err| err.to_string())?;
    let rel_dir = format!("skills/{name}");
    let skill_md = root.join(".lazyagents").join(&rel_dir).join("SKILL.md");
    if !skill_md.is_file() {
        let desc = args
            .description
            .as_deref()
            .unwrap_or("Describe when the agent should activate this skill.");
        let body =
            format!("---\nname: {name}\ndescription: {desc}\n---\n\n# {name}\n\n1. Step one.\n");
        write_atomic(&skill_md, &body).map_err(|err| err.to_string())?;
    }
    manifest.skills.push(SkillSpec {
        name,
        source: rel_dir,
        enabled: true,
    });
    Ok(())
}

fn add_agent(root: &Path, manifest: &mut ProjectManifest, args: &AddArgs) -> Result<(), String> {
    let name = KebabName::parse(&args.name).map_err(|err| err.to_string())?;
    let rel = format!("agents/{name}.md");
    let full = root.join(".lazyagents").join(&rel);
    if !full.is_file() {
        write_atomic(
            &full,
            &format!("# {name}\n\nDefine this subagent's system prompt and boundaries here.\n"),
        )
        .map_err(|err| err.to_string())?;
    }
    manifest.agents.push(SubagentSpec {
        name,
        source: rel,
        description: args
            .description
            .clone()
            .unwrap_or_else(|| "Specialized project subagent.".to_owned()),
        model: ModelTier::Inherit,
        capabilities: vec![ToolCapability::FileRead, ToolCapability::Search],
        skills: Vec::new(),
        mcp_servers: Vec::new(),
        enabled: true,
        targets: SubagentTargetExt::default(),
    });
    Ok(())
}

fn add_hook(manifest: &mut ProjectManifest, args: &AddArgs) -> Result<(), String> {
    let name = ResourceId::parse(&args.name).map_err(|err| err.to_string())?;
    let event: HookEvent = args.event.into();
    let command = args
        .command
        .clone()
        .unwrap_or_else(|| "lazyagents hook eval git-guard".to_owned());
    let matcher = if event.uses_tool_matcher() {
        Some(HookMatcher::Capability {
            capability: ToolCapability::Shell,
        })
    } else {
        None
    };
    manifest.hooks.push(HookSpec {
        name,
        enabled: true,
        event,
        matcher,
        command,
        timeout_secs: 15,
    });
    Ok(())
}

fn add_mcp(manifest: &mut ProjectManifest, args: &AddArgs) -> Result<(), String> {
    let name = ResourceId::parse(&args.name).map_err(|err| err.to_string())?;
    let transport = if let Some(url) = &args.url {
        McpTransport::StreamableHttp {
            url: url.clone(),
            auth: McpAuthMode::HeaderVar,
            headers: BTreeMap::new(),
        }
    } else {
        let cmd = args.command.clone().ok_or_else(|| {
            "specify `--url <https://...>` or `--command <bin>` for `add mcp`".to_owned()
        })?;
        McpTransport::Stdio {
            command: cmd,
            args: Vec::new(),
            env: BTreeMap::new(),
            cwd: None,
        }
    };
    manifest.mcp_servers.push(McpServerSpec {
        name,
        enabled: true,
        transport,
        variables: Vec::new(),
        tool_loading: McpToolLoading::Eager,
        allowed_tools: Vec::new(),
        denied_tools: Vec::new(),
        risk_surfaces: Vec::new(),
    });
    Ok(())
}

fn add_plugin(manifest: &mut ProjectManifest, args: &AddArgs) -> Result<(), String> {
    let name = KebabName::parse(&args.name).map_err(|err| err.to_string())?;
    manifest.plugins.push(PluginSpec {
        name,
        display_name: None,
        version: Some("0.1.0".to_owned()),
        author: None,
        homepage: None,
        repository: None,
        description: args
            .description
            .clone()
            .unwrap_or_else(|| "Project plugin bundle.".to_owned()),
        logo: None,
        suggested_prompts: Vec::new(),
        enabled: true,
        include_rules: Vec::new(),
        include_skills: Vec::new(),
        include_agents: Vec::new(),
        include_hooks: Vec::new(),
        include_mcp: Vec::new(),
    });
    Ok(())
}

fn toggle_in<T>(
    items: &mut [T],
    predicate: impl Fn(&T) -> bool,
    mutate: impl FnOnce(&mut T),
) -> bool {
    items.iter_mut().find(|x| predicate(x)).is_some_and(|item| {
        mutate(item);
        true
    })
}
