//! Typed in-TUI mutations for the 8 Antigravity resource kinds (`Instructions`, `Rules`,
//! `Skills`, `Subagents`, `Hooks`, `McpServers`, `Manifests`, `Plugins`).

use la_core::ProjectManifest;
use la_core::manifest::{InstructionSection, ModelTier, RuleTrigger};

use crate::state::NavSection;

/// Toggles the `enabled` (or `auto_probe`) flag on the selected resource in `nav`.
/// Returns `true` when `manifest` changed.
pub(super) fn toggle_resource_enabled(
    nav: NavSection,
    index: usize,
    manifest: &mut ProjectManifest,
) -> bool {
    match nav {
        NavSection::Instructions => {
            if let Some(inst) = manifest.instructions.get_mut(index) {
                inst.auto_probe = !inst.auto_probe;
                return true;
            }
            false
        }
        NavSection::Rules => {
            if let Some(rule) = manifest.rules.get_mut(index) {
                rule.enabled = !rule.enabled;
                return true;
            }
            false
        }
        NavSection::Skills => {
            if let Some(skill) = manifest.skills.get_mut(index) {
                skill.enabled = !skill.enabled;
                return true;
            }
            false
        }
        NavSection::Subagents => {
            if let Some(agent) = manifest.agents.get_mut(index) {
                agent.enabled = !agent.enabled;
                return true;
            }
            false
        }
        NavSection::Hooks => {
            if let Some(hook) = manifest.hooks.get_mut(index) {
                hook.enabled = !hook.enabled;
                return true;
            }
            false
        }
        NavSection::McpServers => {
            if let Some(mcp) = manifest.mcp_servers.get_mut(index) {
                mcp.enabled = !mcp.enabled;
                return true;
            }
            false
        }
        NavSection::Plugins => {
            if let Some(plugin) = manifest.plugins.get_mut(index) {
                plugin.enabled = !plugin.enabled;
                return true;
            }
            false
        }
        _ => false,
    }
}

/// Cycles the primary enum property on the selected resource:
/// - `Rules`: `AlwaysOn -> Glob -> ModelDecision -> Manual -> AlwaysOn` (instant budget offload!)
/// - `Subagents`: `Inherit -> FlashLite -> Flash -> Pro -> Inherit`
pub(super) fn cycle_resource_property(
    nav: NavSection,
    index: usize,
    manifest: &mut ProjectManifest,
) -> Option<&'static str> {
    match nav {
        NavSection::Rules => {
            let rule = manifest.rules.get_mut(index)?;
            rule.trigger = match rule.trigger {
                RuleTrigger::AlwaysOn => {
                    if rule.globs.is_empty() {
                        rule.globs.push("**/*".to_owned());
                    }
                    RuleTrigger::Glob
                }
                RuleTrigger::Glob => RuleTrigger::ModelDecision,
                RuleTrigger::ModelDecision => RuleTrigger::Manual,
                RuleTrigger::Manual => RuleTrigger::AlwaysOn,
            };
            Some("Rule trigger cycled")
        }
        NavSection::Subagents => {
            let agent = manifest.agents.get_mut(index)?;
            agent.model = match agent.model {
                ModelTier::Inherit => ModelTier::FlashLite,
                ModelTier::FlashLite => ModelTier::Flash,
                ModelTier::Flash => ModelTier::Pro,
                ModelTier::Pro => ModelTier::Inherit,
            };
            Some("Subagent model tier updated")
        }
        _ => None,
    }
}

/// Moves a section up or down inside the first `AGENTS.md` instruction plan (`J` / `K`).
pub(super) fn reorder_instruction_section(
    inst_idx: usize,
    section_idx: usize,
    up: bool,
    manifest: &mut ProjectManifest,
) -> Option<usize> {
    let inst = manifest.instructions.get_mut(inst_idx)?;
    if inst.sections.len() < 2 || section_idx >= inst.sections.len() {
        return None;
    }
    let target = if up {
        section_idx.checked_sub(1)?
    } else {
        let next = section_idx.saturating_add(1);
        if next >= inst.sections.len() {
            return None;
        }
        next
    };
    inst.sections.swap(section_idx, target);
    Some(target)
}

/// Resolves the `.lazyagents/`-relative editable Markdown file path for the currently selected item.
#[must_use]
pub(super) fn editable_source_path(
    nav: NavSection,
    index: usize,
    section_idx: usize,
    manifest: &ProjectManifest,
) -> Option<(String, String)> {
    match nav {
        NavSection::Instructions => {
            let inst = manifest.instructions.get(index)?;
            if let Some(InstructionSection::File { file, .. }) = inst.sections.get(section_idx) {
                return Some((file.clone(), format!("{} :: {file}", inst.output)));
            }
            None
        }
        NavSection::Rules => manifest
            .rules
            .get(index)
            .map(|r| (r.source.clone(), format!("Rule: {}", r.id))),
        NavSection::Skills => manifest.skills.get(index).map(|s| {
            (
                format!("{}/SKILL.md", s.source),
                format!("Skill: {}", s.name),
            )
        }),
        NavSection::Subagents => manifest
            .agents
            .get(index)
            .map(|a| (a.source.clone(), format!("Subagent: {}", a.name))),
        _ => None,
    }
}
