//! Modal form builders and submit handlers for Markdown-backed Antigravity resources:
//! `Instructions` (`AGENTS.md`), `Rules`, `Skills`, and `Subagents`.

use la_core::manifest::{
    AntigravityAgentExt, InstructionSection, InstructionSpec, ModelTier, RuleSpec, RuleTrigger,
    SkillSpec, SubagentSpec, SubagentTargetExt,
};
use la_core::{KebabName, ProjectManifest, ResourceId, TargetKind, ToolCapability};

use crate::action::StudioCommand;
use crate::form::{FieldValidator, FormField, FormTab, ModalFormState};
use crate::i18n::I18n;
use crate::state::WorkspaceSnapshot;

use super::{
    FormTarget, capability_label, opt_text, parse_capability, parse_kebab_names, parse_resource_ids,
};

pub(super) fn build_instruction_plan_form(
    edit_idx: Option<usize>,
    manifest: &ProjectManifest,
    i18n: I18n,
) -> (ModalFormState, FormTarget) {
    let existing = edit_idx.and_then(|idx| manifest.instructions.get(idx));
    let output = existing.map_or("AGENTS.md", |p| p.output.as_str());
    let auto_probe = existing.is_none_or(|p| p.auto_probe);

    let title = if edit_idx.is_some() {
        i18n.pick("Edit Instruction Plan", "编辑 AGENTS.md 编译计划")
    } else {
        i18n.pick(
            "Add Instruction Plan (Root or Nested)",
            "新建 AGENTS.md 编译计划",
        )
    };
    let form = ModalFormState::new(
        title,
        vec![
            FormField::text(
                "output",
                i18n.pick("Output Path", "输出路径"),
                output,
                "crates/la-tui/AGENTS.md",
                FieldValidator::Required,
            )
            .with_hint(i18n.pick(
                "Workspace-relative path (e.g., AGENTS.md or crates/app/AGENTS.md)",
                "仓库相对路径（如 AGENTS.md 或 crates/app/AGENTS.md）",
            )),
            FormField::toggle(
                "auto_probe",
                i18n.pick("Auto-Probe Facts", "自动探测仓库事实"),
                auto_probe,
            ),
        ],
    );
    (form, FormTarget::InstructionPlan { edit_idx })
}

pub(super) fn build_instruction_section_form(
    inst_idx: usize,
    edit_section: Option<usize>,
    manifest: &ProjectManifest,
    i18n: I18n,
) -> (ModalFormState, FormTarget) {
    let existing = manifest
        .instructions
        .get(inst_idx)
        .and_then(|inst| edit_section.and_then(|s| inst.sections.get(s)));

    let (kind_idx, value, when) = match existing {
        Some(InstructionSection::Preset { preset, when }) => (0, preset.as_str(), when.as_deref()),
        Some(InstructionSection::File { file, when }) => (1, file.as_str(), when.as_deref()),
        Some(InstructionSection::Inline { inline, when }) => (2, inline.as_str(), when.as_deref()),
        None => (0, "builtin:engineering/git-workflow", None),
    };

    let title = if edit_section.is_some() {
        i18n.pick("Edit AGENTS.md Section", "编辑 AGENTS.md 区块")
    } else {
        i18n.pick("Add AGENTS.md Section", "新增 AGENTS.md 区块")
    };
    let form = ModalFormState::new(
        title,
        vec![
            FormField::enum_select(
                "kind",
                i18n.pick("Section Kind", "区块类型"),
                vec!["preset".into(), "file".into(), "inline".into()],
                kind_idx,
            ),
            FormField::text(
                "value",
                i18n.pick("Preset / File / Text", "预设 / 文件 / 内联"),
                value,
                "sections/custom.md",
                FieldValidator::Required,
            )
            .with_hint(i18n.pick(
                "Preset ID (builtin:...), .lazyagents/ relative file, or inline Markdown",
                "内置预设 ID、.lazyagents/ 相对文件路径或简短内联 Markdown",
            )),
            FormField::text(
                "when",
                i18n.pick("When Condition", "条件表达式"),
                when.unwrap_or(""),
                "project.has_rust",
                FieldValidator::None,
            ),
        ],
    );
    (
        form,
        FormTarget::InstructionSection {
            inst_idx,
            edit_section,
        },
    )
}

pub(super) fn build_rule_form(
    edit_idx: Option<usize>,
    manifest: &ProjectManifest,
    i18n: I18n,
) -> (ModalFormState, FormTarget) {
    let existing = edit_idx.and_then(|idx| manifest.rules.get(idx));
    let trigger_idx = existing.map_or(1, |r| match r.trigger {
        RuleTrigger::AlwaysOn => 0,
        RuleTrigger::Glob => 1,
        RuleTrigger::ModelDecision => 2,
        RuleTrigger::Manual => 3,
    });

    let title = if edit_idx.is_some() {
        i18n.pick("Edit Conditional Rule", "编辑条件规则")
    } else {
        i18n.pick("Add Conditional Rule", "新建条件规则")
    };
    let form = ModalFormState::new(
        title,
        vec![
            FormField::text(
                "id",
                i18n.pick("Rule ID", "规则 ID"),
                existing.map_or("", |r| r.id.as_str()),
                "rust-safety",
                FieldValidator::ResourceId,
            ),
            FormField::enum_select(
                "trigger",
                i18n.pick("Trigger", "触发模式"),
                vec![
                    "always_on".into(),
                    "glob".into(),
                    "model_decision".into(),
                    "manual".into(),
                ],
                trigger_idx,
            ),
            FormField::string_list(
                "globs",
                i18n.pick("Globs", "匹配 Globs"),
                existing.map_or_else(|| vec!["**/*.rs".to_owned()], |r| r.globs.clone()),
                "crates/**/*.rs",
                FieldValidator::Required,
            ),
            FormField::text(
                "description",
                i18n.pick("Description", "规则描述"),
                existing
                    .and_then(|r| r.description.as_deref())
                    .unwrap_or(""),
                "Short activation description",
                FieldValidator::None,
            ),
            FormField::text(
                "source",
                i18n.pick("Source File", "源文件路径"),
                existing.map_or("", |r| r.source.as_str()),
                "rules/<id>.md (auto)",
                FieldValidator::None,
            ),
            FormField::toggle(
                "enabled",
                i18n.pick("Enabled", "启用"),
                existing.is_none_or(|r| r.enabled),
            ),
        ],
    );
    (form, FormTarget::Rule { edit_idx })
}

pub(super) fn build_skill_form(
    edit_idx: Option<usize>,
    manifest: &ProjectManifest,
    existing_skill_md: Option<&str>,
    i18n: I18n,
) -> (ModalFormState, FormTarget) {
    let existing = edit_idx.and_then(|idx| manifest.skills.get(idx));
    let parsed_desc = existing_skill_md.and_then(extract_skill_description);

    let title = if edit_idx.is_some() {
        i18n.pick("Edit Skill Bundle", "编辑技能包")
    } else {
        i18n.pick("Add Skill Bundle", "新建技能包")
    };
    let form = ModalFormState::new(
        title,
        vec![
            FormField::text(
                "name",
                i18n.pick("Skill Name", "技能名称"),
                existing.map_or("", |s| s.name.as_str()),
                "release-prep",
                FieldValidator::KebabName,
            )
            .with_hint(i18n.pick(
                "Lowercase kebab-case (must match folder & SKILL.md frontmatter)",
                "小写 kebab-case（必须与目录及 SKILL.md frontmatter 一致）",
            )),
            FormField::text(
                "description",
                i18n.pick("Description", "触发描述"),
                parsed_desc.as_deref().unwrap_or(""),
                "When and how the agent should activate this skill",
                FieldValidator::Required,
            ),
            FormField::text(
                "source",
                i18n.pick("Source Folder", "源码目录"),
                existing.map_or("", |s| s.source.as_str()),
                "skills/<name> (auto)",
                FieldValidator::None,
            ),
            FormField::toggle(
                "enabled",
                i18n.pick("Enabled", "启用"),
                existing.is_none_or(|s| s.enabled),
            ),
        ],
    );
    (form, FormTarget::Skill { edit_idx })
}

pub(super) fn build_subagent_form(
    edit_idx: Option<usize>,
    manifest: &ProjectManifest,
    i18n: I18n,
) -> (ModalFormState, FormTarget) {
    let existing = edit_idx.and_then(|idx| manifest.agents.get(idx));
    let model_idx = existing.map_or(0, |a| match a.model {
        ModelTier::Inherit => 0,
        ModelTier::FlashLite => 1,
        ModelTier::Flash => 2,
        ModelTier::Pro => 3,
    });
    let caps: Vec<String> = existing.map_or_else(
        || vec!["file_read".to_owned(), "search".to_owned()],
        |a| a.capabilities.iter().map(capability_label).collect(),
    );
    let skills: Vec<String> =
        existing.map_or_default(|a| a.skills.iter().map(ToString::to_string).collect());
    let mcps: Vec<String> =
        existing.map_or_default(|a| a.mcp_servers.iter().map(ToString::to_string).collect());

    let mut fields = vec![
        FormField::text(
            "name",
            i18n.pick("Agent Name", "代理名称"),
            existing.map_or("", |a| a.name.as_str()),
            "security-auditor",
            FieldValidator::KebabName,
        ),
        FormField::text(
            "description",
            i18n.pick("Description", "职责描述"),
            existing.map_or("", |a| a.description.as_str()),
            "Read-only security and architecture reviewer",
            FieldValidator::Required,
        ),
        FormField::enum_select(
            "model",
            i18n.pick("Model Tier", "模型档位"),
            vec![
                "inherit".into(),
                "flash_lite".into(),
                "flash".into(),
                "pro".into(),
            ],
            model_idx,
        ),
        FormField::string_list(
            "capabilities",
            i18n.pick("Capabilities", "工具能力"),
            caps,
            "file_read | file_write | shell | search",
            FieldValidator::Required,
        ),
        FormField::string_list(
            "skills",
            i18n.pick("Bound Skills", "绑定技能"),
            skills,
            "pr-author",
            FieldValidator::KebabName,
        ),
        FormField::string_list(
            "mcp_servers",
            i18n.pick("Bound MCPs", "绑定 MCP"),
            mcps,
            "github",
            FieldValidator::ResourceId,
        ),
        FormField::toggle(
            "enabled",
            i18n.pick("Enabled", "启用"),
            existing.is_none_or(|a| a.enabled),
        ),
    ];
    fields.extend(subagent_antigravity_fields(existing, i18n));

    let title = if edit_idx.is_some() {
        i18n.pick("Edit Custom Subagent", "编辑自定义子代理")
    } else {
        i18n.pick("Add Custom Subagent", "新建自定义子代理")
    };
    let form = ModalFormState::new(title, fields).with_tabs(vec![
        FormTab::general(i18n.pick("General", "通用配置")),
        FormTab::for_target("Antigravity", TargetKind::Antigravity),
    ]);
    (form, FormTarget::Subagent { edit_idx })
}

fn subagent_antigravity_fields(existing: Option<&SubagentSpec>, i18n: I18n) -> Vec<FormField> {
    let ext = existing.map(|a| &a.targets.antigravity);
    vec![
        FormField::toggle("main_agent", "mainAgent", ext.is_some_and(|e| e.main_agent)).on_tab(1),
        FormField::toggle("subagent", "subagent", ext.is_none_or(|e| e.subagent)).on_tab(1),
        FormField::text(
            "permission_mode",
            "permissionMode",
            ext.and_then(|e| e.permission_mode.as_deref()).unwrap_or(""),
            "plan",
            FieldValidator::None,
        )
        .on_tab(1),
        FormField::text(
            "command_execution_policy",
            "commandPolicy",
            ext.and_then(|e| e.command_execution_policy.as_deref())
                .unwrap_or(""),
            "sandbox",
            FieldValidator::None,
        )
        .on_tab(1),
        FormField::text(
            "source",
            i18n.pick("Prompt File", "提示词文件"),
            existing.map_or("", |a| a.source.as_str()),
            "agents/<name>.md (auto)",
            FieldValidator::None,
        )
        .on_tab(1),
    ]
}

pub(super) fn apply_instruction_plan(
    edit_idx: Option<usize>,
    form: &ModalFormState,
    workspace: &mut WorkspaceSnapshot,
) -> Result<(String, Vec<StudioCommand>), String> {
    let output = opt_text(form, "output").ok_or_else(|| "Output path is required".to_owned())?;
    let auto_probe = form.bool_value("auto_probe").unwrap_or(true);

    if let Some(idx) = edit_idx {
        let Some(plan) = workspace.manifest.instructions.get_mut(idx) else {
            return Err("Instruction plan no longer exists".to_owned());
        };
        plan.output.clone_from(&output);
        plan.auto_probe = auto_probe;
        Ok((format!("Updated instruction plan `{output}`"), Vec::new()))
    } else {
        if workspace
            .manifest
            .instructions
            .iter()
            .any(|p| p.output == output)
        {
            return Err(format!("Instruction plan `{output}` already exists"));
        }
        workspace.manifest.instructions.push(InstructionSpec {
            output: output.clone(),
            auto_probe,
            sections: vec![InstructionSection::Preset {
                preset: "builtin:engineering/git-workflow".to_owned(),
                when: None,
            }],
        });
        Ok((format!("Added instruction plan `{output}`"), Vec::new()))
    }
}

pub(super) fn apply_instruction_section(
    inst_idx: usize,
    edit_section: Option<usize>,
    form: &ModalFormState,
    workspace: &mut WorkspaceSnapshot,
) -> Result<(String, Vec<StudioCommand>), String> {
    let kind = form.enum_value("kind").unwrap_or("preset");
    let value = opt_text(form, "value").ok_or_else(|| "Section value is required".to_owned())?;
    let when = opt_text(form, "when");

    let mut cmds = Vec::new();
    let section = match kind {
        "file" => {
            if !workspace.source_files.contains_key(&value) {
                let scaffold = format!("# Section ({value})\n\nDocument project rules here.\n");
                workspace
                    .source_files
                    .insert(value.clone(), scaffold.clone());
                cmds.push(StudioCommand::SaveSourceFile {
                    rel_path: value.clone(),
                    content: scaffold,
                });
            }
            InstructionSection::File {
                file: value.clone(),
                when,
            }
        }
        "inline" => InstructionSection::Inline {
            inline: value.clone(),
            when,
        },
        _ => InstructionSection::Preset {
            preset: value.clone(),
            when,
        },
    };

    let Some(plan) = workspace.manifest.instructions.get_mut(inst_idx) else {
        return Err("Target instruction plan not found".to_owned());
    };
    if let Some(sec_idx) = edit_section {
        let Some(slot) = plan.sections.get_mut(sec_idx) else {
            return Err("Instruction section index out of bounds".to_owned());
        };
        *slot = section;
        Ok((format!("Updated section `{value}`"), cmds))
    } else {
        plan.sections.push(section);
        Ok((format!("Added section `{value}`"), cmds))
    }
}

pub(super) fn apply_rule(
    edit_idx: Option<usize>,
    form: &ModalFormState,
    workspace: &mut WorkspaceSnapshot,
) -> Result<(String, Vec<StudioCommand>), String> {
    let id = ResourceId::parse(form.text_value("id").unwrap_or_default())
        .map_err(|err| err.to_string())?;
    let trigger = match form.enum_value("trigger").unwrap_or("glob") {
        "always_on" => RuleTrigger::AlwaysOn,
        "model_decision" => RuleTrigger::ModelDecision,
        "manual" => RuleTrigger::Manual,
        _ => RuleTrigger::Glob,
    };
    let globs = form
        .string_list_values("globs")
        .map_or_default(<[String]>::to_vec);
    if trigger == RuleTrigger::Glob && globs.is_empty() {
        return Err("`glob` trigger requires at least one glob pattern".to_owned());
    }
    let description = opt_text(form, "description");
    let source = opt_text(form, "source").unwrap_or_else(|| format!("rules/{id}.md"));
    let enabled = form.bool_value("enabled").unwrap_or(true);

    let duplicate = workspace
        .manifest
        .rules
        .iter()
        .enumerate()
        .any(|(i, r)| Some(i) != edit_idx && r.id == id);
    if duplicate {
        return Err(format!("Rule `{id}` already exists"));
    }

    let mut cmds = Vec::new();
    if !workspace.source_files.contains_key(&source) {
        let body = format!("# {id}\n\nDefine conditional guidance for `{id}`.\n");
        workspace.source_files.insert(source.clone(), body.clone());
        cmds.push(StudioCommand::SaveSourceFile {
            rel_path: source.clone(),
            content: body,
        });
    }

    let spec = RuleSpec {
        id: id.clone(),
        source,
        trigger,
        globs,
        description,
        enabled,
    };
    if let Some(idx) = edit_idx {
        let Some(slot) = workspace.manifest.rules.get_mut(idx) else {
            return Err("Rule no longer exists".to_owned());
        };
        *slot = spec;
        Ok((format!("Updated rule `{id}`"), cmds))
    } else {
        workspace.manifest.rules.push(spec);
        Ok((format!("Added rule `{id}`"), cmds))
    }
}

pub(super) fn apply_skill(
    edit_idx: Option<usize>,
    form: &ModalFormState,
    workspace: &mut WorkspaceSnapshot,
) -> Result<(String, Vec<StudioCommand>), String> {
    let name = KebabName::parse(form.text_value("name").unwrap_or_default())
        .map_err(|err| err.to_string())?;
    let description = opt_text(form, "description")
        .ok_or_else(|| "Skill description is required for SKILL.md frontmatter".to_owned())?;
    let source = opt_text(form, "source").unwrap_or_else(|| format!("skills/{name}"));
    let enabled = form.bool_value("enabled").unwrap_or(true);

    let duplicate = workspace
        .manifest
        .skills
        .iter()
        .enumerate()
        .any(|(i, s)| Some(i) != edit_idx && s.name == name);
    if duplicate {
        return Err(format!("Skill `{name}` already exists"));
    }

    let mut cmds = Vec::new();
    let skill_md_path = format!("{source}/SKILL.md");
    let updated_body = workspace.source_files.get(&skill_md_path).map_or_else(
        || {
            format!(
                "---\nname: {name}\ndescription: {description}\n---\n\n# {name}\n\n1. Step one.\n"
            )
        },
        |existing| sync_skill_frontmatter(existing, name.as_str(), &description),
    );
    if workspace.source_files.get(&skill_md_path) != Some(&updated_body) {
        workspace
            .source_files
            .insert(skill_md_path.clone(), updated_body.clone());
        cmds.push(StudioCommand::SaveSourceFile {
            rel_path: skill_md_path,
            content: updated_body,
        });
    }

    let spec = SkillSpec {
        name: name.clone(),
        source,
        enabled,
    };
    if let Some(idx) = edit_idx {
        let Some(slot) = workspace.manifest.skills.get_mut(idx) else {
            return Err("Skill no longer exists".to_owned());
        };
        *slot = spec;
        Ok((format!("Updated skill `{name}`"), cmds))
    } else {
        workspace.manifest.skills.push(spec);
        Ok((format!("Added skill `{name}`"), cmds))
    }
}

pub(super) fn apply_subagent(
    edit_idx: Option<usize>,
    form: &ModalFormState,
    workspace: &mut WorkspaceSnapshot,
) -> Result<(String, Vec<StudioCommand>), String> {
    let name = KebabName::parse(form.text_value("name").unwrap_or_default())
        .map_err(|err| err.to_string())?;
    let description = opt_text(form, "description")
        .ok_or_else(|| "Subagent description is required".to_owned())?;
    let model = match form.enum_value("model").unwrap_or("inherit") {
        "flash_lite" => ModelTier::FlashLite,
        "flash" => ModelTier::Flash,
        "pro" => ModelTier::Pro,
        _ => ModelTier::Inherit,
    };
    let capabilities: Vec<ToolCapability> = form
        .string_list_values("capabilities")
        .unwrap_or(&[])
        .iter()
        .map(|s| parse_capability(s))
        .collect();
    let skills = parse_kebab_names(form.string_list_values("skills").unwrap_or(&[]))?;
    let mcp_servers = parse_resource_ids(form.string_list_values("mcp_servers").unwrap_or(&[]))?;
    let source = opt_text(form, "source").unwrap_or_else(|| format!("agents/{name}.md"));

    let duplicate = workspace
        .manifest
        .agents
        .iter()
        .enumerate()
        .any(|(i, a)| Some(i) != edit_idx && a.name == name);
    if duplicate {
        return Err(format!("Subagent `{name}` already exists"));
    }

    let mut cmds = Vec::new();
    if !workspace.source_files.contains_key(&source) {
        let prompt =
            format!("You are `{name}`, a specialized project subagent.\n\n{description}\n");
        workspace
            .source_files
            .insert(source.clone(), prompt.clone());
        cmds.push(StudioCommand::SaveSourceFile {
            rel_path: source.clone(),
            content: prompt,
        });
    }

    let spec = SubagentSpec {
        name: name.clone(),
        source,
        description,
        model,
        capabilities,
        skills,
        mcp_servers,
        enabled: form.bool_value("enabled").unwrap_or(true),
        targets: SubagentTargetExt {
            antigravity: AntigravityAgentExt {
                main_agent: form.bool_value("main_agent").unwrap_or(false),
                subagent: form.bool_value("subagent").unwrap_or(true),
                permission_mode: opt_text(form, "permission_mode"),
                command_execution_policy: opt_text(form, "command_execution_policy"),
            },
        },
    };
    if let Some(idx) = edit_idx {
        let Some(slot) = workspace.manifest.agents.get_mut(idx) else {
            return Err("Subagent no longer exists".to_owned());
        };
        *slot = spec;
        Ok((format!("Updated subagent `{name}`"), cmds))
    } else {
        workspace.manifest.agents.push(spec);
        Ok((format!("Added subagent `{name}`"), cmds))
    }
}

fn extract_skill_description(skill_md: &str) -> Option<String> {
    let rest = skill_md.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    let fm = rest.get(..end)?;
    for line in fm.lines() {
        if let Some(val) = line.trim().strip_prefix("description:") {
            let clean = val.trim().trim_matches('"').trim_matches('\'');
            if !clean.is_empty() {
                return Some(clean.to_owned());
            }
        }
    }
    None
}

fn sync_skill_frontmatter(existing: &str, name: &str, description: &str) -> String {
    let Some(rest) = existing.strip_prefix("---\n") else {
        return format!("---\nname: {name}\ndescription: {description}\n---\n\n{existing}");
    };
    let Some(end) = rest.find("\n---") else {
        return format!("---\nname: {name}\ndescription: {description}\n---\n\n{existing}");
    };
    let fm = rest.get(..end).unwrap_or("");
    let tail = rest.get(end..).unwrap_or("\n---\n");
    let mut updated_lines = Vec::new();
    let mut saw_name = false;
    let mut saw_desc = false;
    for line in fm.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("name:") {
            updated_lines.push(format!("name: {name}"));
            saw_name = true;
        } else if trimmed.starts_with("description:") {
            updated_lines.push(format!("description: {description}"));
            saw_desc = true;
        } else {
            updated_lines.push(line.to_owned());
        }
    }
    if !saw_name {
        updated_lines.insert(0, format!("name: {name}"));
    }
    if !saw_desc {
        updated_lines.push(format!("description: {description}"));
    }
    format!("---\n{}{tail}", updated_lines.join("\n"))
}
