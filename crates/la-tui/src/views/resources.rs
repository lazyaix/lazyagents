//! Master/detail workspace renderer dispatcher for all 8 Antigravity resource kinds,
//! the Sync & Diff center, and the native inline grapheme editor overlay.

mod docs;
mod runtime;
mod sync_diff;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::editor::EditorMode;
use crate::state::{AppState, NavSection};
use crate::views::pane_block;

/// Dispatches rendering for the 8 Antigravity asset views, Sync & Diff center, and inline editor.
pub(super) fn render(state: &mut AppState, frame: &mut Frame<'_>, area: Rect) {
    if state.inline_edit.is_some() {
        render_inline_editor(state, frame, area);
        return;
    }

    match state.nav {
        NavSection::Instructions => docs::render_instructions(state, frame, area),
        NavSection::Rules => docs::render_rules(state, frame, area),
        NavSection::Skills => docs::render_skills(state, frame, area),
        NavSection::Subagents => docs::render_subagents(state, frame, area),
        NavSection::Hooks => runtime::render_hooks_and_playground(state, frame, area),
        NavSection::McpServers => runtime::render_mcp_servers(state, frame, area),
        NavSection::Manifests => runtime::render_manifests(state, frame, area),
        NavSection::Plugins => runtime::render_plugins(state, frame, area),
        NavSection::SyncDiff => sync_diff::render_sync_diff(state, frame, area),
        _ => {}
    }
}

fn render_inline_editor(state: &mut AppState, frame: &mut Frame<'_>, area: Rect) {
    let i18n = state.i18n();
    let theme = state.theme;
    let [editor_area, hint_area] =
        Layout::vertical([Constraint::Min(6), Constraint::Length(1)]).areas(area);

    if let Some(ref mut edit) = state.inline_edit {
        let mode_badge = match edit.editor.mode() {
            EditorMode::Insert => i18n.editor_mode_insert(),
            EditorMode::Normal => i18n.editor_mode_normal(),
        };
        let dirty = if edit.editor.is_dirty() { " ●" } else { "" };
        let vim_tag = if edit.editor.vim_enabled() {
            " VIM"
        } else {
            ""
        };
        let title = format!(
            " {} {} [{mode_badge}{vim_tag}]{dirty} ",
            theme.glyphs.cursor, edit.title
        );
        let block = pane_block(title, true, &theme);
        edit.editor
            .render_into(editor_area, frame.buffer_mut(), Some(block), &theme, true);
    }

    let hint = Paragraph::new(Line::from(Span::styled(
        i18n.pick(
            " Ctrl+S Save & Recompile   Ctrl+V Toggle Vim Mode   Ctrl+Z Undo   Esc Close Editor",
            " Ctrl+S 保存并重编译   Ctrl+V 切换 Vim 模式   Ctrl+Z 撤销   Esc 退出编辑器",
        ),
        theme.muted_style(),
    )))
    .style(theme.surface_style());
    frame.render_widget(hint, hint_area);
}
