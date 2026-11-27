//! HUD Overview Dashboard featuring the Cyber Governance Health Gauge, Ratatui 0.30.2
//! `GraphType::Area` Context Budget Radar, Probed Repository Facts, and Active Scope Provenance.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Axis, Chart, Dataset, Gauge, GraphType, Paragraph, Wrap};

use crate::config::{ColorProfile, GlyphSet};
use crate::state::{AppState, FocusPane};
use crate::status::budget_status_color;
use crate::views::pane_block;

/// Renders the primary HUD Dashboard workspace inside `area`.
pub(super) fn render(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
    let focused = state.focus == FocusPane::Workspace;
    let [top_row, radar_area, bottom_row] = Layout::vertical([
        Constraint::Length(6),
        Constraint::Min(10),
        Constraint::Length(7),
    ])
    .areas(area);

    render_health_and_kpis(state, frame, top_row, focused);
    render_budget_area_radar(state, frame, radar_area, focused);
    render_scopes_and_facts(state, frame, bottom_row, focused);
}

fn render_health_and_kpis(state: &AppState, frame: &mut Frame<'_>, area: Rect, focused: bool) {
    let theme = &state.theme;
    let i18n = state.i18n();
    let [health_area, kpi_area] =
        Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)]).areas(area);

    let score = state.workspace.health_score();
    let score_color = if score >= 85 {
        theme.status_success
    } else if score >= 60 {
        theme.status_warn
    } else {
        theme.status_error
    };
    let gauge_style = if theme.color_profile == ColorProfile::NoColor {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
            .fg(score_color)
            .bg(theme.bg_surface_elevated)
    };
    let health_block = pane_block(
        format!(" {} {} ", theme.glyphs.dashboard, i18n.health_score_title()),
        focused,
        theme,
    );
    let gauge = Gauge::default()
        .block(health_block)
        .gauge_style(gauge_style)
        .percent(u16::from(score))
        .label(format!("{score} / 100"));
    frame.render_widget(gauge, health_area);

    let m = &state.workspace.manifest;
    let (errs, warns) = state.workspace.diagnostic_counts();
    let kpi_lines = vec![
        Line::from(vec![
            Span::styled(
                format!("{} {}  ", theme.glyphs.rules, m.rules.len()),
                Style::default().fg(theme.accent_primary),
            ),
            Span::styled(
                format!("{} {}  ", theme.glyphs.skills, m.skills.len()),
                Style::default().fg(theme.accent_secondary),
            ),
            Span::styled(
                format!("{} {}  ", theme.glyphs.agents, m.agents.len()),
                Style::default().fg(theme.accent_glow),
            ),
            Span::styled(
                format!("{} {}  ", theme.glyphs.hooks, m.hooks.len()),
                Style::default().fg(theme.status_running),
            ),
            Span::styled(
                format!("{} {}", theme.glyphs.mcp, m.mcp_servers.len()),
                Style::default().fg(theme.status_thinking),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                i18n.pick("Compiled Outputs: ", "编译输出文件: "),
                theme.muted_style(),
            ),
            Span::styled(
                state.workspace.compiled.files.len().to_string(),
                Style::default()
                    .fg(theme.text_primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  {}  ", theme.glyphs.sep), theme.muted_style()),
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
        ]),
        Line::from(vec![
            Span::styled(i18n.pick("Targets: ", "目标平台: "), theme.muted_style()),
            Span::styled(
                if m.targets.antigravity.enabled {
                    "Antigravity [ACTIVE]"
                } else {
                    "Antigravity [OFF]"
                },
                Style::default()
                    .fg(theme.status_success)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    let kpi_block = pane_block(
        format!(
            " {} {} ",
            theme.glyphs.plugins,
            i18n.pick("STUDIO TELEMETRY", "工作室资产遥测")
        ),
        false,
        theme,
    );
    frame.render_widget(Paragraph::new(kpi_lines).block(kpi_block), kpi_area);
}

fn render_budget_area_radar(state: &AppState, frame: &mut Frame<'_>, area: Rect, focused: bool) {
    let theme = &state.theme;
    let i18n = state.i18n();
    let budget = &state.workspace.lint_report.budget;
    let status_color = budget_status_color(budget.always_on_status, theme);

    let [chart_area, summary_area] =
        Layout::vertical([Constraint::Min(7), Constraint::Length(3)]).areas(area);

    let points: Vec<(f64, f64)> = if budget.files.is_empty() {
        vec![(0.0, 0.0), (1.0, 4.0), (2.0, 2.0)]
    } else if budget.files.len() == 1 {
        let pct = file_budget_percent(
            budget.files.first().map_or(0, |f| f.expanded_bytes),
            budget.files.first().map_or(24_000, |f| f.max_bytes),
        );
        vec![(0.0, pct), (1.0, pct)]
    } else {
        budget
            .files
            .iter()
            .enumerate()
            .map(|(idx, fb)| {
                let x = f64::from(u32::try_from(idx).unwrap_or(0));
                (x, file_budget_percent(fb.expanded_bytes, fb.max_bytes))
            })
            .collect()
    };

    let max_x = points.last().map_or(2.0, |&(x, _)| x.max(1.0));
    let marker = if theme.glyph_set == GlyphSet::Ascii {
        Marker::Dot
    } else {
        Marker::Braille
    };

    let dataset = Dataset::default()
        .name(i18n.pick("Per-File % of 24KB Cap", "单文件 24KB 占用%"))
        .marker(marker)
        .graph_type(GraphType::Area)
        .fill_to_y(0.0)
        .style(Style::default().fg(status_color))
        .data(&points);

    let block = pane_block(
        format!(
            " {} {} ({}/{} tok) ",
            theme.glyphs.hooks,
            i18n.budget_radar_title(),
            budget.always_on_tokens,
            budget.max_always_on_tokens
        ),
        focused,
        theme,
    );

    let chart = Chart::new(vec![dataset])
        .block(block)
        .style(theme.surface_style())
        .x_axis(
            Axis::default()
                .style(theme.muted_style())
                .bounds([0.0, max_x])
                .labels(vec![
                    Span::styled("0", theme.muted_style()),
                    Span::styled(format!("{} files", budget.files.len()), theme.muted_style()),
                ]),
        )
        .y_axis(
            Axis::default()
                .style(theme.muted_style())
                .bounds([0.0, 100.0])
                .labels(vec![
                    Span::styled("0%", theme.muted_style()),
                    Span::styled("80%", Style::default().fg(theme.status_warn)),
                    Span::styled("100%", Style::default().fg(theme.status_error)),
                ]),
        );
    frame.render_widget(chart, chart_area);

    let summary_text = budget.offload_suggestions.first().map_or_else(
        || {
            Line::from(Span::styled(
                format!(
                    " {} {}",
                    theme.glyphs.ok,
                    i18n.pick(
                        "All instruction & rule files are safely within Antigravity's 24 KB / 20k-token context budgets.",
                        "所有指令与规则文件均安全处于 Antigravity 24 KB / 20k-token 上下文预算内。",
                    )
                ),
                Style::default().fg(theme.status_success),
            ))
        },
        |hint| {
            Line::from(Span::styled(
                format!(" {} {hint}", theme.glyphs.warn),
                Style::default().fg(theme.status_warn),
            ))
        },
    );
    frame.render_widget(
        Paragraph::new(summary_text)
            .style(theme.surface_style())
            .wrap(Wrap { trim: true }),
        summary_area,
    );
}

fn render_scopes_and_facts(state: &AppState, frame: &mut Frame<'_>, area: Rect, _focused: bool) {
    let theme = &state.theme;
    let i18n = state.i18n();
    let [scopes_area, facts_area] =
        Layout::horizontal([Constraint::Percentage(48), Constraint::Percentage(52)]).areas(area);

    let scope_names: Vec<&str> = state
        .workspace
        .active_scopes
        .iter()
        .map(|s| s.as_str())
        .collect();
    let scope_lines = vec![
        Line::from(vec![
            Span::styled(i18n.pick("Layers: ", "生效层级: "), theme.muted_style()),
            Span::styled(
                scope_names.join(" -> "),
                Style::default()
                    .fg(theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                i18n.pick("Git Mode: ", "Git 托管模式: "),
                theme.muted_style(),
            ),
            Span::styled(
                format!("{:?}", state.workspace.manifest.project.git_mode),
                Style::default().fg(theme.text_primary),
            ),
        ]),
        Line::from(vec![
            Span::styled(i18n.pick("Root: ", "工作区路径: "), theme.muted_style()),
            Span::styled(
                state.workspace.root_display.clone(),
                Style::default().fg(theme.text_secondary),
            ),
        ]),
    ];
    let scopes_block = pane_block(
        format!(" {} {} ", theme.glyphs.lock, i18n.scope_provenance_title()),
        false,
        theme,
    );
    frame.render_widget(Paragraph::new(scope_lines).block(scopes_block), scopes_area);

    let facts = &state.workspace.facts;
    let langs: Vec<String> = facts.languages.iter().map(|l| format!("{l:?}")).collect();
    let facts_lines = vec![
        Line::from(vec![
            Span::styled(i18n.pick("Languages: ", "探测语言: "), theme.muted_style()),
            Span::styled(
                if langs.is_empty() {
                    "-".to_owned()
                } else {
                    langs.join(", ")
                },
                Style::default().fg(theme.accent_glow),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                i18n.pick("Toolchain: ", "工具链版本: "),
                theme.muted_style(),
            ),
            Span::styled(
                facts
                    .toolchain_version
                    .clone()
                    .unwrap_or_else(|| "-".to_owned()),
                Style::default().fg(theme.text_primary),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                i18n.pick("GitHub Repo: ", "GitHub 仓库: "),
                theme.muted_style(),
            ),
            Span::styled(
                facts
                    .git_remote_repo
                    .clone()
                    .unwrap_or_else(|| "-".to_owned()),
                Style::default().fg(theme.accent_secondary),
            ),
        ]),
    ];
    let facts_block = pane_block(
        format!(
            " {} {} ",
            theme.glyphs.instructions,
            i18n.pick("PROBED REPO FACTS", "仓库探测事实")
        ),
        false,
        theme,
    );
    frame.render_widget(Paragraph::new(facts_lines).block(facts_block), facts_area);
}

fn file_budget_percent(expanded: usize, max_bytes: usize) -> f64 {
    if max_bytes == 0 {
        return 100.0;
    }
    let ratio_milli = expanded
        .saturating_mul(1000)
        .checked_div(max_bytes)
        .unwrap_or(1000)
        .min(1000);
    f64::from(u32::try_from(ratio_milli).unwrap_or(1000)) / 10.0
}
