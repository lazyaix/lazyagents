//! Importer ingesting existing repository Antigravity configurations into `.lazyagents/`.
//!
//! Discovers `AGENTS.md`, `GEMINI.md`, `.agents/rules/`, `.agents/skills/`, `.agents/agents/`,
//! `.agents/hooks.json`, `.agents/mcp_config.json`, and `.agents/*.json` without modifying originals.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use la_core::manifest::{
    GitOutputMode, HookMatcher, HookSpec, IndexManifestSpec, InstructionFileKind,
    InstructionSection, InstructionSpec, McpAuthMode, McpServerSpec, McpToolLoading, McpTransport,
    McpVariable, ModelTier, RuleSpec, RuleTrigger, SkillSpec, SubagentSpec, SubagentTargetExt,
};
use la_core::{HookEvent, KebabName, ProjectManifest, ResourceId};

use crate::error::RuntimeError;
use crate::prober::probe_repository;
use crate::workspace::{build_initial_manifest, save_manifest};
use crate::writer::write_atomic;

/// Source format selector for `lazyagents import`.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum ImportSource {
    /// Automatically discover Antigravity repository customizations.
    #[default]
    Auto,
    /// Import native Antigravity (`AGENTS.md`, `GEMINI.md`, `.agents/**`).
    Antigravity,
    /// Reserved Claude Code importer (post-V1).
    ClaudeCode,
    /// Reserved `OpenAI` Codex importer (post-V1).
    Codex,
}

/// Result of scanning an existing repository into a `.lazyagents/` Hybrid SSOT draft.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedWorkspace {
    /// Draft canonical manifest reflecting imported items.
    pub manifest: ProjectManifest,
    /// Source files to persist under `.lazyagents/<rel>`.
    pub source_files: BTreeMap<String, String>,
    /// Human-readable summary notes (imported counts and any skipped/unsupported items).
    pub notes: Vec<String>,
}

/// Scans `root` for existing agent customizations and builds an [`ImportedWorkspace`]
/// without writing any files to disk.
///
/// # Errors
///
/// Returns [`RuntimeError::Io`] on filesystem read failures or [`RuntimeError::Core`] if a
/// reserved importer is explicitly requested.
pub fn import_workspace(
    root: &Path,
    source: ImportSource,
) -> Result<ImportedWorkspace, RuntimeError> {
    if matches!(source, ImportSource::ClaudeCode | ImportSource::Codex) {
        return Err(RuntimeError::Core(la_core::CoreError::Compile {
            message: format!(
                "importer `{source:?}` is reserved for M4; use `auto` or `antigravity` in V1"
            ),
        }));
    }

    let facts = probe_repository(root)?;
    let mut manifest = build_initial_manifest(&facts, GitOutputMode::Commit);
    manifest.instructions.clear();

    let mut source_files = BTreeMap::new();
    let mut notes = Vec::new();

    import_root_instructions(root, &mut manifest, &mut source_files, &mut notes)?;
    import_rules(root, &mut manifest, &mut source_files, &mut notes)?;
    import_skills(root, &mut manifest, &mut source_files, &mut notes)?;
    import_subagents(root, &mut manifest, &mut source_files, &mut notes)?;
    import_hooks_json(root, &mut manifest, &mut notes)?;
    import_mcp_config(root, &mut manifest, &mut notes)?;
    import_index_manifests(root, &mut manifest, &mut notes)?;

    if manifest.instructions.is_empty() {
        manifest.instructions.push(InstructionSpec {
            output: "AGENTS.md".to_owned(),
            auto_probe: true,
            sections: vec![InstructionSection::Preset {
                preset: "builtin:engineering/git-workflow".to_owned(),
                when: None,
            }],
        });
    }

    Ok(ImportedWorkspace {
        manifest,
        source_files,
        notes,
    })
}

/// Persists an [`ImportedWorkspace`] atomically into `<root>/.lazyagents/`.
///
/// # Errors
///
/// Returns [`RuntimeError::Io`] or [`RuntimeError::Core`] on serialization or write failure.
pub fn write_imported_workspace(
    root: &Path,
    imported: &ImportedWorkspace,
) -> Result<(), RuntimeError> {
    for (rel, content) in &imported.source_files {
        write_atomic(&root.join(".lazyagents").join(rel), content)?;
    }
    save_manifest(root, &imported.manifest, false)
}

fn import_root_instructions(
    root: &Path,
    manifest: &mut ProjectManifest,
    source_files: &mut BTreeMap<String, String>,
    notes: &mut Vec<String>,
) -> Result<(), RuntimeError> {
    for (filename, kind) in [
        ("AGENTS.md", InstructionFileKind::AgentsMd),
        ("GEMINI.md", InstructionFileKind::GeminiMd),
    ] {
        let full = root.join(filename);
        if !full.is_file() {
            continue;
        }
        let body = fs::read_to_string(&full)
            .map_err(|err| RuntimeError::io(full.display().to_string(), &err))?;
        let rel = "sections/imported-instructions.md".to_owned();
        source_files.insert(rel.clone(), body);
        manifest.targets.antigravity.instruction_file = kind;
        manifest.instructions.push(InstructionSpec {
            output: filename.to_owned(),
            auto_probe: true,
            sections: vec![InstructionSection::File {
                file: rel,
                when: None,
            }],
        });
        notes.push(format!(
            "imported root `{filename}` into `.lazyagents/sections/imported-instructions.md`"
        ));
        break;
    }
    Ok(())
}

fn import_rules(
    root: &Path,
    manifest: &mut ProjectManifest,
    source_files: &mut BTreeMap<String, String>,
    notes: &mut Vec<String>,
) -> Result<(), RuntimeError> {
    let dir = root.join(".agents").join("rules");
    if !dir.is_dir() {
        return Ok(());
    }
    let mut paths = list_sorted_entries(&dir)?;
    paths.retain(|p| p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("md")));

    for path in paths {
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(id) = ResourceId::parse(stem) else {
            notes.push(format!("skipped rule `{stem}`: invalid identifier"));
            continue;
        };
        let raw = fs::read_to_string(&path)
            .map_err(|err| RuntimeError::io(path.display().to_string(), &err))?;
        let (trigger, globs, description, body) = parse_rule_frontmatter(&raw);
        let rel = format!("rules/{stem}.md");
        source_files.insert(rel.clone(), body);
        manifest.rules.push(RuleSpec {
            id,
            source: rel,
            trigger,
            globs,
            description,
            enabled: true,
        });
        notes.push(format!("imported rule `.agents/rules/{stem}.md`"));
    }
    Ok(())
}

fn import_skills(
    root: &Path,
    manifest: &mut ProjectManifest,
    source_files: &mut BTreeMap<String, String>,
    notes: &mut Vec<String>,
) -> Result<(), RuntimeError> {
    let dir = root.join(".agents").join("skills");
    if !dir.is_dir() {
        return Ok(());
    }
    for subdir in list_sorted_entries(&dir)? {
        if !subdir.is_dir() {
            continue;
        }
        let Some(folder) = subdir.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(name) = KebabName::parse(folder) else {
            notes.push(format!(
                "skipped skill `{folder}`: not lowercase kebab-case"
            ));
            continue;
        };
        if !subdir.join("SKILL.md").is_file() {
            notes.push(format!("skipped skill `{folder}`: missing SKILL.md"));
            continue;
        }
        copy_dir_into_map(&subdir, &subdir, &format!("skills/{folder}"), source_files)?;
        manifest.skills.push(SkillSpec {
            name,
            source: format!("skills/{folder}"),
            enabled: true,
        });
        notes.push(format!("imported skill `.agents/skills/{folder}/`"));
    }
    Ok(())
}

fn import_subagents(
    root: &Path,
    manifest: &mut ProjectManifest,
    source_files: &mut BTreeMap<String, String>,
    notes: &mut Vec<String>,
) -> Result<(), RuntimeError> {
    let dir = root.join(".agents").join("agents");
    if !dir.is_dir() {
        return Ok(());
    }
    for path in list_sorted_entries(&dir)? {
        let is_md = path.is_file()
            && path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("md"));
        if !is_md {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(name) = KebabName::parse(stem) else {
            notes.push(format!(
                "skipped subagent `{stem}`: not lowercase kebab-case"
            ));
            continue;
        };
        let raw = fs::read_to_string(&path)
            .map_err(|err| RuntimeError::io(path.display().to_string(), &err))?;
        let (description, body) = split_description_and_body(&raw);
        let rel = format!("agents/{stem}.md");
        source_files.insert(rel.clone(), body);
        manifest.agents.push(SubagentSpec {
            name,
            source: rel,
            description: description.unwrap_or_else(|| format!("Imported subagent `{stem}`.")),
            model: ModelTier::Inherit,
            capabilities: Vec::new(),
            skills: Vec::new(),
            mcp_servers: Vec::new(),
            enabled: true,
            targets: SubagentTargetExt::default(),
        });
        notes.push(format!("imported subagent `.agents/agents/{stem}.md`"));
    }
    Ok(())
}

fn import_hooks_json(
    root: &Path,
    manifest: &mut ProjectManifest,
    notes: &mut Vec<String>,
) -> Result<(), RuntimeError> {
    let path = root.join(".agents").join("hooks.json");
    if !path.is_file() {
        return Ok(());
    }
    let raw = fs::read_to_string(&path)
        .map_err(|err| RuntimeError::io(path.display().to_string(), &err))?;
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(&raw) else {
        notes.push("skipped `.agents/hooks.json`: invalid JSON object".to_owned());
        return Ok(());
    };

    for (name_str, cfg) in map {
        let Some(obj) = cfg.as_object() else {
            continue;
        };
        let enabled = obj
            .get("enabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        let events: [(&str, HookEvent); 6] = [
            ("SessionStart", HookEvent::SessionStart),
            ("PreToolUse", HookEvent::PreToolUse),
            ("PostToolUse", HookEvent::PostToolUse),
            ("PreInvocation", HookEvent::PreInvocation),
            ("PostInvocation", HookEvent::PostInvocation),
            ("Stop", HookEvent::Stop),
        ];

        for (key, event) in events {
            let Some(arr) = obj.get(key).and_then(serde_json::Value::as_array) else {
                continue;
            };
            let Ok(id) = ResourceId::parse(&name_str) else {
                continue;
            };
            if let Some(spec) = extract_first_hook_spec(id, enabled, event, arr) {
                manifest.hooks.push(spec);
                notes.push(format!("imported hook `{name_str}` ({key})"));
            }
        }
    }
    Ok(())
}

fn extract_first_hook_spec(
    name: ResourceId,
    enabled: bool,
    event: HookEvent,
    arr: &[serde_json::Value],
) -> Option<HookSpec> {
    let first = arr.first()?.as_object()?;
    if event.uses_tool_matcher() {
        let matcher = first
            .get("matcher")
            .and_then(serde_json::Value::as_str)
            .map(|r| HookMatcher::Regex {
                regex: r.to_owned(),
            });
        let handler = first.get("hooks")?.as_array()?.first()?.as_object()?;
        let command = handler.get("command")?.as_str()?.to_owned();
        let timeout_secs = handler
            .get("timeout")
            .and_then(serde_json::Value::as_u64)
            .and_then(|v| u32::try_from(v).ok())
            .filter(|&v| v > 0)
            .unwrap_or(30);
        Some(HookSpec {
            name,
            enabled,
            event,
            matcher,
            command,
            timeout_secs,
        })
    } else {
        let command = first.get("command")?.as_str()?.to_owned();
        let timeout_secs = first
            .get("timeout")
            .and_then(serde_json::Value::as_u64)
            .and_then(|v| u32::try_from(v).ok())
            .filter(|&v| v > 0)
            .unwrap_or(30);
        Some(HookSpec {
            name,
            enabled,
            event,
            matcher: None,
            command,
            timeout_secs,
        })
    }
}

fn import_mcp_config(
    root: &Path,
    manifest: &mut ProjectManifest,
    notes: &mut Vec<String>,
) -> Result<(), RuntimeError> {
    let path = root.join(".agents").join("mcp_config.json");
    if !path.is_file() {
        return Ok(());
    }
    let raw = fs::read_to_string(&path)
        .map_err(|err| RuntimeError::io(path.display().to_string(), &err))?;
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) else {
        notes.push("skipped `.agents/mcp_config.json`: invalid JSON".to_owned());
        return Ok(());
    };
    let Some(servers) = val.get("mcpServers").and_then(serde_json::Value::as_object) else {
        return Ok(());
    };

    for (name_str, cfg) in servers {
        let Some(obj) = cfg.as_object() else {
            continue;
        };
        let Ok(name) = ResourceId::parse(name_str) else {
            continue;
        };
        let enabled = !obj
            .get("disabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let variables: Vec<McpVariable> = obj
            .get("variables")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        let transport = if let Some(cmd) = obj.get("command").and_then(serde_json::Value::as_str) {
            let args = obj
                .get("args")
                .and_then(serde_json::Value::as_array)
                .map_or_default(|a| {
                    a.iter()
                        .filter_map(serde_json::Value::as_str)
                        .map(str::to_owned)
                        .collect()
                });
            let env = json_str_map(obj.get("env"));
            let cwd = obj
                .get("cwd")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned);
            McpTransport::Stdio {
                command: cmd.to_owned(),
                args,
                env,
                cwd,
            }
        } else if let Some(url) = obj
            .get("url")
            .or_else(|| obj.get("serverUrl"))
            .and_then(serde_json::Value::as_str)
        {
            let headers = json_str_map(obj.get("headers"));
            if url.ends_with("/sse") {
                McpTransport::LegacySse {
                    url: url.to_owned(),
                    headers,
                }
            } else {
                McpTransport::StreamableHttp {
                    url: url.to_owned(),
                    auth: McpAuthMode::HeaderVar,
                    headers,
                }
            }
        } else {
            notes.push(format!(
                "skipped MCP server `{name_str}`: missing command/url"
            ));
            continue;
        };

        manifest.mcp_servers.push(McpServerSpec {
            name,
            enabled,
            transport,
            variables,
            tool_loading: McpToolLoading::Eager,
            allowed_tools: Vec::new(),
            denied_tools: Vec::new(),
            risk_surfaces: Vec::new(),
        });
        notes.push(format!("imported MCP server `{name_str}`"));
    }
    Ok(())
}

fn import_index_manifests(
    root: &Path,
    manifest: &mut ProjectManifest,
    notes: &mut Vec<String>,
) -> Result<(), RuntimeError> {
    let dir = root.join(".agents");
    for (file, slot) in [
        ("skills.json", &mut manifest.manifests.skills),
        ("rules.json", &mut manifest.manifests.rules),
        ("plugins.json", &mut manifest.manifests.plugins),
    ] {
        let path = dir.join(file);
        if !path.is_file() {
            continue;
        }
        let raw = fs::read_to_string(&path)
            .map_err(|err| RuntimeError::io(path.display().to_string(), &err))?;
        if let Ok(parsed) = serde_json::from_str::<IndexManifestSpec>(&raw) {
            *slot = parsed;
            notes.push(format!("imported `.agents/{file}`"));
        }
    }
    Ok(())
}

fn json_str_map(val: Option<&serde_json::Value>) -> BTreeMap<String, String> {
    let Some(obj) = val.and_then(serde_json::Value::as_object) else {
        return BTreeMap::new();
    };
    obj.iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
        .collect()
}

fn parse_rule_frontmatter(raw: &str) -> (RuleTrigger, Vec<String>, Option<String>, String) {
    let Some(rest) = raw.strip_prefix("---\n") else {
        return (RuleTrigger::AlwaysOn, Vec::new(), None, raw.to_owned());
    };
    let Some((fm, after)) = rest.split_once("\n---\n") else {
        return (RuleTrigger::AlwaysOn, Vec::new(), None, raw.to_owned());
    };
    let mut trigger = RuleTrigger::AlwaysOn;
    let mut globs = Vec::new();
    let mut description = None;

    for line in fm.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("trigger:") {
            trigger = match val.trim() {
                "glob" => RuleTrigger::Glob,
                "model_decision" => RuleTrigger::ModelDecision,
                "manual" => RuleTrigger::Manual,
                _ => RuleTrigger::AlwaysOn,
            };
        } else if let Some(val) = trimmed.strip_prefix("globs:") {
            globs = val
                .split(',')
                .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_owned())
                .filter(|s| !s.is_empty())
                .collect();
        } else if let Some(val) = trimmed.strip_prefix("description:") {
            let clean = val.trim().trim_matches('"').trim_matches('\'');
            if !clean.is_empty() {
                description = Some(clean.to_owned());
            }
        }
    }

    (
        trigger,
        globs,
        description,
        after.trim_start_matches('\n').to_owned(),
    )
}

fn split_description_and_body(raw: &str) -> (Option<String>, String) {
    let Some(rest) = raw.strip_prefix("---\n") else {
        return (None, raw.to_owned());
    };
    let Some((fm, after)) = rest.split_once("\n---\n") else {
        return (None, raw.to_owned());
    };
    let desc = fm.lines().find_map(|line| {
        line.trim()
            .strip_prefix("description:")
            .map(|v| v.trim().trim_matches('"').trim_matches('\'').to_owned())
            .filter(|v| !v.is_empty())
    });
    (desc, after.trim_start_matches('\n').to_owned())
}

fn list_sorted_entries(dir: &Path) -> Result<Vec<std::path::PathBuf>, RuntimeError> {
    let entries =
        fs::read_dir(dir).map_err(|err| RuntimeError::io(dir.display().to_string(), &err))?;
    let mut out = Vec::new();
    for entry_res in entries {
        let entry = entry_res.map_err(|err| RuntimeError::io(dir.display().to_string(), &err))?;
        out.push(entry.path());
    }
    out.sort();
    Ok(out)
}

fn copy_dir_into_map(
    root: &Path,
    dir: &Path,
    prefix: &str,
    out: &mut BTreeMap<String, String>,
) -> Result<(), RuntimeError> {
    for path in list_sorted_entries(dir)? {
        if path.is_dir() {
            copy_dir_into_map(root, &path, prefix, out)?;
        } else if path.is_file()
            && let Ok(rel) = path.strip_prefix(root)
            && let Ok(content) = fs::read_to_string(&path)
        {
            let key = format!("{prefix}/{}", rel.to_string_lossy().replace('\\', "/"));
            out.insert(key, content);
        }
    }
    Ok(())
}
