//! Runtime JSON Antigravity resource views: `Hooks` + live Hook Policy Playground,
//! `McpServers`, declarative index `Manifests`, and `Plugins` with `ratatui-image` logo graphics.

use la_core::manifest::McpTransport;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::media::{DecodedRgbaImage, PluginLogoPreview};
use crate::state::{AppState, FocusPane};
use crate::status::policy_decision_badge;
use crate::views::pane_block;

pub(super) fn render_hooks_and_playground(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
    let theme = &state.theme;
    let i18n = state.i18n();
    let focused = state.focus == FocusPane::Workspace;
    let [hooks_area, lab_area, hint_area] = Layout::vertical([
        Constraint::Percentage(42),
        Constraint::Min(10),
        Constraint::Length(1),
    ])
    .areas(area);

    let hooks = &state.workspace.manifest.hooks;
    let hook_lines: Vec<Line<'static>> = if hooks.is_empty() {
        vec![Line::from(Span::styled(
            i18n.pick(
                "Built-in Rust guardrails (git-guard, secret-guard, protected-paths, verify-gate) compile automatically from [engineering].",
                "内置纯 Rust 护栏（git-guard / secret-guard / protected-paths / verify-gate）由 [engineering] 自动编译生成。",
            ),
            theme.muted_style(),
        ))]
    } else {
        hooks
            .iter()
            .enumerate()
            .map(|(idx, hook)| {
                let sel = idx == state.list_index && !state.playground.input_focused;
                let marker = if sel { theme.glyphs.cursor } else { " " };
                let toggle = if hook.enabled {
                    theme.glyphs.toggle_on
                } else {
                    theme.glyphs.toggle_off
                };
                let style = if sel {
                    theme.selection_style(focused)
                } else {
                    Style::default().fg(theme.text_primary)
                };
                Line::from(vec![
                    Span::styled(format!(" {marker} {toggle} {:<18} ", hook.name), style),
                    Span::styled(
                        format!(" {:?} ", hook.event),
                        theme.badge_style(theme.accent_primary),
                    ),
                    Span::styled(
                        format!("  {} ({}s)", hook.command, hook.timeout_secs),
                        theme.muted_style(),
                    ),
                ])
            })
            .collect()
    };
    frame.render_widget(
        Paragraph::new(hook_lines)
            .block(pane_block(
                format!(" {} {} ", theme.glyphs.hooks, i18n.nav_hooks()),
                focused && !state.playground.input_focused,
                theme,
            ))
            .wrap(Wrap { trim: false }),
        hooks_area,
    );

    render_hook_playground_panel(state, frame, lab_area, focused);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            i18n.pick(
                " / Focus Playground Input   g Cycle Guard   n Next Sample   Enter Evaluate Live",
                " / 聚焦沙盒输入框   g 切换护栏   n 切换样例   Enter 实时判决",
            ),
            theme.muted_style(),
        )))
        .style(theme.surface_style()),
        hint_area,
    );
}

fn render_hook_playground_panel(
    state: &AppState,
    frame: &mut Frame<'_>,
    area: Rect,
    focused: bool,
) {
    let theme = &state.theme;
    let i18n = state.i18n();
    let pg = &state.playground;
    let mut lab_lines = vec![
        Line::from(vec![
            Span::styled(i18n.pick(" Guard: ", " 护栏: "), theme.muted_style()),
            Span::styled(
                format!(" {} ", pg.guard.as_str()),
                theme.badge_style(theme.accent_secondary),
            ),
            Span::styled(
                i18n.pick(
                    "   [g] Switch Guard   [n] Next Sample   [Enter] Evaluate",
                    "   [g] 切换护栏   [n] 下一个示例   [Enter] 立即测试",
                ),
                theme.muted_style(),
            ),
        ]),
        Line::default(),
        Line::from(vec![
            Span::styled(
                if pg.input_focused { " > " } else { "   " },
                Style::default()
                    .fg(theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                pg.input.clone(),
                if pg.input_focused {
                    theme.selection_style(true)
                } else {
                    Style::default().fg(theme.text_primary)
                },
            ),
            if pg.input_focused {
                Span::styled("█", Style::default().fg(theme.accent_primary))
            } else {
                Span::raw("")
            },
        ]),
        Line::default(),
    ];

    if let Some(verdict) = &pg.last_verdict {
        let (label, color) = policy_decision_badge(verdict.decision, theme);
        lab_lines.push(Line::from(vec![
            Span::styled(i18n.pick(" Verdict: ", " 实时判决: "), theme.muted_style()),
            Span::styled(
                format!(
                    " {} ({}) ",
                    label,
                    verdict.wire_decision.to_ascii_uppercase()
                ),
                theme.badge_style(color),
            ),
        ]));
        if let Some(reason) = &verdict.reason {
            lab_lines.push(Line::from(Span::styled(
                format!(" Reason: {reason}"),
                Style::default().fg(color),
            )));
        }
        lab_lines.push(Line::from(Span::styled(
            format!(" JSON: {}", verdict.raw_json),
            theme.muted_style(),
        )));
    } else {
        lab_lines.push(Line::from(Span::styled(
            i18n.pick(
                " Press `Enter` to evaluate this payload against la-core's pure-Rust hook policy engine.",
                " 按 `Enter` 立即调用 la-core 纯 Rust 策略引擎测试当前输入。",
            ),
            theme.muted_style(),
        )));
    }

    frame.render_widget(
        Paragraph::new(lab_lines)
            .block(pane_block(
                format!(" {} {} ", theme.glyphs.hooks, i18n.hook_playground_title()),
                focused && state.playground.input_focused,
                theme,
            ))
            .wrap(Wrap { trim: false }),
        area,
    );
}

pub(super) fn render_mcp_servers(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
    let theme = &state.theme;
    let i18n = state.i18n();
    let focused = state.focus == FocusPane::Workspace;
    let [list_area, detail_area] =
        Layout::vertical([Constraint::Percentage(52), Constraint::Min(6)]).areas(area);

    let servers = &state.workspace.manifest.mcp_servers;
    let list_lines: Vec<Line<'static>> = if servers.is_empty() {
        vec![Line::from(Span::styled(
            i18n.pick("No MCP servers configured.", "暂未配置 MCP 服务。"),
            theme.muted_style(),
        ))]
    } else {
        servers
            .iter()
            .enumerate()
            .map(|(idx, srv)| {
                let sel = idx == state.list_index;
                let marker = if sel { theme.glyphs.cursor } else { " " };
                let toggle = if srv.enabled {
                    theme.glyphs.toggle_on
                } else {
                    theme.glyphs.toggle_off
                };
                let (badge, color, summary) = match &srv.transport {
                    McpTransport::Stdio { command, .. } => {
                        ("STDIO", theme.accent_primary, command.as_str())
                    }
                    McpTransport::StreamableHttp { url, .. } => {
                        ("HTTP", theme.status_success, url.as_str())
                    }
                    McpTransport::LegacySse { url, .. } => {
                        ("SSE [LA006]", theme.status_error, url.as_str())
                    }
                };
                let style = if sel {
                    theme.selection_style(focused)
                } else {
                    Style::default().fg(theme.text_primary)
                };
                Line::from(vec![
                    Span::styled(format!(" {marker} {toggle} {:<16} ", srv.name), style),
                    Span::styled(format!(" {badge} "), theme.badge_style(color)),
                    Span::styled(format!("  {summary}"), theme.muted_style()),
                ])
            })
            .collect()
    };
    frame.render_widget(
        Paragraph::new(list_lines).block(pane_block(
            format!(" {} {} ", theme.glyphs.mcp, i18n.nav_mcp()),
            focused,
            theme,
        )),
        list_area,
    );

    let detail_lines = servers.get(state.list_index).map_or_else(
        || vec![Line::from(Span::styled("-", theme.muted_style()))],
        |srv| {
            let mut lines = vec![Line::from(Span::styled(
                format!("Declared ${{VAR}} Variables ({}):", srv.variables.len()),
                Style::default()
                    .fg(theme.accent_secondary)
                    .add_modifier(Modifier::BOLD),
            ))];
            for var in &srv.variables {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  ${{{}}} ", var.name),
                        Style::default().fg(theme.accent_glow),
                    ),
                    Span::styled(
                        format!("— {}: {}", var.title, var.description),
                        theme.muted_style(),
                    ),
                ]));
            }
            if srv.variables.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  (zero plaintext secrets enforced by LA001/LA007)",
                    theme.muted_style(),
                )));
            }
            lines
        },
    );
    frame.render_widget(
        Paragraph::new(detail_lines)
            .block(pane_block(
                format!(
                    " {} {} ",
                    theme.glyphs.lock,
                    i18n.pick("MCP VARIABLES & TRANSPORT", "MCP 变量与传输层")
                ),
                false,
                theme,
            ))
            .wrap(Wrap { trim: false }),
        detail_area,
    );
}

pub(super) fn render_manifests(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
    let theme = &state.theme;
    let i18n = state.i18n();
    let focused = state.focus == FocusPane::Workspace;
    let m = &state.workspace.manifest.manifests;
    let entries = [
        ("skills.json", &m.skills),
        ("rules.json", &m.rules),
        ("plugins.json", &m.plugins),
    ];

    let mut lines = Vec::new();
    for (idx, (name, spec)) in entries.into_iter().enumerate() {
        let sel = idx == state.list_index;
        let marker = if sel { theme.glyphs.cursor } else { " " };
        let style = if sel {
            theme.selection_style(focused)
        } else {
            Style::default().fg(theme.text_primary)
        };
        lines.push(Line::from(vec![
            Span::styled(format!(" {marker} {name:<16} "), style),
            Span::styled(
                format!(
                    "inherits={}  entries={}",
                    spec.inherits.len(),
                    spec.entries.len()
                ),
                Style::default().fg(theme.accent_glow),
            ),
        ]));
        if sel {
            for ent in &spec.entries {
                lines.push(Line::from(Span::styled(
                    format!(
                        "     entry: {}  include_only={:?}  exclude={:?}",
                        ent.path, ent.include_only, ent.exclude
                    ),
                    theme.muted_style(),
                )));
            }
        }
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(pane_block(
                format!(" {} {} ", theme.glyphs.manifests, i18n.nav_manifests()),
                focused,
                theme,
            ))
            .wrap(Wrap { trim: false }),
        area,
    );
}

pub(super) fn render_plugins(state: &mut AppState, frame: &mut Frame<'_>, area: Rect) {
    let theme = state.theme;
    let i18n = state.i18n();
    let focused = state.focus == FocusPane::Workspace;
    let [list_area, bottom_area] =
        Layout::vertical([Constraint::Percentage(46), Constraint::Min(8)]).areas(area);

    let plugins = state.workspace.manifest.plugins.clone();
    let list_lines: Vec<Line<'static>> = if plugins.is_empty() {
        vec![Line::from(Span::styled(
            i18n.pick("No project plugins declared.", "暂未配置项目插件包。"),
            theme.muted_style(),
        ))]
    } else {
        plugins
            .iter()
            .enumerate()
            .map(|(idx, p)| {
                let sel = idx == state.list_index;
                let marker = if sel { theme.glyphs.cursor } else { " " };
                let toggle = if p.enabled {
                    theme.glyphs.toggle_on
                } else {
                    theme.glyphs.toggle_off
                };
                let style = if sel {
                    theme.selection_style(focused)
                } else {
                    Style::default().fg(theme.text_primary)
                };
                Line::from(vec![
                    Span::styled(format!(" {marker} {toggle} {:<18} ", p.name), style),
                    Span::styled(
                        p.version.as_deref().unwrap_or("0.1.0").to_owned(),
                        Style::default().fg(theme.accent_secondary),
                    ),
                    Span::styled(format!("  {}", p.description), theme.muted_style()),
                ])
            })
            .collect()
    };
    frame.render_widget(
        Paragraph::new(list_lines).block(pane_block(
            format!(" {} {} ", theme.glyphs.plugins, i18n.nav_plugins()),
            focused,
            &theme,
        )),
        list_area,
    );

    render_plugin_details_and_logo(state, frame, bottom_area, &plugins);
}

fn render_plugin_details_and_logo(
    state: &mut AppState,
    frame: &mut Frame<'_>,
    bottom_area: Rect,
    plugins: &[la_core::manifest::PluginSpec],
) {
    let theme = state.theme;
    let i18n = state.i18n();
    let [meta_area, logo_area] =
        Layout::horizontal([Constraint::Percentage(62), Constraint::Percentage(38)])
            .areas(bottom_area);

    let meta_lines = plugins.get(state.list_index).map_or_else(
        || vec![Line::from(Span::styled("-", theme.muted_style()))],
        |p| {
            vec![
                Line::from(vec![
                    Span::styled("Title: ", theme.muted_style()),
                    Span::styled(
                        p.display_name.clone().unwrap_or_else(|| p.name.to_string()),
                        Style::default()
                            .fg(theme.accent_primary)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Bundle: ", theme.muted_style()),
                    Span::styled(
                        format!(
                            "rules={} skills={} agents={} hooks={} mcp={}",
                            p.include_rules.len(),
                            p.include_skills.len(),
                            p.include_agents.len(),
                            p.include_hooks.len(),
                            p.include_mcp.len()
                        ),
                        Style::default().fg(theme.accent_glow),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Logo Asset: ", theme.muted_style()),
                    Span::styled(
                        p.logo
                            .clone()
                            .unwrap_or_else(|| "<procedural cyber emblem>".to_owned()),
                        theme.muted_style(),
                    ),
                ]),
            ]
        },
    );
    frame.render_widget(
        Paragraph::new(meta_lines)
            .block(pane_block(
                format!(
                    " {} {} ",
                    theme.glyphs.plugins,
                    i18n.pick("PLUGIN MANIFEST", "插件详情")
                ),
                false,
                &theme,
            ))
            .wrap(Wrap { trim: false }),
        meta_area,
    );

    let logo_block = pane_block(
        format!(
            " {} LOGO ({:?}) ",
            theme.glyphs.brand, state.capabilities.graphics
        ),
        false,
        &theme,
    );
    let logo_inner = logo_block.inner(logo_area);
    frame.render_widget(logo_block, logo_area);

    let selected_name = plugins
        .get(state.list_index)
        .map(|p| p.name.as_str().to_owned());
    let graphics_kind = state.capabilities.graphics;
    if let Some(name) = selected_name {
        if let Some(preview) = state.plugin_logos.get_mut(&name) {
            preview.render(frame, logo_inner);
        } else {
            let emblem = DecodedRgbaImage::cyber_emblem();
            if let Ok(preview) = PluginLogoPreview::from_rgba(&emblem, graphics_kind) {
                let entry = state.plugin_logos.entry(name).or_insert(preview);
                entry.render(frame, logo_inner);
            }
        }
    }
}
