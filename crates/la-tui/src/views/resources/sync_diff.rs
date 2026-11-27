//! Unified and side-by-side Sync & Diff Center renderer with Conflict Guard status badges.

use la_core::FileAction;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::highlight::{PreviewSyntax, render_side_by_side_diff, render_unified_diff_styled};
use crate::state::{AppState, DiffStyle, FocusPane};
use crate::views::pane_block;

pub(super) fn render_sync_diff(state: &mut AppState, frame: &mut Frame<'_>, area: Rect) {
    let theme = state.theme;
    let i18n = state.i18n();
    let focused = state.focus == FocusPane::Workspace;
    let [files_area, diff_area, hint_area] = Layout::vertical([
        Constraint::Length(8),
        Constraint::Min(8),
        Constraint::Length(1),
    ])
    .areas(area);

    let plan_files = &state.workspace.write_plan.files;
    let file_lines: Vec<Line<'static>> = plan_files
        .iter()
        .enumerate()
        .map(|(idx, f)| {
            let sel = idx == state.selected_output_idx;
            let marker = if sel { theme.glyphs.cursor } else { " " };
            let (tag, color) = match f.action {
                FileAction::Create => ("CREATE", theme.status_success),
                FileAction::Update => ("UPDATE", theme.accent_primary),
                FileAction::Delete => ("DELETE", theme.status_warn),
                FileAction::ExternalConflict => ("CONFLICT", theme.status_error),
                FileAction::Unchanged => ("CLEAN", theme.text_muted),
            };
            let style = if sel {
                theme.selection_style(focused)
            } else {
                Style::default().fg(theme.text_primary)
            };
            Line::from(vec![
                Span::styled(format!(" {marker} "), style),
                Span::styled(format!(" {tag:<8} "), theme.badge_style(color)),
                Span::styled(format!(" {}", f.path), style),
            ])
        })
        .collect();
    frame.render_widget(
        Paragraph::new(file_lines).block(pane_block(
            format!(" {} {} ", theme.glyphs.diff, i18n.nav_sync_diff()),
            focused,
            &theme,
        )),
        files_area,
    );

    render_diff_body(state, frame, diff_area);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            i18n.pick(
                " s Safe Sync (Conflict Guard)   v Toggle Unified/Side-by-Side   j/k Select File",
                " s 安全同步（含冲突守卫）   v 切换统一/并排对比   j/k 选择产物文件",
            ),
            theme.muted_style(),
        )))
        .style(theme.surface_style()),
        hint_area,
    );
}

fn render_diff_body(state: &mut AppState, frame: &mut Frame<'_>, diff_area: Rect) {
    let theme = state.theme;
    let mode_tag = match state.diff_style {
        DiffStyle::Unified => "UNIFIED",
        DiffStyle::SideBySide => "SIDE-BY-SIDE",
    };
    let selected_plan = state
        .workspace
        .write_plan
        .files
        .get(state.selected_output_idx)
        .cloned();

    let diff_block = pane_block(
        format!(
            " {} {} [{mode_tag}] ",
            theme.glyphs.diff,
            selected_plan.as_ref().map_or("Diff", |f| f.path.as_str())
        ),
        false,
        &theme,
    );

    if let Some(plan_file) = selected_plan {
        if plan_file.unified_diff.is_empty() {
            let syntax = PreviewSyntax::from_path(&plan_file.path);
            let inner_w = diff_area.width.saturating_sub(2).max(16);
            let content = plan_file.final_content.as_deref().unwrap_or("");
            let preview = state
                .preview_renderer
                .render_cached(content, syntax, inner_w, &theme)
                .to_vec();
            frame.render_widget(
                Paragraph::new(preview)
                    .block(diff_block)
                    .scroll((state.inspector_scroll, 0)),
                diff_area,
            );
        } else if state.diff_style == DiffStyle::SideBySide && diff_area.width >= 68 {
            let before = state
                .workspace
                .workspace_files
                .get(&plan_file.path)
                .map_or("", String::as_str);
            let after = plan_file.final_content.as_deref().unwrap_or("");
            let rows = render_side_by_side_diff(before, after, &theme);
            let inner = diff_block.inner(diff_area);
            frame.render_widget(diff_block, diff_area);
            let [left_col, right_col] =
                Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .areas(inner);
            let left_lines: Vec<Line<'static>> = rows.iter().map(|r| r.old_line.clone()).collect();
            let right_lines: Vec<Line<'static>> = rows.into_iter().map(|r| r.new_line).collect();
            frame.render_widget(
                Paragraph::new(left_lines).scroll((state.inspector_scroll, 0)),
                left_col,
            );
            frame.render_widget(
                Paragraph::new(right_lines).scroll((state.inspector_scroll, 0)),
                right_col,
            );
        } else {
            let styled = render_unified_diff_styled(&plan_file.unified_diff, &theme);
            frame.render_widget(
                Paragraph::new(styled)
                    .block(diff_block)
                    .scroll((state.inspector_scroll, 0)),
                diff_area,
            );
        }
    } else {
        frame.render_widget(Paragraph::new("-").block(diff_block), diff_area);
    }
}
