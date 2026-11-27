//! Typed modal form builders and submit appliers for all 8 Antigravity resource views
//! (`Instructions`, `Rules`, `Skills`, `Subagents`, `Hooks`, `McpServers`, `Manifests`, `Plugins`).

mod docs;
mod plugins;
mod runtime;

use la_core::{KebabName, ProjectManifest, ResourceId, ToolCapability};

use crate::action::StudioCommand;
use crate::form::ModalFormState;
use crate::i18n::I18n;
use crate::state::{NavSection, WorkspaceSnapshot};

/// Identifies which resource entity an open [`ModalFormState`] creates or updates on submit.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FormTarget {
    /// Add or edit an `AGENTS.md` / nested Monorepo `InstructionSpec`.
    InstructionPlan {
        /// Existing plan index when editing, or `None` when creating.
        edit_idx: Option<usize>,
    },
    /// Add or edit a modular section inside `manifest.instructions[inst_idx]`.
    InstructionSection {
        /// Owning instruction plan index.
        inst_idx: usize,
        /// Existing section index when editing, or `None` when appending.
        edit_section: Option<usize>,
    },
    /// Add or edit a conditional `.agents/rules/*.md` rule.
    Rule {
        /// Existing rule index when editing, or `None` when creating.
        edit_idx: Option<usize>,
    },
    /// Add or edit a `.agents/skills/<name>/` bundle.
    Skill {
        /// Existing skill index when editing, or `None` when creating.
        edit_idx: Option<usize>,
    },
    /// Add or edit a `.agents/agents/<name>.md` custom subagent.
    Subagent {
        /// Existing subagent index when editing, or `None` when creating.
        edit_idx: Option<usize>,
    },
    /// Add or edit a `.agents/hooks.json` lifecycle hook.
    Hook {
        /// Existing hook index when editing, or `None` when creating.
        edit_idx: Option<usize>,
    },
    /// Add or edit a `.agents/mcp_config.json` MCP server.
    McpServer {
        /// Existing server index when editing, or `None` when creating.
        edit_idx: Option<usize>,
    },
    /// Edit one of the 3 declarative Antigravity JSON index manifests (`0=skills`, `1=rules`, `2=plugins`).
    IndexManifest {
        /// Manifest slot index (`0`, `1`, or `2`).
        kind_idx: usize,
    },
    /// Add or edit a `.agents/plugins/<name>/` project plugin bundle.
    Plugin {
        /// Existing plugin index when editing, or `None` when creating.
        edit_idx: Option<usize>,
    },
}

/// Builds a shadowed modal form for adding a new resource in `nav` (`a` key).
#[must_use]
pub(super) fn build_add_form(
    nav: NavSection,
    list_index: usize,
    manifest: &ProjectManifest,
    i18n: I18n,
) -> Option<(ModalFormState, FormTarget)> {
    match nav {
        NavSection::Instructions => {
            if manifest.instructions.is_empty() {
                Some(docs::build_instruction_plan_form(None, manifest, i18n))
            } else {
                let inst_idx = list_index.min(manifest.instructions.len().saturating_sub(1));
                Some(docs::build_instruction_section_form(
                    inst_idx, None, manifest, i18n,
                ))
            }
        }
        NavSection::Rules => Some(docs::build_rule_form(None, manifest, i18n)),
        NavSection::Skills => Some(docs::build_skill_form(None, manifest, None, i18n)),
        NavSection::Subagents => Some(docs::build_subagent_form(None, manifest, i18n)),
        NavSection::Hooks => Some(runtime::build_hook_form(None, manifest, i18n)),
        NavSection::McpServers => Some(runtime::build_mcp_form(None, manifest, i18n)),
        NavSection::Manifests => {
            let kind_idx = list_index.min(2);
            Some(plugins::build_manifest_form(kind_idx, manifest, i18n))
        }
        NavSection::Plugins => Some(plugins::build_plugin_form(None, manifest, i18n)),
        _ => None,
    }
}

/// Builds a shadowed modal form for adding a new Monorepo `*/AGENTS.md` instruction plan (`A` key).
#[must_use]
pub(super) fn build_add_instruction_plan_form(
    manifest: &ProjectManifest,
    i18n: I18n,
) -> (ModalFormState, FormTarget) {
    docs::build_instruction_plan_form(None, manifest, i18n)
}

/// Builds a shadowed modal form for editing the selected resource in `nav` (`E` key).
#[must_use]
pub(super) fn build_edit_form(
    nav: NavSection,
    list_index: usize,
    section_index: usize,
    workspace: &WorkspaceSnapshot,
    i18n: I18n,
) -> Option<(ModalFormState, FormTarget)> {
    let manifest = &workspace.manifest;
    match nav {
        NavSection::Instructions => {
            let inst = manifest.instructions.get(list_index)?;
            if section_index < inst.sections.len() {
                Some(docs::build_instruction_section_form(
                    list_index,
                    Some(section_index),
                    manifest,
                    i18n,
                ))
            } else {
                Some(docs::build_instruction_plan_form(
                    Some(list_index),
                    manifest,
                    i18n,
                ))
            }
        }
        NavSection::Rules => (list_index < manifest.rules.len())
            .then(|| docs::build_rule_form(Some(list_index), manifest, i18n)),
        NavSection::Skills => {
            let skill = manifest.skills.get(list_index)?;
            let skill_md = workspace
                .source_files
                .get(&format!("{}/SKILL.md", skill.source))
                .map(String::as_str);
            Some(docs::build_skill_form(
                Some(list_index),
                manifest,
                skill_md,
                i18n,
            ))
        }
        NavSection::Subagents => (list_index < manifest.agents.len())
            .then(|| docs::build_subagent_form(Some(list_index), manifest, i18n)),
        NavSection::Hooks => (list_index < manifest.hooks.len())
            .then(|| runtime::build_hook_form(Some(list_index), manifest, i18n)),
        NavSection::McpServers => (list_index < manifest.mcp_servers.len())
            .then(|| runtime::build_mcp_form(Some(list_index), manifest, i18n)),
        NavSection::Manifests => Some(plugins::build_manifest_form(
            list_index.min(2),
            manifest,
            i18n,
        )),
        NavSection::Plugins => (list_index < manifest.plugins.len())
            .then(|| plugins::build_plugin_form(Some(list_index), manifest, i18n)),
        _ => None,
    }
}

/// Applies a validated [`ModalFormState`] to `workspace` and returns a toast summary plus emitted
/// persistence [`StudioCommand`]s. Returns `Err(msg)` to keep the modal open on domain errors.
pub(super) fn apply_form_submit(
    target: FormTarget,
    form: &ModalFormState,
    workspace: &mut WorkspaceSnapshot,
) -> Result<(String, Vec<StudioCommand>), String> {
    let (summary, mut cmds) = match target {
        FormTarget::InstructionPlan { edit_idx } => {
            docs::apply_instruction_plan(edit_idx, form, workspace)?
        }
        FormTarget::InstructionSection {
            inst_idx,
            edit_section,
        } => docs::apply_instruction_section(inst_idx, edit_section, form, workspace)?,
        FormTarget::Rule { edit_idx } => docs::apply_rule(edit_idx, form, workspace)?,
        FormTarget::Skill { edit_idx } => docs::apply_skill(edit_idx, form, workspace)?,
        FormTarget::Subagent { edit_idx } => docs::apply_subagent(edit_idx, form, workspace)?,
        FormTarget::Hook { edit_idx } => runtime::apply_hook(edit_idx, form, workspace)?,
        FormTarget::McpServer { edit_idx } => runtime::apply_mcp(edit_idx, form, workspace)?,
        FormTarget::IndexManifest { kind_idx } => {
            plugins::apply_index_manifest(kind_idx, form, workspace)
        }
        FormTarget::Plugin { edit_idx } => plugins::apply_plugin(edit_idx, form, workspace)?,
    };

    workspace.recompute();
    cmds.push(StudioCommand::SaveManifest(Box::new(
        workspace.manifest.clone(),
    )));
    Ok((summary, cmds))
}

pub(super) fn opt_text(form: &ModalFormState, key: &str) -> Option<String> {
    form.text_value(key)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

pub(super) fn parse_resource_ids(raw: &[String]) -> Result<Vec<ResourceId>, String> {
    raw.iter()
        .map(|s| ResourceId::parse(s).map_err(|err| err.to_string()))
        .collect()
}

pub(super) fn parse_kebab_names(raw: &[String]) -> Result<Vec<KebabName>, String> {
    raw.iter()
        .map(|s| KebabName::parse(s).map_err(|err| err.to_string()))
        .collect()
}

pub(super) fn capability_label(cap: &ToolCapability) -> String {
    match cap {
        ToolCapability::FileRead => "file_read".to_owned(),
        ToolCapability::FileWrite => "file_write".to_owned(),
        ToolCapability::Shell => "shell".to_owned(),
        ToolCapability::Search => "search".to_owned(),
        ToolCapability::WebSearch => "web_search".to_owned(),
        ToolCapability::Browser => "browser".to_owned(),
        ToolCapability::Subagent => "subagent".to_owned(),
        ToolCapability::Custom(s) => s.clone(),
    }
}

pub(super) fn parse_capability(raw: &str) -> ToolCapability {
    match raw.trim().to_ascii_lowercase().as_str() {
        "file_read" => ToolCapability::FileRead,
        "file_write" => ToolCapability::FileWrite,
        "shell" => ToolCapability::Shell,
        "search" => ToolCapability::Search,
        "web_search" => ToolCapability::WebSearch,
        "browser" => ToolCapability::Browser,
        "subagent" => ToolCapability::Subagent,
        _ => ToolCapability::Custom(raw.trim().to_owned()),
    }
}
