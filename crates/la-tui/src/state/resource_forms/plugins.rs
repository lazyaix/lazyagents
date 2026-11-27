//! Modal form builders and submit handlers for Antigravity JSON index `Manifests`
//! (`skills.json` / `rules.json` / `plugins.json`) and cohesive project `Plugins`.

use la_core::manifest::{IndexEntry, IndexManifestSpec, PluginSpec};
use la_core::{KebabName, ProjectManifest, TargetKind};

use crate::action::StudioCommand;
use crate::form::{
    FieldValidator, FormField, FormTab, KeyValueEntry, KeyValueListState, ModalFormState,
};
use crate::i18n::I18n;
use crate::state::WorkspaceSnapshot;

use super::{FormTarget, opt_text, parse_kebab_names, parse_resource_ids};

pub(super) fn build_manifest_form(
    kind_idx: usize,
    manifest: &ProjectManifest,
    i18n: I18n,
) -> (ModalFormState, FormTarget) {
    let (file_name, spec) = match kind_idx {
        1 => ("rules.json", &manifest.manifests.rules),
        2 => ("plugins.json", &manifest.manifests.plugins),
        _ => ("skills.json", &manifest.manifests.skills),
    };
    let entries = index_entries_to_kv(&spec.entries);
    let inherits = index_entries_to_kv(&spec.inherits);

    let form = ModalFormState::new(
        format!(
            "{} {file_name}",
            i18n.pick("Edit Index Manifest", "编辑索引清单")
        ),
        vec![
            FormField::kv_list(
                "entries",
                i18n.pick("Directory Entries", "扫描目录条目"),
                KeyValueListState::new(
                    entries,
                    "Path",
                    "include_only (csv)",
                    Some("exclude (csv)".to_owned()),
                    false,
                ),
            ),
            FormField::kv_list(
                "inherits",
                i18n.pick("Inherited Manifests", "继承清单文件"),
                KeyValueListState::new(
                    inherits,
                    "Manifest Path",
                    "include_only (csv)",
                    Some("exclude (csv)".to_owned()),
                    false,
                ),
            )
            .on_tab(1),
        ],
    )
    .with_tabs(vec![
        FormTab::general(i18n.pick("Entries", "目录条目")),
        FormTab::for_target(i18n.pick("Inherits", "继承链"), TargetKind::Antigravity),
    ]);
    (form, FormTarget::IndexManifest { kind_idx })
}

pub(super) fn build_plugin_form(
    edit_idx: Option<usize>,
    manifest: &ProjectManifest,
    i18n: I18n,
) -> (ModalFormState, FormTarget) {
    let existing = edit_idx.and_then(|idx| manifest.plugins.get(idx));
    let title = if edit_idx.is_some() {
        i18n.pick("Edit Project Plugin", "编辑项目插件包")
    } else {
        i18n.pick("Add Project Plugin", "新建项目插件包")
    };
    let mut fields = plugin_metadata_fields(existing, i18n);
    fields.extend(plugin_bundle_fields(existing, i18n));

    let form = ModalFormState::new(title, fields).with_tabs(vec![
        FormTab::for_target(i18n.pick("Metadata", "元数据"), TargetKind::Antigravity),
        FormTab::general(i18n.pick("Bundle Contents", "打包内容")),
    ]);
    (form, FormTarget::Plugin { edit_idx })
}

fn plugin_metadata_fields(existing: Option<&PluginSpec>, i18n: I18n) -> Vec<FormField> {
    vec![
        FormField::text(
            "name",
            i18n.pick("Plugin Name", "插件名称"),
            existing.map_or("", |p| p.name.as_str()),
            "team-kit",
            FieldValidator::KebabName,
        ),
        FormField::text(
            "display_name",
            i18n.pick("Display Name", "展示名称"),
            existing
                .and_then(|p| p.display_name.as_deref())
                .unwrap_or(""),
            "Team Governance Kit",
            FieldValidator::None,
        ),
        FormField::text(
            "version",
            i18n.pick("Version", "版本号"),
            existing
                .and_then(|p| p.version.as_deref())
                .unwrap_or("0.1.0"),
            "0.1.0",
            FieldValidator::None,
        ),
        FormField::text(
            "description",
            i18n.pick("Description", "简介"),
            existing.map_or("", |p| p.description.as_str()),
            "Cohesive repository customization bundle",
            FieldValidator::Required,
        ),
        FormField::text(
            "author",
            i18n.pick("Author", "作者"),
            existing.and_then(|p| p.author.as_deref()).unwrap_or(""),
            "LazyAIX",
            FieldValidator::None,
        ),
        FormField::text(
            "homepage",
            i18n.pick("Homepage", "主页 URL"),
            existing.and_then(|p| p.homepage.as_deref()).unwrap_or(""),
            "https://github.com/lazyaix/lazyagents",
            FieldValidator::HttpsUrl,
        ),
        FormField::text(
            "repository",
            i18n.pick("Repository", "仓库 URL"),
            existing.and_then(|p| p.repository.as_deref()).unwrap_or(""),
            "https://github.com/lazyaix/lazyagents",
            FieldValidator::HttpsUrl,
        ),
        FormField::text(
            "logo",
            i18n.pick("Logo Asset", "Logo 相对路径"),
            existing.and_then(|p| p.logo.as_deref()).unwrap_or(""),
            "assets/logo.png",
            FieldValidator::None,
        ),
        FormField::string_list(
            "suggested_prompts",
            i18n.pick("Starter Prompts", "推荐提示词"),
            existing.map_or_else(Vec::new, |p| p.suggested_prompts.clone()),
            "Review my PR",
            FieldValidator::Required,
        ),
        FormField::toggle(
            "enabled",
            i18n.pick("Enabled", "启用"),
            existing.is_none_or(|p| p.enabled),
        ),
    ]
}

fn plugin_bundle_fields(existing: Option<&PluginSpec>, i18n: I18n) -> Vec<FormField> {
    vec![
        FormField::string_list(
            "include_rules",
            i18n.pick("Bundle Rules", "打包规则 ID"),
            existing.map_or_default(|p| p.include_rules.iter().map(ToString::to_string).collect()),
            "rust-safety",
            FieldValidator::ResourceId,
        )
        .on_tab(1),
        FormField::string_list(
            "include_skills",
            i18n.pick("Bundle Skills", "打包技能"),
            existing.map_or_default(|p| p.include_skills.iter().map(ToString::to_string).collect()),
            "pr-author",
            FieldValidator::KebabName,
        )
        .on_tab(1),
        FormField::string_list(
            "include_agents",
            i18n.pick("Bundle Subagents", "打包子代理"),
            existing.map_or_default(|p| p.include_agents.iter().map(ToString::to_string).collect()),
            "auditor",
            FieldValidator::KebabName,
        )
        .on_tab(1),
        FormField::string_list(
            "include_hooks",
            i18n.pick("Bundle Hooks", "打包钩子"),
            existing.map_or_default(|p| p.include_hooks.iter().map(ToString::to_string).collect()),
            "lazyagents-git-guard",
            FieldValidator::ResourceId,
        )
        .on_tab(1),
        FormField::string_list(
            "include_mcp",
            i18n.pick("Bundle MCPs", "打包 MCP"),
            existing.map_or_default(|p| p.include_mcp.iter().map(ToString::to_string).collect()),
            "github",
            FieldValidator::ResourceId,
        )
        .on_tab(1),
    ]
}

pub(super) fn apply_index_manifest(
    kind_idx: usize,
    form: &ModalFormState,
    workspace: &mut WorkspaceSnapshot,
) -> (String, Vec<StudioCommand>) {
    let entries = kv_to_index_entries(form.kv_entries("entries").unwrap_or(&[]));
    let inherits = kv_to_index_entries(form.kv_entries("inherits").unwrap_or(&[]));
    let updated = IndexManifestSpec { inherits, entries };
    let label = match kind_idx {
        1 => {
            workspace.manifest.manifests.rules = updated;
            "rules.json"
        }
        2 => {
            workspace.manifest.manifests.plugins = updated;
            "plugins.json"
        }
        _ => {
            workspace.manifest.manifests.skills = updated;
            "skills.json"
        }
    };
    (format!("Updated `{label}` index manifest"), Vec::new())
}

pub(super) fn apply_plugin(
    edit_idx: Option<usize>,
    form: &ModalFormState,
    workspace: &mut WorkspaceSnapshot,
) -> Result<(String, Vec<StudioCommand>), String> {
    let name = KebabName::parse(form.text_value("name").unwrap_or_default())
        .map_err(|err| err.to_string())?;
    let description =
        opt_text(form, "description").ok_or_else(|| "Plugin description is required".to_owned())?;
    let duplicate = workspace
        .manifest
        .plugins
        .iter()
        .enumerate()
        .any(|(i, p)| Some(i) != edit_idx && p.name == name);
    if duplicate {
        return Err(format!("Plugin `{name}` already exists"));
    }

    let logo = opt_text(form, "logo");
    let include_rules =
        parse_resource_ids(form.string_list_values("include_rules").unwrap_or(&[]))?;
    let include_skills =
        parse_kebab_names(form.string_list_values("include_skills").unwrap_or(&[]))?;
    let include_agents =
        parse_kebab_names(form.string_list_values("include_agents").unwrap_or(&[]))?;
    let include_hooks =
        parse_resource_ids(form.string_list_values("include_hooks").unwrap_or(&[]))?;
    let include_mcp = parse_resource_ids(form.string_list_values("include_mcp").unwrap_or(&[]))?;
    validate_plugin_refs(
        &workspace.manifest,
        &include_rules,
        &include_skills,
        &include_agents,
    )?;

    let mut cmds = Vec::new();
    if let Some(rel_path) = &logo {
        cmds.push(StudioCommand::LoadPluginLogo {
            plugin_name: name.as_str().to_owned(),
            rel_path: rel_path.clone(),
        });
    }

    let spec = PluginSpec {
        name: name.clone(),
        display_name: opt_text(form, "display_name"),
        version: opt_text(form, "version"),
        author: opt_text(form, "author"),
        homepage: opt_text(form, "homepage"),
        repository: opt_text(form, "repository"),
        description,
        logo,
        suggested_prompts: form
            .string_list_values("suggested_prompts")
            .map_or_default(<[String]>::to_vec),
        enabled: form.bool_value("enabled").unwrap_or(true),
        include_rules,
        include_skills,
        include_agents,
        include_hooks,
        include_mcp,
    };

    if let Some(idx) = edit_idx {
        let Some(slot) = workspace.manifest.plugins.get_mut(idx) else {
            return Err("Plugin no longer exists".to_owned());
        };
        *slot = spec;
        Ok((format!("Updated plugin `{name}`"), cmds))
    } else {
        workspace.manifest.plugins.push(spec);
        Ok((format!("Added plugin `{name}`"), cmds))
    }
}

fn validate_plugin_refs(
    manifest: &ProjectManifest,
    include_rules: &[la_core::ResourceId],
    include_skills: &[KebabName],
    include_agents: &[KebabName],
) -> Result<(), String> {
    for id in include_rules {
        if !manifest.rules.iter().any(|r| r.id == *id) {
            return Err(format!(
                "Referenced rule `{id}` is not declared in manifest"
            ));
        }
    }
    for name in include_skills {
        if !manifest.skills.iter().any(|s| s.name == *name) {
            return Err(format!(
                "Referenced skill `{name}` is not declared in manifest"
            ));
        }
    }
    for name in include_agents {
        if !manifest.agents.iter().any(|a| a.name == *name) {
            return Err(format!(
                "Referenced subagent `{name}` is not declared in manifest"
            ));
        }
    }
    Ok(())
}

fn index_entries_to_kv(entries: &[IndexEntry]) -> Vec<KeyValueEntry> {
    entries
        .iter()
        .map(|e| KeyValueEntry {
            key: e.path.clone(),
            value: e.include_only.join(", "),
            extra: Some(e.exclude.join(", ")),
        })
        .collect()
}

fn kv_to_index_entries(entries: &[KeyValueEntry]) -> Vec<IndexEntry> {
    entries
        .iter()
        .map(|e| IndexEntry {
            path: e.key.clone(),
            include_only: split_csv(&e.value),
            exclude: e.extra.as_deref().map_or_default(split_csv),
        })
        .collect()
}

fn split_csv(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}
