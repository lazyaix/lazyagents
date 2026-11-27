//! Root 3-Zone Ultra Cyberpunk Command Deck renderer and adaptive layout coordinator.

mod dashboard;
mod governance;
mod modals;
mod resources;
mod splash;

use core::time::Duration;

use la_core::DiagnosticSeverity;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

use crate::config::ColorProfile;
use crate::highlight::{PreviewSyntax, render_unified_diff_styled};
use crate::state::{
    AppState, FocusPane, InspectorTab, LayoutGeometry, LogLevel, NavSection, ToastLevel,
};
use crate::status::budget_status_color;
use crate::text::truncate_with_ellipsis;

/// Minimum terminal columns required before showing the graceful tiny-terminal notice.
const MIN_STUDIO_WIDTH: u16 = 60;

/// Minimum terminal rows required before showing the graceful tiny-terminal notice.
const MIN_STUDIO_HEIGHT: u16 = 16;

/// Width threshold below which the Left Navigation Rail auto-collapses (`80x24` friendly).
const AUTO_HIDE_RAIL_WIDTH: u16 = 92;

/// Width threshold below which the Right Inspector Pod auto-collapses into compact single-pane mode.
const AUTO_HIDE_INSPECTOR_WIDTH: u16 = 108;

/// Renders a complete frame of the L4 Ultra Cyberpunk Command Deck and steps active `TachyonFX` shaders.
pub fn render_studio(state: &mut AppState, frame: &mut Frame<'_>, dt: Duration) {
    let area = frame.area();
    frame.render_widget(Block::new().style(state.theme.canvas_style()), area);

    if area.width < MIN_STUDIO_WIDTH || area.height < MIN_STUDIO_HEIGHT {
        render_tiny_terminal_guard(state, frame, area);
        return;
    }

    if state.splash_active {
        splash::render(state, frame, area);
        state
            .effects
            .process_frame(dt, frame.buffer_mut(), area, None, &state.theme);
        return;
    }

    let drawer_h = if state.visibility.logs_drawer { 8 } else { 0 };
    let [hud_area, body_area, logs_area, footer_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(10),
        Constraint::Length(drawer_h),
        Constraint::Length(1),
    ])
    .areas(area);

    render_top_hud_bar(state, frame, hud_area);

    let show_rail = state.visibility.rail && area.width >= AUTO_HIDE_RAIL_WIDTH;
    let show_inspector = state.visibility.inspector && area.width >= AUTO_HIDE_INSPECTOR_WIDTH;

    let (rail_area, workspace_area, inspector_area) =
        split_command_deck(body_area, show_rail, show_inspector);

    state.geometry = LayoutGeometry {
        rail_area,
        workspace_area,
        inspector_area,
        logs_area,
        rail_items_top: rail_area.y.saturating_add(1),
    };

    if show_rail {
        render_navigation_rail(state, frame, rail_area);
    }
    render_primary_workspace(state, frame, workspace_area);
    if show_inspector {
        render_inspector_pod(state, frame, inspector_area);
    }
    if state.visibility.logs_drawer && logs_area.height >= 4 {
        render_logs_drawer(state, frame, logs_area);
    }
    render_footer_bar(state, frame, footer_area);

    if let Some(toast) = state.toast.clone() {
        render_toast(state, frame, area, &toast);
    }

    if let Some(modal_kind) = state.modal {
        modals::render(state, frame, area, modal_kind);
    }

    let focused_rect = if state.modal.is_some() {
        None
    } else {
        match state.focus {
            FocusPane::Rail if show_rail => Some(rail_area),
            FocusPane::Inspector if show_inspector => Some(inspector_area),
            FocusPane::LogsDrawer if state.visibility.logs_drawer => Some(logs_area),
            _ => Some(workspace_area),
        }
    };

    state
        .effects
        .process_frame(dt, frame.buffer_mut(), area, focused_rect, &state.theme);
}

fn split_command_deck(area: Rect, show_rail: bool, show_inspector: bool) -> (Rect, Rect, Rect) {
    match (show_rail, show_inspector) {
        (true, true) => Layout::horizontal([
            Constraint::Length(24),
            Constraint::Percentage(52),
            Constraint::Min(30),
        ])
        .areas::<3>(area)
        .into(),
        (true, false) => {
            let [rail, center] =
                Layout::horizontal([Constraint::Length(24), Constraint::Min(30)]).areas(area);
            (rail, center, Rect::default())
        }
        (false, true) => {
            let [center, inspector] =
                Layout::horizontal([Constraint::Percentage(56), Constraint::Min(30)]).areas(area);
            (Rect::default(), center, inspector)
        }
        (false, false) => (Rect::default(), area, Rect::default()),
    }
}

fn render_tiny_terminal_guard(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
    let i18n = state.i18n();
    let lines = vec![
        Line::from(Span::styled(
            i18n.terminal_too_small_title(),
            Style::default()
                .fg(state.theme.status_warn)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!(
                "{} ({}x{})",
                i18n.terminal_too_small_hint(),
                area.width,
                area.height
            ),
            state.theme.muted_style(),
        )),
    ];
    let para = Paragraph::new(lines)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .style(state.theme.canvas_style());
    frame.render_widget(para, area);
}

fn render_top_hud_bar(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
    let theme = &state.theme;
    let i18n = state.i18n();
    let budget = &state.workspace.lint_report.budget;
    let (sync_label, sync_color) = if state.workspace.write_plan.has_conflicts() {
        (i18n.status_conflict(), theme.status_error)
    } else if state.workspace.write_plan.has_drift() {
        (i18n.status_drifted(), theme.status_warn)
    } else {
        (i18n.status_synced(), theme.status_success)
    };

    let (errs, warns) = state.workspace.diagnostic_counts();
    let health = state.workspace.health_score();
    let budget_color = budget_status_color(budget.always_on_status, theme);

    let spans = vec![
        Span::styled(
            format!(" {} LAZYAGENTS ", theme.glyphs.brand),
            theme.badge_style(theme.accent_primary),
        ),
        Span::raw(" "),
        Span::styled(
            state.workspace.manifest.project.name.clone(),
            Style::default()
                .fg(theme.text_primary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {} ", theme.glyphs.sep), theme.muted_style()),
        Span::styled(format!(" {sync_label} "), theme.badge_style(sync_color)),
        Span::raw(" "),
        Span::styled(
            format!(
                "HP:{health}%  TOK:{}/{}",
                budget.always_on_tokens, budget.max_always_on_tokens
            ),
            Style::default().fg(budget_color),
        ),
        Span::styled(format!(" {} ", theme.glyphs.sep), theme.muted_style()),
        Span::styled(
            format!("{} {errs}  {} {warns}", theme.glyphs.err, theme.glyphs.warn),
            Style::default().fg(if errs > 0 {
                theme.status_error
            } else if warns > 0 {
                theme.status_warn
            } else {
                theme.status_success
            }),
        ),
        Span::styled(format!(" {} ", theme.glyphs.sep), theme.muted_style()),
        Span::styled(
            format!(
                "{} | {} | {}",
                theme.variant.display_name(),
                state.config.shader_preset,
                state.config.lang.as_str().to_ascii_uppercase()
            ),
            Style::default().fg(theme.accent_secondary),
        ),
    ];

    let bar_style = if theme.color_profile == ColorProfile::NoColor {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(theme.text_primary)
            .bg(theme.bg_surface_elevated)
    };
    frame.render_widget(Paragraph::new(Line::from(spans)).style(bar_style), area);
}

fn render_navigation_rail(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
    let theme = &state.theme;
    let i18n = state.i18n();
    let focused = state.focus == FocusPane::Rail;
    let m = &state.workspace.manifest;

    let mut lines = Vec::with_capacity(18);
    lines.push(rail_group_header(i18n.nav_group_overview(), theme));
    lines.push(rail_item_line(state, NavSection::Dashboard, None, focused));

    lines.push(Line::default());
    lines.push(rail_group_header(i18n.nav_group_governance(), theme));
    lines.push(rail_item_line(
        state,
        NavSection::Engineering,
        None,
        focused,
    ));
    lines.push(rail_item_line(
        state,
        NavSection::GitHubAudit,
        state.github_audit.as_ref().map(|r| r.checks.len()),
        focused,
    ));

    lines.push(Line::default());
    lines.push(rail_group_header(i18n.nav_group_resources(), theme));
    lines.push(rail_item_line(
        state,
        NavSection::Instructions,
        Some(m.instructions.len()),
        focused,
    ));
    lines.push(rail_item_line(
        state,
        NavSection::Rules,
        Some(m.rules.len()),
        focused,
    ));
    lines.push(rail_item_line(
        state,
        NavSection::Skills,
        Some(m.skills.len()),
        focused,
    ));
    lines.push(rail_item_line(
        state,
        NavSection::Subagents,
        Some(m.agents.len()),
        focused,
    ));
    lines.push(rail_item_line(
        state,
        NavSection::Hooks,
        Some(m.hooks.len()),
        focused,
    ));
    lines.push(rail_item_line(
        state,
        NavSection::McpServers,
        Some(m.mcp_servers.len()),
        focused,
    ));
    lines.push(rail_item_line(
        state,
        NavSection::Manifests,
        Some(3),
        focused,
    ));
    lines.push(rail_item_line(
        state,
        NavSection::Plugins,
        Some(m.plugins.len()),
        focused,
    ));

    lines.push(Line::default());
    lines.push(rail_group_header(i18n.nav_group_output(), theme));
    let drift_count = state
        .workspace
        .write_plan
        .files
        .iter()
        .filter(|f| f.drifted)
        .count();
    lines.push(rail_item_line(
        state,
        NavSection::SyncDiff,
        Some(drift_count),
        focused,
    ));

    let block = pane_block(format!(" {} DECK ", theme.glyphs.brand), focused, theme);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn rail_group_header(title: &str, theme: &crate::theme::Theme) -> Line<'static> {
    Line::from(Span::styled(
        format!(" {title}"),
        Style::default()
            .fg(theme.accent_secondary)
            .add_modifier(Modifier::BOLD),
    ))
}

fn rail_item_line(
    state: &AppState,
    section: NavSection,
    count: Option<usize>,
    rail_focused: bool,
) -> Line<'static> {
    let theme = &state.theme;
    let active = state.nav == section;
    let prefix = if active { theme.glyphs.cursor } else { " " };
    let label = truncate_with_ellipsis(section.label(state.i18n()), 14, theme.glyphs.ellipsis);
    let badge = count.map_or_else(String::new, |n| format!(" ({n})"));
    let text = format!("{prefix} {} {label}{badge}", section.glyph(theme));

    let style = if active {
        theme.selection_style(rail_focused)
    } else {
        Style::default().fg(theme.text_secondary)
    };
    Line::from(Span::styled(text, style))
}

fn render_primary_workspace(state: &mut AppState, frame: &mut Frame<'_>, area: Rect) {
    if state.inline_edit.is_some() {
        resources::render(state, frame, area);
        return;
    }
    match state.nav {
        NavSection::Dashboard => dashboard::render(state, frame, area),
        NavSection::Engineering => governance::render_engineering(state, frame, area),
        NavSection::GitHubAudit => governance::render_github_audit(state, frame, area),
        _ => resources::render(state, frame, area),
    }
}

fn render_inspector_pod(state: &mut AppState, frame: &mut Frame<'_>, area: Rect) {
    let focused = state.focus == FocusPane::Inspector;
    let i18n = state.i18n();
    let tab_labels = [
        (InspectorTab::Preview, i18n.inspector_preview()),
        (InspectorTab::Diff, i18n.inspector_diff()),
        (InspectorTab::Diagnostics, i18n.inspector_diagnostics()),
        (InspectorTab::Budget, i18n.inspector_budget()),
    ];

    let title_spans: Vec<Span<'static>> = tab_labels
        .into_iter()
        .flat_map(|(tab, label)| {
            let style = if tab == state.inspector_tab {
                state.theme.badge_style(state.theme.accent_primary)
            } else {
                state.theme.muted_style()
            };
            [Span::styled(format!(" {label} "), style), Span::raw(" ")]
        })
        .collect();

    let block = Block::bordered()
        .title(Line::from(title_spans))
        .border_type(if focused {
            state.theme.glyphs.focused_border
        } else {
            state.theme.glyphs.muted_border
        })
        .border_style(state.theme.border_style(focused))
        .style(state.theme.surface_style());

    let inner_w = area.width.saturating_sub(2).max(16);
    let content_lines = inspector_lines(state, inner_w);
    let para = Paragraph::new(content_lines)
        .block(block)
        .scroll((state.inspector_scroll, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(para, area);
}

fn inspector_lines(state: &mut AppState, inner_w: u16) -> Vec<Line<'static>> {
    match state.inspector_tab {
        InspectorTab::Preview => {
            if let Some(file) = state.selected_output().cloned() {
                let syntax = PreviewSyntax::from_path(&file.path);
                state
                    .preview_renderer
                    .render_cached(&file.content, syntax, inner_w, &state.theme)
                    .to_vec()
            } else {
                vec![Line::from(Span::styled(
                    state
                        .i18n()
                        .pick("No compiled files yet.", "暂无编译输出文件。"),
                    state.theme.muted_style(),
                ))]
            }
        }
        InspectorTab::Diff => inspector_diff_lines(state),
        InspectorTab::Diagnostics => inspector_diagnostic_lines(state),
        InspectorTab::Budget => inspector_budget_lines(state),
    }
}

fn inspector_diff_lines(state: &AppState) -> Vec<Line<'static>> {
    let diff_text: String = state
        .workspace
        .write_plan
        .files
        .iter()
        .filter(|f| !f.unified_diff.is_empty())
        .map(|f| f.unified_diff.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if diff_text.is_empty() {
        vec![Line::from(Span::styled(
            format!(
                "{} {}",
                state.theme.glyphs.ok,
                state.i18n().pick(
                    "Zero drift — workspace matches compiled output.",
                    "零漂移 — 磁盘文件与编译产物完全一致。",
                )
            ),
            Style::default().fg(state.theme.status_success),
        ))]
    } else {
        render_unified_diff_styled(&diff_text, &state.theme)
    }
}

fn inspector_diagnostic_lines(state: &AppState) -> Vec<Line<'static>> {
    if state.workspace.lint_report.diagnostics.is_empty() {
        return vec![Line::from(Span::styled(
            format!(
                "{} {}",
                state.theme.glyphs.ok,
                state.i18n().pick(
                    "0 LA001-LA017 compliance issues.",
                    "0 项 LA001-LA017 合规问题。"
                )
            ),
            Style::default().fg(state.theme.status_success),
        ))];
    }
    state
        .workspace
        .lint_report
        .diagnostics
        .iter()
        .map(|diag| {
            let color = match diag.severity {
                DiagnosticSeverity::Error => state.theme.status_error,
                DiagnosticSeverity::Warning => state.theme.status_warn,
                DiagnosticSeverity::Info => state.theme.accent_primary,
            };
            Line::from(vec![
                Span::styled(
                    format!("[{}] ", diag.code),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{}: {}", diag.location.path, diag.message),
                    Style::default().fg(state.theme.text_primary),
                ),
            ])
        })
        .collect()
}

fn inspector_budget_lines(state: &AppState) -> Vec<Line<'static>> {
    let budget = &state.workspace.lint_report.budget;
    let mut lines = vec![Line::from(Span::styled(
        format!(
            "Always-On: {} / {} tokens",
            budget.always_on_tokens, budget.max_always_on_tokens
        ),
        Style::default()
            .fg(budget_status_color(budget.always_on_status, &state.theme))
            .add_modifier(Modifier::BOLD),
    ))];
    for fb in &budget.files {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", fb.path),
                Style::default().fg(state.theme.text_primary),
            ),
            Span::styled(
                format!(
                    "{}/{} B (~{} tok)",
                    fb.expanded_bytes, fb.max_bytes, fb.estimated_tokens
                ),
                Style::default().fg(budget_status_color(fb.status, &state.theme)),
            ),
        ]));
    }
    for hint in &budget.offload_suggestions {
        lines.push(Line::from(Span::styled(
            format!("{} {hint}", state.theme.glyphs.warn),
            Style::default().fg(state.theme.status_warn),
        )));
    }
    lines
}

fn render_logs_drawer(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
    let theme = &state.theme;
    let focused = state.focus == FocusPane::LogsDrawer;
    let title = format!(
        " {} {} ",
        theme.glyphs.hooks,
        state.i18n().logs_drawer_title()
    );

    let lines: Vec<Line<'static>> = if state.logs.is_empty() {
        vec![Line::from(Span::styled(
            state.i18n().pick(
                "Redacted tracing ring buffer active (zero stdout pollution).",
                "脱敏日志环形缓冲已就绪（零 stdout 污染）。",
            ),
            theme.muted_style(),
        ))]
    } else {
        state
            .logs
            .iter()
            .rev()
            .map(|entry| {
                let color = match entry.level {
                    LogLevel::Debug => theme.text_muted,
                    LogLevel::Info => theme.accent_primary,
                    LogLevel::Warn => theme.status_warn,
                    LogLevel::Error => theme.status_error,
                };
                Line::from(vec![
                    Span::styled(format!("{} ", entry.timestamp), theme.muted_style()),
                    Span::styled(
                        format!("[{:?}] ", entry.level),
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{}: ", entry.target),
                        Style::default().fg(theme.accent_secondary),
                    ),
                    Span::styled(
                        entry.message.clone(),
                        Style::default().fg(theme.text_primary),
                    ),
                ])
            })
            .collect()
    };

    let block = pane_block(title, focused, theme);
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .scroll((state.logs_scroll, 0)),
        area,
    );
}

fn render_footer_bar(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
    let theme = &state.theme;
    let text = truncate_with_ellipsis(
        state.i18n().footer_hints(),
        usize::from(area.width),
        theme.glyphs.ellipsis,
    );
    let style = if theme.color_profile == ColorProfile::NoColor {
        Style::default().add_modifier(Modifier::DIM)
    } else {
        Style::default()
            .fg(theme.text_secondary)
            .bg(theme.bg_surface)
    };
    frame.render_widget(Paragraph::new(text).style(style), area);
}

fn render_toast(
    state: &AppState,
    frame: &mut Frame<'_>,
    viewport: Rect,
    toast: &crate::state::ToastMessage,
) {
    let theme = &state.theme;
    let accent = match toast.level {
        ToastLevel::Info => theme.accent_primary,
        ToastLevel::Success => theme.status_success,
        ToastLevel::Warning => theme.status_warn,
        ToastLevel::Error => theme.status_error,
    };
    let width = u16::try_from(toast.message.len().saturating_add(6))
        .unwrap_or(44)
        .clamp(24, viewport.width.saturating_sub(4));
    let height = 3u16;
    let x = viewport
        .x
        .saturating_add(viewport.width.saturating_sub(width).saturating_sub(2));
    let y = viewport
        .y
        .saturating_add(viewport.height.saturating_sub(height).saturating_sub(2));
    let rect = Rect::new(x, y, width, height);

    frame.render_widget(Clear, rect);
    let block = Block::bordered()
        .border_type(theme.glyphs.focused_border)
        .border_style(Style::default().fg(accent).add_modifier(Modifier::BOLD))
        .style(theme.modal_style())
        .shadow(theme.modal_shadow());
    let para = Paragraph::new(Line::from(Span::styled(
        format!(" {} ", toast.message),
        Style::default().fg(theme.text_primary),
    )))
    .block(block);
    frame.render_widget(para, rect);
}

pub(super) fn pane_block(
    title: String,
    focused: bool,
    theme: &crate::theme::Theme,
) -> Block<'static> {
    Block::bordered()
        .title(Line::from(Span::styled(
            title,
            Style::default()
                .fg(if focused {
                    theme.accent_primary
                } else {
                    theme.text_secondary
                })
                .add_modifier(Modifier::BOLD),
        )))
        .border_type(if focused {
            theme.glyphs.focused_border
        } else {
            theme.glyphs.muted_border
        })
        .border_style(theme.border_style(focused))
        .style(theme.surface_style())
}
