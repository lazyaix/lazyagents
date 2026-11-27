//! Command Deck navigation, focus cycling, mouse hit-testing, and scroll movement
//! helpers extracted from [`crate::state::AppState`].

use ratatui::layout::Rect;

use crate::action::Action;
use crate::i18n::I18n;
use crate::state::governance::governance_rows;
use crate::state::{AppState, DiffStyle, FocusPane, InspectorTab, ModalKind, NavSection};
use crate::theme::Theme;

impl NavSection {
    /// Complete ordered list of all 12 Command Deck navigation sections.
    pub const ALL: [Self; 12] = [
        Self::Dashboard,
        Self::Engineering,
        Self::GitHubAudit,
        Self::Instructions,
        Self::Rules,
        Self::Skills,
        Self::Subagents,
        Self::Hooks,
        Self::McpServers,
        Self::Manifests,
        Self::Plugins,
        Self::SyncDiff,
    ];

    /// Advances to the next section with wrap-around.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Dashboard => Self::Engineering,
            Self::Engineering => Self::GitHubAudit,
            Self::GitHubAudit => Self::Instructions,
            Self::Instructions => Self::Rules,
            Self::Rules => Self::Skills,
            Self::Skills => Self::Subagents,
            Self::Subagents => Self::Hooks,
            Self::Hooks => Self::McpServers,
            Self::McpServers => Self::Manifests,
            Self::Manifests => Self::Plugins,
            Self::Plugins => Self::SyncDiff,
            Self::SyncDiff => Self::Dashboard,
        }
    }

    /// Moves to the previous section with wrap-around.
    #[must_use]
    pub const fn prev(self) -> Self {
        match self {
            Self::Dashboard => Self::SyncDiff,
            Self::Engineering => Self::Dashboard,
            Self::GitHubAudit => Self::Engineering,
            Self::Instructions => Self::GitHubAudit,
            Self::Rules => Self::Instructions,
            Self::Skills => Self::Rules,
            Self::Subagents => Self::Skills,
            Self::Hooks => Self::Subagents,
            Self::McpServers => Self::Hooks,
            Self::Manifests => Self::McpServers,
            Self::Plugins => Self::Manifests,
            Self::SyncDiff => Self::Plugins,
        }
    }

    /// Returns the localized display label for this navigation section.
    #[must_use]
    pub const fn label(self, i18n: I18n) -> &'static str {
        match self {
            Self::Dashboard => i18n.nav_dashboard(),
            Self::Engineering => i18n.nav_engineering(),
            Self::GitHubAudit => i18n.nav_github_audit(),
            Self::Instructions => i18n.nav_instructions(),
            Self::Rules => i18n.nav_rules(),
            Self::Skills => i18n.nav_skills(),
            Self::Subagents => i18n.nav_agents(),
            Self::Hooks => i18n.nav_hooks(),
            Self::McpServers => i18n.nav_mcp(),
            Self::Manifests => i18n.nav_manifests(),
            Self::Plugins => i18n.nav_plugins(),
            Self::SyncDiff => i18n.nav_sync_diff(),
        }
    }

    /// Returns the glyph icon from `theme.glyphs` for this section.
    #[must_use]
    pub const fn glyph(self, theme: &Theme) -> &'static str {
        match self {
            Self::Dashboard => theme.glyphs.dashboard,
            Self::Engineering | Self::GitHubAudit => theme.glyphs.governance,
            Self::Instructions => theme.glyphs.instructions,
            Self::Rules => theme.glyphs.rules,
            Self::Skills => theme.glyphs.skills,
            Self::Subagents => theme.glyphs.agents,
            Self::Hooks => theme.glyphs.hooks,
            Self::McpServers => theme.glyphs.mcp,
            Self::Manifests => theme.glyphs.manifests,
            Self::Plugins => theme.glyphs.plugins,
            Self::SyncDiff => theme.glyphs.diff,
        }
    }
}

impl InspectorTab {
    /// Cycles to the next tab inside the right live inspector pod.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Preview => Self::Diff,
            Self::Diff => Self::Diagnostics,
            Self::Diagnostics => Self::Budget,
            Self::Budget => Self::Preview,
        }
    }

    /// Cycles to the previous tab inside the right live inspector pod.
    #[must_use]
    pub const fn prev(self) -> Self {
        match self {
            Self::Preview => Self::Budget,
            Self::Diff => Self::Preview,
            Self::Diagnostics => Self::Diff,
            Self::Budget => Self::Diagnostics,
        }
    }
}

impl AppState {
    pub(super) fn update_navigation(&mut self, action: &Action) {
        match *action {
            Action::SelectNav(section) => self.switch_nav(section),
            Action::NextNav => self.switch_nav(self.nav.next()),
            Action::PrevNav => self.switch_nav(self.nav.prev()),
            Action::MoveDown => self.handle_vertical_move(1),
            Action::MoveUp => self.handle_vertical_move(-1),
            Action::MoveLeft => self.handle_horizontal_move(false),
            Action::MoveRight => self.handle_horizontal_move(true),
            Action::JumpTop => self.jump_edge(true),
            Action::JumpBottom => self.jump_edge(false),
            Action::CycleFocusPane => self.cycle_focus(),
            Action::FocusPane(pane) => self.focus = pane,
            Action::ToggleRail => {
                self.visibility.rail = !self.visibility.rail;
                if !self.visibility.rail && self.focus == FocusPane::Rail {
                    self.focus = FocusPane::Workspace;
                }
            }
            Action::ToggleInspector => {
                self.visibility.inspector = !self.visibility.inspector;
                if !self.visibility.inspector && self.focus == FocusPane::Inspector {
                    self.focus = FocusPane::Workspace;
                }
            }
            Action::SelectInspectorTab(tab) => {
                self.inspector_tab = tab;
                self.inspector_scroll = 0;
            }
            Action::ToggleDiffMode => {
                self.diff_style = match self.diff_style {
                    DiffStyle::Unified => DiffStyle::SideBySide,
                    DiffStyle::SideBySide => DiffStyle::Unified,
                };
            }
            Action::ToggleLogsDrawer => {
                self.visibility.logs_drawer = !self.visibility.logs_drawer;
                if self.visibility.logs_drawer {
                    self.focus = FocusPane::LogsDrawer;
                } else if self.focus == FocusPane::LogsDrawer {
                    self.focus = FocusPane::Workspace;
                }
            }
            Action::ScrollLines(delta) => self.scroll_active(delta),
            _ => {}
        }
    }

    pub(super) fn handle_mouse_click(&mut self, col: u16, row: u16) {
        if rect_contains(self.geometry.rail_area, col, row) {
            self.focus = FocusPane::Rail;
            if row >= self.geometry.rail_items_top
                && let Some(section) =
                    rail_section_at_row(row.saturating_sub(self.geometry.rail_items_top))
            {
                self.switch_nav(section);
            }
        } else if rect_contains(self.geometry.inspector_area, col, row) {
            self.focus = FocusPane::Inspector;
        } else if rect_contains(self.geometry.logs_area, col, row) {
            self.focus = FocusPane::LogsDrawer;
        } else if rect_contains(self.geometry.workspace_area, col, row) {
            self.focus = FocusPane::Workspace;
        }
    }

    pub(super) fn handle_mouse_scroll(&mut self, col: u16, row: u16, up: bool) {
        let delta = if up { -2 } else { 2 };
        if rect_contains(self.geometry.inspector_area, col, row) {
            self.focus = FocusPane::Inspector;
            self.scroll_active(delta);
        } else if rect_contains(self.geometry.logs_area, col, row) {
            self.focus = FocusPane::LogsDrawer;
            self.scroll_active(delta);
        } else if rect_contains(self.geometry.workspace_area, col, row) {
            self.focus = FocusPane::Workspace;
            self.handle_vertical_move(if up { -1 } else { 1 });
        }
    }

    pub(super) fn clamp_selection_indices(&mut self) {
        let count = self.current_resource_count();
        if count == 0 {
            self.list_index = 0;
        } else if self.list_index >= count {
            self.list_index = count.saturating_sub(1);
        }
        let sec_count = self
            .workspace
            .manifest
            .instructions
            .get(self.list_index)
            .map_or(0, |inst| inst.sections.len());
        if sec_count == 0 {
            self.section_index = 0;
        } else if self.section_index >= sec_count {
            self.section_index = sec_count.saturating_sub(1);
        }
    }

    fn switch_nav(&mut self, section: NavSection) {
        if self.nav != section {
            self.nav = section;
            self.list_index = 0;
            self.section_index = 0;
            self.governance_row = 0;
            self.inspector_scroll = 0;
            self.effects
                .trigger_view_transition(self.geometry.workspace_area, &self.theme);
        }
    }

    fn handle_vertical_move(&mut self, delta: i32) {
        if self.modal == Some(ModalKind::CommandPalette) {
            let count = self.palette.filtered(self.i18n()).len();
            self.palette.selected = shift_index(self.palette.selected, count, delta);
            return;
        }
        if self.modal == Some(ModalKind::CatalogPicker) {
            let count = self.catalog.filtered(&self.workspace.manifest).len();
            self.catalog.selected = shift_index(self.catalog.selected, count, delta);
            return;
        }
        if self.modal == Some(ModalKind::InitWizard) && self.wizard.step == 1 {
            self.wizard.option_index = shift_index(self.wizard.option_index, 2, delta);
            return;
        }
        match self.focus {
            FocusPane::Rail => {
                if delta > 0 {
                    self.switch_nav(self.nav.next());
                } else {
                    self.switch_nav(self.nav.prev());
                }
            }
            FocusPane::Inspector => {
                self.scroll_active(if delta > 0 { 2 } else { -2 });
            }
            FocusPane::LogsDrawer => {
                self.scroll_active(if delta > 0 { 1 } else { -1 });
            }
            FocusPane::Workspace => match self.nav {
                NavSection::Engineering => {
                    let count = governance_rows(
                        self.governance_domain,
                        &self.workspace.manifest.engineering,
                        self.i18n(),
                    )
                    .len();
                    self.governance_row = shift_index(self.governance_row, count, delta);
                }
                NavSection::Instructions => {
                    let count = self
                        .workspace
                        .manifest
                        .instructions
                        .get(self.list_index)
                        .map_or(0, |inst| inst.sections.len());
                    self.section_index = shift_index(self.section_index, count, delta);
                }
                NavSection::GitHubAudit => {
                    let count = self
                        .github_audit
                        .as_ref()
                        .map_or(0, |report| report.checks.len());
                    self.list_index = shift_index(self.list_index, count, delta);
                }
                NavSection::SyncDiff => {
                    let count = self.workspace.write_plan.files.len();
                    self.selected_output_idx = shift_index(self.selected_output_idx, count, delta);
                }
                _ => {
                    let count = self.current_resource_count();
                    self.list_index = shift_index(self.list_index, count, delta);
                }
            },
        }
    }

    fn handle_horizontal_move(&mut self, right: bool) {
        match self.focus {
            FocusPane::Inspector => {
                self.inspector_tab = if right {
                    self.inspector_tab.next()
                } else {
                    self.inspector_tab.prev()
                };
                self.inspector_scroll = 0;
            }
            FocusPane::Workspace if self.nav == NavSection::Engineering => {
                self.governance_domain = if right {
                    self.governance_domain.next()
                } else {
                    self.governance_domain.prev()
                };
                self.governance_row = 0;
            }
            FocusPane::Rail if right => {
                self.focus = FocusPane::Workspace;
            }
            FocusPane::Workspace if !right && self.visibility.rail => {
                self.focus = FocusPane::Rail;
            }
            _ => {}
        }
    }

    fn jump_edge(&mut self, top: bool) {
        if self.focus == FocusPane::Inspector {
            self.inspector_scroll = if top { 0 } else { u16::MAX / 2 };
            return;
        }
        if self.nav == NavSection::Engineering {
            let count = governance_rows(
                self.governance_domain,
                &self.workspace.manifest.engineering,
                self.i18n(),
            )
            .len();
            self.governance_row = if top { 0 } else { count.saturating_sub(1) };
        } else {
            let count = self.current_resource_count();
            self.list_index = if top { 0 } else { count.saturating_sub(1) };
        }
    }

    fn cycle_focus(&mut self) {
        let mut order = Vec::with_capacity(4);
        if self.visibility.rail {
            order.push(FocusPane::Rail);
        }
        order.push(FocusPane::Workspace);
        if self.visibility.inspector {
            order.push(FocusPane::Inspector);
        }
        if self.visibility.logs_drawer {
            order.push(FocusPane::LogsDrawer);
        }
        let pos = order.iter().position(|&p| p == self.focus).unwrap_or(0);
        let next_idx = pos.saturating_add(1).checked_rem(order.len()).unwrap_or(0);
        if let Some(&next_pane) = order.get(next_idx) {
            self.focus = next_pane;
        }
    }

    fn scroll_active(&mut self, delta: i16) {
        let target = if self.focus == FocusPane::LogsDrawer {
            &mut self.logs_scroll
        } else {
            &mut self.inspector_scroll
        };
        if delta >= 0 {
            *target = target.saturating_add(delta.unsigned_abs());
        } else {
            *target = target.saturating_sub(delta.unsigned_abs());
        }
    }

    const fn current_resource_count(&self) -> usize {
        let m = &self.workspace.manifest;
        match self.nav {
            NavSection::Instructions => m.instructions.len(),
            NavSection::Rules => m.rules.len(),
            NavSection::Skills => m.skills.len(),
            NavSection::Subagents => m.agents.len(),
            NavSection::Hooks => m.hooks.len(),
            NavSection::McpServers => m.mcp_servers.len(),
            NavSection::Manifests => 3,
            NavSection::Plugins => m.plugins.len(),
            NavSection::SyncDiff => self.workspace.write_plan.files.len(),
            _ => 0,
        }
    }
}

fn shift_index(current: usize, len: usize, delta: i32) -> usize {
    if len == 0 {
        return 0;
    }
    if delta >= 0 {
        current.saturating_add(1).min(len.saturating_sub(1))
    } else {
        current.saturating_sub(1)
    }
}

const fn rect_contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

/// Maps a relative line offset inside the left navigation rail (which includes 4 group header rows)
/// back to the clicked [`NavSection`].
pub(crate) const fn rail_section_at_row(offset: u16) -> Option<NavSection> {
    match offset {
        1 => Some(NavSection::Dashboard),
        3 => Some(NavSection::Engineering),
        4 => Some(NavSection::GitHubAudit),
        6 => Some(NavSection::Instructions),
        7 => Some(NavSection::Rules),
        8 => Some(NavSection::Skills),
        9 => Some(NavSection::Subagents),
        10 => Some(NavSection::Hooks),
        11 => Some(NavSection::McpServers),
        12 => Some(NavSection::Manifests),
        13 => Some(NavSection::Plugins),
        15 => Some(NavSection::SyncDiff),
        _ => None,
    }
}
