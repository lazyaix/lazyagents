//! Interactive state for the shadowed Delete Confirmation modal (`ConfirmDeleteState`)
//! and the offline Built-in Catalog & Preset Picker modal (`CatalogPickerState`).

use la_core::catalog::{builtin_github_mcp_preset, builtin_skill_template, list_section_presets};
use la_core::engineering::GitHubScaffold;
use la_core::manifest::{InstructionSection, InstructionSpec};
use la_core::{KebabName, ProjectManifest};

use crate::action::StudioCommand;
use crate::state::{NavSection, WorkspaceSnapshot};
use crate::text::fuzzy_match;

/// Specific manifest item queued for deletion inside [`ConfirmDeleteState`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DeleteTarget {
    /// Remove a section from `manifest.instructions[inst_idx]`.
    InstructionSection {
        /// Instruction plan index.
        inst_idx: usize,
        /// Section index within that plan.
        section_idx: usize,
    },
    /// Remove `manifest.rules[index]`.
    Rule(usize),
    /// Remove `manifest.skills[index]`.
    Skill(usize),
    /// Remove `manifest.agents[index]`.
    Subagent(usize),
    /// Remove `manifest.hooks[index]`.
    Hook(usize),
    /// Remove `manifest.mcp_servers[index]`.
    McpServer(usize),
    /// Remove `manifest.plugins[index]`.
    Plugin(usize),
}

/// Interactive state for the shadowed Delete Confirmation modal (`d` key).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfirmDeleteState {
    /// Human-readable resource kind label (e.g., `"Rule"`, `"Skill"`, `"MCP Server"`).
    pub resource_kind: String,
    /// Target item identifier or label shown prominently in the confirmation modal.
    pub item_label: String,
    /// Optional `.lazyagents/`-relative backing source file or directory path.
    pub backing_path: Option<String>,
    /// Whether confirming should also delete `backing_path` from disk (`--purge-files`).
    pub purge_source_file: bool,
    /// Canonical delete target executed on confirmation.
    pub target: DeleteTarget,
}

/// Category filter pill inside the Built-in Catalog Picker modal (`C` key).
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum CatalogCategory {
    /// Show every offline catalog entry.
    #[default]
    All,
    /// Built-in `AGENTS.md` instruction section presets.
    Presets,
    /// Built-in Git/GitHub collaboration skills.
    Skills,
    /// Built-in Streamable HTTP MCP presets.
    Mcp,
    /// Opt-in `.github/*` governance scaffolds.
    Scaffolds,
}

/// Typed installable catalog entry payload.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CatalogEntryKind {
    /// Built-in `AGENTS.md` section preset ID.
    SectionPreset(&'static str),
    /// Built-in Git/GitHub collaboration skill name.
    BuiltinSkill(&'static str),
    /// Built-in Streamable HTTP GitHub MCP server preset.
    GitHubMcp,
    /// Opt-in `.github/*` governance scaffold.
    GitHubScaffold(GitHubScaffold),
}

/// Single browseable item inside the offline Built-in Catalog Picker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogItem {
    /// Canonical identifier or display name.
    pub id: String,
    /// Short human-readable summary.
    pub summary: String,
    /// Category bucket for filtering and badge color.
    pub category: CatalogCategory,
    /// Installable payload action.
    pub kind: CatalogEntryKind,
    /// Whether this item is already active in the current project manifest.
    pub installed: bool,
}

/// Fuzzy-filtered catalog row with match score and highlight indices.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilteredCatalogEntry {
    /// Underlying catalog item.
    pub item: CatalogItem,
    /// Fuzzy score.
    pub score: i32,
    /// Matched character offsets in `item.id`.
    pub matched_indices: Vec<usize>,
}

/// Interactive state for the shadowed Built-in Catalog & Preset Picker modal (`C` key).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CatalogPickerState {
    /// Live fuzzy search query.
    pub query: String,
    /// Active category filter pill.
    pub category: CatalogCategory,
    /// Highlighted row index within filtered results.
    pub selected: usize,
}

impl ConfirmDeleteState {
    /// Builds a [`ConfirmDeleteState`] from the currently highlighted item in `nav`, if deletable.
    #[must_use]
    pub fn from_selection(
        nav: NavSection,
        list_index: usize,
        section_index: usize,
        manifest: &ProjectManifest,
    ) -> Option<Self> {
        match nav {
            NavSection::Instructions => {
                let inst = manifest.instructions.get(list_index)?;
                let sec = inst.sections.get(section_index)?;
                let (label, backing_path) = match sec {
                    InstructionSection::Preset { preset, .. } => (preset.clone(), None),
                    InstructionSection::File { file, .. } => (file.clone(), Some(file.clone())),
                    InstructionSection::Inline { inline, .. } => (
                        inline
                            .lines()
                            .next()
                            .unwrap_or("<inline section>")
                            .to_owned(),
                        None,
                    ),
                };
                Some(Self {
                    resource_kind: "Instruction Section".to_owned(),
                    item_label: label,
                    backing_path,
                    purge_source_file: false,
                    target: DeleteTarget::InstructionSection {
                        inst_idx: list_index,
                        section_idx: section_index,
                    },
                })
            }
            NavSection::Rules => manifest.rules.get(list_index).map(|r| Self {
                resource_kind: "Rule".to_owned(),
                item_label: r.id.to_string(),
                backing_path: Some(r.source.clone()),
                purge_source_file: false,
                target: DeleteTarget::Rule(list_index),
            }),
            NavSection::Skills => manifest.skills.get(list_index).map(|s| Self {
                resource_kind: "Skill".to_owned(),
                item_label: s.name.to_string(),
                backing_path: Some(s.source.clone()),
                purge_source_file: false,
                target: DeleteTarget::Skill(list_index),
            }),
            NavSection::Subagents => manifest.agents.get(list_index).map(|a| Self {
                resource_kind: "Subagent".to_owned(),
                item_label: a.name.to_string(),
                backing_path: Some(a.source.clone()),
                purge_source_file: false,
                target: DeleteTarget::Subagent(list_index),
            }),
            NavSection::Hooks => manifest.hooks.get(list_index).map(|h| Self {
                resource_kind: "Hook".to_owned(),
                item_label: h.name.to_string(),
                backing_path: None,
                purge_source_file: false,
                target: DeleteTarget::Hook(list_index),
            }),
            NavSection::McpServers => manifest.mcp_servers.get(list_index).map(|m| Self {
                resource_kind: "MCP Server".to_owned(),
                item_label: m.name.to_string(),
                backing_path: None,
                purge_source_file: false,
                target: DeleteTarget::McpServer(list_index),
            }),
            NavSection::Plugins => manifest.plugins.get(list_index).map(|p| Self {
                resource_kind: "Plugin".to_owned(),
                item_label: p.name.to_string(),
                backing_path: None,
                purge_source_file: false,
                target: DeleteTarget::Plugin(list_index),
            }),
            _ => None,
        }
    }

    /// Executes the deletion against `workspace` and returns emitted [`StudioCommand`]s.
    pub fn apply(self, workspace: &mut WorkspaceSnapshot) -> Vec<StudioCommand> {
        let mut removed = false;
        match self.target {
            DeleteTarget::InstructionSection {
                inst_idx,
                section_idx,
            } => {
                if let Some(inst) = workspace.manifest.instructions.get_mut(inst_idx)
                    && section_idx < inst.sections.len()
                {
                    inst.sections.remove(section_idx);
                    removed = true;
                }
            }
            DeleteTarget::Rule(idx) if idx < workspace.manifest.rules.len() => {
                workspace.manifest.rules.remove(idx);
                removed = true;
            }
            DeleteTarget::Skill(idx) if idx < workspace.manifest.skills.len() => {
                workspace.manifest.skills.remove(idx);
                removed = true;
            }
            DeleteTarget::Subagent(idx) if idx < workspace.manifest.agents.len() => {
                workspace.manifest.agents.remove(idx);
                removed = true;
            }
            DeleteTarget::Hook(idx) if idx < workspace.manifest.hooks.len() => {
                workspace.manifest.hooks.remove(idx);
                removed = true;
            }
            DeleteTarget::McpServer(idx) if idx < workspace.manifest.mcp_servers.len() => {
                workspace.manifest.mcp_servers.remove(idx);
                removed = true;
            }
            DeleteTarget::Plugin(idx) if idx < workspace.manifest.plugins.len() => {
                workspace.manifest.plugins.remove(idx);
                removed = true;
            }
            _ => {}
        }

        if !removed {
            return Vec::new();
        }

        let mut cmds = Vec::new();
        if self.purge_source_file
            && let Some(rel_path) = self.backing_path
        {
            let prefix = format!("{}/", rel_path.trim_end_matches('/'));
            workspace
                .source_files
                .retain(|k, _| k != &rel_path && !k.starts_with(&prefix));
            cmds.push(StudioCommand::DeleteSourceFile { rel_path });
        }

        workspace.recompute();
        cmds.push(StudioCommand::SaveManifest(Box::new(
            workspace.manifest.clone(),
        )));
        cmds
    }
}

impl CatalogCategory {
    /// Cycles to the next category filter pill.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::All => Self::Presets,
            Self::Presets => Self::Skills,
            Self::Skills => Self::Mcp,
            Self::Mcp => Self::Scaffolds,
            Self::Scaffolds => Self::All,
        }
    }

    /// Short uppercase badge tag for UI rendering.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::Presets => "PRESET",
            Self::Skills => "SKILL",
            Self::Mcp => "MCP",
            Self::Scaffolds => "GITHUB",
        }
    }
}

impl CatalogPickerState {
    /// Resets query, category filter, and selection cursor when opening the picker.
    pub fn reset(&mut self) {
        self.query.clear();
        self.category = CatalogCategory::All;
        self.selected = 0;
    }

    /// Returns fuzzy-matched catalog items reflecting current installation status in `manifest`.
    #[must_use]
    pub fn filtered(&self, manifest: &ProjectManifest) -> Vec<FilteredCatalogEntry> {
        let mut items: Vec<FilteredCatalogEntry> = all_catalog_items(manifest)
            .into_iter()
            .filter(|item| self.category == CatalogCategory::All || item.category == self.category)
            .filter_map(|item| {
                let haystack = format!("{} {}", item.id, item.summary);
                fuzzy_match(&self.query, &item.id)
                    .or_else(|| {
                        fuzzy_match(&self.query, &haystack).map(|m| crate::text::FuzzyMatch {
                            score: m.score.saturating_sub(10),
                            matched_indices: Vec::new(),
                        })
                    })
                    .map(|m| FilteredCatalogEntry {
                        item,
                        score: m.score,
                        matched_indices: m.matched_indices,
                    })
            })
            .collect();
        if !self.query.trim().is_empty() {
            items.sort_by(|a, b| {
                b.score
                    .cmp(&a.score)
                    .then_with(|| a.item.id.cmp(&b.item.id))
            });
        }
        items
    }

    /// Installs the highlighted catalog item into `manifest` and returns a status message if mutated.
    pub fn install_selected(&self, manifest: &mut ProjectManifest) -> Option<String> {
        let matches = self.filtered(manifest);
        let chosen = matches.get(self.selected)?;
        install_catalog_entry(chosen.item.kind, manifest)
    }
}

/// Builds the complete list of offline catalog items with live `installed` badges.
#[must_use]
pub fn all_catalog_items(manifest: &ProjectManifest) -> Vec<CatalogItem> {
    let mut items = Vec::new();

    for preset in list_section_presets() {
        let installed = manifest.instructions.iter().any(|inst| {
            inst.sections.iter().any(
                |s| matches!(s, InstructionSection::Preset { preset: p, .. } if p == preset.id),
            )
        });
        items.push(CatalogItem {
            id: preset.id.to_owned(),
            summary: preset.summary.to_owned(),
            category: CatalogCategory::Presets,
            kind: CatalogEntryKind::SectionPreset(preset.id),
            installed,
        });
    }

    for skill_name in [
        "worktree-isolation",
        "pr-author",
        "ci-remediation",
        "stacked-pr",
        "release-prep",
    ] {
        if let Some(tpl) = builtin_skill_template(skill_name) {
            let installed = manifest
                .engineering
                .github
                .enable_skills
                .iter()
                .any(|s| s.as_str() == skill_name)
                || manifest
                    .skills
                    .iter()
                    .any(|s| s.name.as_str() == skill_name)
                || (skill_name == "worktree-isolation"
                    && manifest.engineering.git.worktree.enabled);
            items.push(CatalogItem {
                id: format!("skill:{skill_name}"),
                summary: tpl.description.to_owned(),
                category: CatalogCategory::Skills,
                kind: CatalogEntryKind::BuiltinSkill(skill_name),
                installed,
            });
        }
    }

    let gh_mcp_installed = manifest.engineering.github.enable_mcp_preset
        || manifest
            .mcp_servers
            .iter()
            .any(|s| s.name.as_str() == "github");
    items.push(CatalogItem {
        id: "mcp:github".to_owned(),
        summary:
            "Streamable HTTP GitHub MCP server preset (variables-only, zero plaintext secrets)"
                .to_owned(),
        category: CatalogCategory::Mcp,
        kind: CatalogEntryKind::GitHubMcp,
        installed: gh_mcp_installed,
    });

    let scaffolds = [
        (
            GitHubScaffold::PrTitle,
            "scaffold:pr-title",
            "Conventional Commits PR title check workflow (SHA-pinned)",
        ),
        (
            GitHubScaffold::SecurityZizmor,
            "scaffold:security-zizmor",
            "zizmor GitHub Actions security audit workflow",
        ),
        (
            GitHubScaffold::Dependabot,
            "scaffold:dependabot",
            "Weekly Dependabot config with publish cooldown",
        ),
        (
            GitHubScaffold::IssueTemplates,
            "scaffold:issue-templates",
            "Structured YAML bug report issue template",
        ),
        (
            GitHubScaffold::Contributing,
            "scaffold:contributing",
            "CONTRIBUTING.md synced with branch & verify policy",
        ),
        (
            GitHubScaffold::SecurityPolicy,
            "scaffold:security-policy",
            "SECURITY.md aligned with private advisories & supply-chain rules",
        ),
    ];
    for (scaffold, id, summary) in scaffolds {
        let installed = manifest.engineering.github.scaffolds.contains(&scaffold);
        items.push(CatalogItem {
            id: id.to_owned(),
            summary: summary.to_owned(),
            category: CatalogCategory::Scaffolds,
            kind: CatalogEntryKind::GitHubScaffold(scaffold),
            installed,
        });
    }

    items
}

fn install_catalog_entry(kind: CatalogEntryKind, manifest: &mut ProjectManifest) -> Option<String> {
    match kind {
        CatalogEntryKind::SectionPreset(preset_id) => {
            let exists = manifest.instructions.iter().any(|inst| {
                inst.sections.iter().any(|s| {
                    matches!(s, InstructionSection::Preset { preset, .. } if preset == preset_id)
                })
            });
            if exists {
                return None;
            }
            let section = InstructionSection::Preset {
                preset: preset_id.to_owned(),
                when: None,
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
            Some(format!("Added section preset `{preset_id}`"))
        }
        CatalogEntryKind::BuiltinSkill(skill_name) => {
            if skill_name == "worktree-isolation" {
                if manifest.engineering.git.worktree.enabled {
                    return None;
                }
                manifest.engineering.git.worktree.enabled = true;
                return Some("Enabled built-in skill `worktree-isolation`".to_owned());
            }
            let kebab = KebabName::parse(skill_name).ok()?;
            manifest.engineering.github.enabled = true;
            if manifest.engineering.github.enable_skills.contains(&kebab) {
                return None;
            }
            manifest.engineering.github.enable_skills.push(kebab);
            Some(format!("Enabled built-in skill `{skill_name}`"))
        }
        CatalogEntryKind::GitHubMcp => {
            if manifest
                .mcp_servers
                .iter()
                .any(|m| m.name.as_str() == "github")
                || manifest.engineering.github.enable_mcp_preset
            {
                return None;
            }
            manifest.mcp_servers.push(builtin_github_mcp_preset());
            Some("Installed Streamable HTTP `github` MCP preset".to_owned())
        }
        CatalogEntryKind::GitHubScaffold(scaffold) => {
            manifest.engineering.github.enabled = true;
            if manifest.engineering.github.scaffolds.contains(&scaffold) {
                return None;
            }
            manifest.engineering.github.scaffolds.push(scaffold);
            Some(format!("Enabled GitHub scaffold `{scaffold:?}`"))
        }
    }
}
