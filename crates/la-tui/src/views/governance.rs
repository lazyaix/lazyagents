//! 7-domain `[engineering]` governance workbench and asynchronous read-only GitHub Audit matrix.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::state::{AppState, AuditStatusView, FocusPane, GovernanceDomain, governance_rows};
use crate::views::pane_block;

/// Renders the interactive 7-domain `[engineering]` governance view.
pub(super) fn render_engineering(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
    let theme = &state.theme;
    let i18n = state.i18n();
    let focused = state.focus == FocusPane::Workspace;

    let [tabs_area, list_area, hint_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(2),
    ])
    .areas(area);

    let tab_spans: Vec<Span<'static>> = GovernanceDomain::ALL
        .into_iter()
        .flat_map(|domain| {
            let label = domain.label(i18n);
            let style = if domain == state.governance_domain {
                theme.badge_style(theme.accent_primary)
            } else {
                theme.muted_style()
            };
            [Span::styled(format!(" {label} "), style), Span::raw(" ")]
        })
        .collect();
    let tabs_block = pane_block(
        format!(
            " {} {} ",
            theme.glyphs.governance,
            i18n.pick(
                "ENGINEERING GOVERNANCE [7 DOMAINS]",
                "工程治理策略 [7 大领域]"
            )
        ),
        focused,
        theme,
    );
    frame.render_widget(
        Paragraph::new(Line::from(tab_spans)).block(tabs_block),
        tabs_area,
    );

    let rows = governance_rows(
        state.governance_domain,
        &state.workspace.manifest.engineering,
        i18n,
    );
    let lines: Vec<Line<'static>> = rows
        .iter()
        .enumerate()
        .map(|(idx, row)| {
            let selected = idx == state.governance_row;
            let cursor = if selected { theme.glyphs.cursor } else { " " };
            let badge = if row.interactive {
                theme.glyphs.toggle_on
            } else {
                theme.glyphs.lock
            };
            let style = if selected {
                theme.selection_style(focused)
            } else {
                Style::default().fg(theme.text_primary)
            };
            Line::from(vec![
                Span::styled(format!(" {cursor} {badge} {:<32} ", row.label), style),
                Span::styled(
                    row.value.clone(),
                    Style::default()
                        .fg(if row.value == "ON" {
                            theme.status_success
                        } else if row.value == "OFF" {
                            theme.text_muted
                        } else {
                            theme.accent_glow
                        })
                        .add_modifier(Modifier::BOLD),
                ),
            ])
        })
        .collect();

    let body_block = pane_block(
        format!(" {} ", state.governance_domain.label(i18n)),
        focused,
        theme,
    );
    frame.render_widget(
        Paragraph::new(lines)
            .block(body_block)
            .wrap(Wrap { trim: false }),
        list_area,
    );

    let hint = Paragraph::new(Line::from(Span::styled(
        i18n.pick(
            "  h/l: Domain   j/k: Move   Space: Quick Toggle   E: Edit Form   a: Add Entry   d: Delete   e: $EDITOR",
            "  h/l: 切换领域   j/k: 移动   Space: 快速切换   E: 表单编辑   a: 新增条目   d: 删除   e: 外部编辑",
        ),
        theme.muted_style(),
    )))
    .style(theme.surface_style());
    frame.render_widget(hint, hint_area);
}

/// Renders the asynchronous read-only GitHub Rulesets & governance audit matrix.
pub(super) fn render_github_audit(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
    let theme = &state.theme;
    let i18n = state.i18n();
    let focused = state.focus == FocusPane::Workspace;

    let [matrix_area, fix_area] =
        Layout::vertical([Constraint::Min(10), Constraint::Length(5)]).areas(area);

    let title = format!(
        " {} {} ",
        theme.glyphs.governance,
        i18n.pick(
            "LIVE READ-ONLY GITHUB AUDIT (gh CLI)",
            "实时只读 GitHub 治理审计 (gh CLI)"
        )
    );

    let mut lines = Vec::new();
    if state.github_audit_loading {
        lines.push(Line::from(Span::styled(
            i18n.pick(
                "Querying live repository settings & Rulesets via local `gh` CLI (read-only)...",
                "正在通过本地 `gh` CLI 只读查询仓库设置与 Rulesets...",
            ),
            Style::default()
                .fg(theme.status_running)
                .add_modifier(Modifier::BOLD),
        )));
    } else if let Some(err) = &state.github_audit_error {
        lines.push(Line::from(Span::styled(
            format!("{} {err}", theme.glyphs.err),
            Style::default().fg(theme.status_error),
        )));
    } else if let Some(report) = &state.github_audit {
        lines.push(Line::from(vec![
            Span::styled(i18n.pick("Repository: ", "审计仓库: "), theme.muted_style()),
            Span::styled(
                report.repo.clone(),
                Style::default()
                    .fg(theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::default());

        for (idx, check) in report.checks.iter().enumerate() {
            let selected = idx == state.list_index;
            let cursor = if selected { theme.glyphs.cursor } else { " " };
            let (tag, color) = match check.status {
                AuditStatusView::Pass => ("PASS", theme.status_success),
                AuditStatusView::Mismatch => ("DRIFT", theme.status_error),
                AuditStatusView::Skipped => ("SKIP", theme.status_warn),
            };
            let row_style = if selected {
                theme.selection_style(focused)
            } else {
                Style::default().fg(theme.text_primary)
            };
            lines.push(Line::from(vec![
                Span::styled(format!(" {cursor} "), row_style),
                Span::styled(format!(" {tag} "), theme.badge_style(color)),
                Span::styled(format!(" {} — ", check.label), row_style),
                Span::styled(check.detail.clone(), theme.muted_style()),
            ]));
        }
    } else {
        lines.push(Line::from(Span::styled(
            i18n.pick(
                "No live GitHub audit run yet. Press `r` to query live Rulesets via local `gh` CLI (zero credentials stored).",
                "尚未执行在线审计。按 `r` 调用本地已登录的 `gh` CLI 只读校验 Rulesets（零令牌存储）。",
            ),
            theme.muted_style(),
        )));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(pane_block(title, focused, theme))
            .wrap(Wrap { trim: false }),
        matrix_area,
    );

    let remediation_line = state
        .github_audit
        .as_ref()
        .and_then(|r| r.checks.get(state.list_index))
        .and_then(|c| c.remediation_cmd.as_deref())
        .map_or_else(
            || {
                Line::from(Span::styled(
                    i18n.pick(
                        "Press `r` to refresh live audit. Select a DRIFT check and press `y` to copy its `gh api` remediation via OSC 52.",
                        "按 `r` 刷新审计；选中 DRIFT 项后按 `y` 通过 OSC 52 一键复制 `gh api` 修复命令（只读不自动修改远端）。",
                    ),
                    theme.muted_style(),
                ))
            },
            |cmd| {
                Line::from(vec![
                    Span::styled(
                        i18n.pick(" [y] Copy fix: ", " [y] 复制修复: "),
                        theme.keycap_style(),
                    ),
                    Span::styled(cmd.to_owned(), Style::default().fg(theme.accent_glow)),
                ])
            },
        );

    let fix_block = pane_block(
        format!(
            " {} {} ",
            theme.glyphs.hooks,
            i18n.pick(
                "REMEDIATION COMMAND (READ-ONLY)",
                "建议修复命令（只读复制）"
            )
        ),
        false,
        theme,
    );
    frame.render_widget(
        Paragraph::new(remediation_line)
            .block(fix_block)
            .wrap(Wrap { trim: true }),
        fix_area,
    );
}
