//! Full interactive master/detail workspaces for all 8 Antigravity resource kinds,
//! the live Hook Policy Playground, the Unified/Side-by-Side Sync & Diff Center,
//! `ratatui-image` plugin logo rendering, and the native inline grapheme editor overlay.

use la_core::FileAction;
use la_core::manifest::{InstructionSection, McpTransport, RuleTrigger};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::editor::EditorMode;
use crate::highlight::{PreviewSyntax, render_side_by_side_diff, render_unified_diff_styled};
use crate::media::{DecodedRgbaImage, PluginLogoPreview};
use crate::state::{AppState, DiffStyle, FocusPane, NavSection};
use crate::status::policy_decision_badge;
use crate::text::truncate_with_ellipsis;
use crate::views::pane_block;

/// Dispatches rendering for the 8 Antigravity asset views, Sync & Diff center, and inline editor.
pub(super) fn render(state: &mut AppState, frame: &mut Frame<'_>, area: Rect) {
    if state.inline_edit.is_some() {
        render_inline_editor(state, frame, area);
        return;
    }

    match state.nav {
        NavSection::Instructions => render_instructions(state, frame, area),
        NavSection::Rules => render_rules(state, frame, area),
        NavSection::Skills => render_skills(state, frame, area),
        NavSection::Subagents => render_subagents(state, frame, area),
        NavSection::Hooks => render_hooks_and_playground(state, frame, area),
        NavSection::McpServers => render_mcp_servers(state, frame, area),
        NavSection::Manifests => render_manifests(state, frame, area),
        NavSection::Plugins => render_plugins(state, frame, area),
        NavSection::SyncDiff => render_sync_diff(state, frame, area),
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

fn render_instructions(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
    let theme = &state.theme;
    let i18n = state.i18n();
    let focused = state.focus == FocusPane::Workspace;
    let [top_area, sections_area, hint_area] = Layout::vertical([
        Constraint::Length(6),
        Constraint::Min(8),
        Constraint::Length(1),
    ])
    .areas(area);

    let insts = &state.workspace.manifest.instructions;
    let mut plan_lines = Vec::new();
    for (idx, inst) in insts.iter().enumerate() {
        let sel = idx == state.list_index;
        let marker = if sel { theme.glyphs.cursor } else { " " };
        let probe_tag = if inst.auto_probe {
            "AUTO-PROBE"
        } else {
            "STATIC"
        };
        let style = if sel {
            theme.selection_style(focused)
        } else {
            Style::default().fg(theme.text_primary)
        };
        plan_lines.push(Line::from(vec![
            Span::styled(format!(" {marker} {}  ", inst.output), style),
            Span::styled(
                format!(" {probe_tag} "),
                theme.badge_style(theme.accent_primary),
            ),
            Span::styled(
                format!("  {} sections", inst.sections.len()),
                theme.muted_style(),
            ),
        ]));
    }
    if plan_lines.is_empty() {
        plan_lines.push(Line::from(Span::styled(
            i18n.pick(
                "No instruction plans declared.",
                "暂未声明 AGENTS.md 编译计划。",
            ),
            theme.muted_style(),
        )));
    }
    frame.render_widget(
        Paragraph::new(plan_lines).block(pane_block(
            format!(
                " {} {} ",
                theme.glyphs.instructions,
                i18n.nav_instructions()
            ),
            focused,
            theme,
        )),
        top_area,
    );

    let mut sec_lines = Vec::new();
    if let Some(inst) = insts.get(state.list_index) {
        for (idx, sec) in inst.sections.iter().enumerate() {
            let sel = idx == state.section_index;
            let cursor = if sel { theme.glyphs.cursor } else { " " };
            let (kind_tag, desc, cond) = match sec {
                InstructionSection::Preset { preset, when } => {
                    ("PRESET", preset.as_str(), when.as_deref())
                }
                InstructionSection::File { file, when } => ("FILE", file.as_str(), when.as_deref()),
                InstructionSection::Inline { inline, when } => (
                    "INLINE",
                    inline.lines().next().unwrap_or(""),
                    when.as_deref(),
                ),
            };
            let style = if sel {
                theme.selection_style(focused)
            } else {
                Style::default().fg(theme.text_primary)
            };
            let mut spans = vec![
                Span::styled(format!(" {cursor} {:>2}. ", idx.saturating_add(1)), style),
                Span::styled(
                    format!(" {kind_tag} "),
                    theme.badge_style(theme.accent_secondary),
                ),
                Span::styled(format!(" {desc}"), style),
            ];
            if let Some(when_expr) = cond {
                spans.push(Span::styled(
                    format!("  [when: {when_expr}]"),
                    theme.muted_style(),
                ));
            }
            sec_lines.push(Line::from(spans));
        }
    }
    frame.render_widget(
        Paragraph::new(sec_lines)
            .block(pane_block(
                format!(
                    " {} {} ",
                    theme.glyphs.manifests,
                    i18n.pick("ORDERED SECTIONS (J/K REORDER)", "有序区块（J/K 调整顺序）")
                ),
                focused,
                theme,
            ))
            .wrap(Wrap { trim: false }),
        sections_area,
    );

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            i18n.pick(
                " j/k Select Section   J/K Reorder Section   Space Toggle Auto-Probe   i Edit File   e $EDITOR",
                " j/k 选择区块   J/K 上下排序   Space 开关自动探测   i 内置编辑   e 外部编辑器",
            ),
            theme.muted_style(),
        )))
        .style(theme.surface_style()),
        hint_area,
    );
}

fn render_rules(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
    let theme = &state.theme;
    let i18n = state.i18n();
    let focused = state.focus == FocusPane::Workspace;
    let [list_area, detail_area, hint_area] = Layout::vertical([
        Constraint::Percentage(52),
        Constraint::Min(6),
        Constraint::Length(1),
    ])
    .areas(area);

    let rules = &state.workspace.manifest.rules;
    let mut list_lines = Vec::new();
    for (idx, rule) in rules.iter().enumerate() {
        let sel = idx == state.list_index;
        let marker = if sel { theme.glyphs.cursor } else { " " };
        let toggle = if rule.enabled {
            theme.glyphs.toggle_on
        } else {
            theme.glyphs.toggle_off
        };
        let (trig_label, trig_color) = match rule.trigger {
            RuleTrigger::AlwaysOn => ("ALWAYS_ON", theme.status_warn),
            RuleTrigger::Glob => ("GLOB", theme.accent_primary),
            RuleTrigger::ModelDecision => ("MODEL", theme.accent_secondary),
            RuleTrigger::Manual => ("MANUAL", theme.text_muted),
        };
        let style = if sel {
            theme.selection_style(focused)
        } else {
            Style::default().fg(theme.text_primary)
        };
        list_lines.push(Line::from(vec![
            Span::styled(format!(" {marker} {toggle} {:<20} ", rule.id), style),
            Span::styled(format!(" {trig_label} "), theme.badge_style(trig_color)),
            Span::styled(format!("  {}", rule.globs.join(", ")), theme.muted_style()),
        ]));
    }
    if list_lines.is_empty() {
        list_lines.push(Line::from(Span::styled(
            i18n.pick("No conditional rules declared yet.", "暂未配置条件规则。"),
            theme.muted_style(),
        )));
    }
    frame.render_widget(
        Paragraph::new(list_lines).block(pane_block(
            format!(" {} {} ", theme.glyphs.rules, i18n.nav_rules()),
            focused,
            theme,
        )),
        list_area,
    );

    let detail_lines = rules.get(state.list_index).map_or_else(
        || vec![Line::from(Span::styled("-", theme.muted_style()))],
        |rule| {
            let body = state
                .workspace
                .source_files
                .get(&rule.source)
                .map_or("<missing source file>", String::as_str);
            vec![
                Line::from(vec![
                    Span::styled("Source: ", theme.muted_style()),
                    Span::styled(rule.source.clone(), Style::default().fg(theme.accent_glow)),
                    Span::styled(format!("  ({} B)", body.len()), theme.muted_style()),
                ]),
                Line::from(vec![
                    Span::styled("Description: ", theme.muted_style()),
                    Span::styled(
                        rule.description.clone().unwrap_or_else(|| "-".to_owned()),
                        Style::default().fg(theme.text_primary),
                    ),
                ]),
                Line::default(),
                Line::from(Span::styled(
                    truncate_with_ellipsis(
                        body.lines().next().unwrap_or(""),
                        76,
                        theme.glyphs.ellipsis,
                    ),
                    Style::default().fg(theme.text_secondary),
                )),
            ]
        },
    );
    frame.render_widget(
        Paragraph::new(detail_lines)
            .block(pane_block(
                format!(
                    " {} {} ",
                    theme.glyphs.instructions,
                    i18n.pick("RULE DETAIL", "规则详情")
                ),
                false,
                theme,
            ))
            .wrap(Wrap { trim: false }),
        detail_area,
    );

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            i18n.pick(
                " Space Enable/Disable   c Cycle Trigger (Budget Offload)   i Inline Edit   e $EDITOR",
                " Space 启停规则   c 切换触发器（快速降预算）   i 内置编辑正文   e 外部编辑",
            ),
            theme.muted_style(),
        )))
        .style(theme.surface_style()),
        hint_area,
    );
}

fn render_skills(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
    let theme = &state.theme;
    let i18n = state.i18n();
    let focused = state.focus == FocusPane::Workspace;
    let [list_area, preview_area, hint_area] = Layout::vertical([
        Constraint::Percentage(45),
        Constraint::Min(6),
        Constraint::Length(1),
    ])
    .areas(area);

    let skills = &state.workspace.manifest.skills;
    let list_lines: Vec<Line<'static>> = if skills.is_empty() {
        vec![Line::from(Span::styled(
            i18n.pick("No skills configured yet.", "暂未配置 Skill 技能包。"),
            theme.muted_style(),
        ))]
    } else {
        skills
            .iter()
            .enumerate()
            .map(|(idx, skill)| {
                let sel = idx == state.list_index;
                let marker = if sel { theme.glyphs.cursor } else { " " };
                let toggle = if skill.enabled {
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
                    Span::styled(format!(" {marker} {toggle} {:<22} ", skill.name), style),
                    Span::styled(skill.source.clone(), theme.muted_style()),
                ])
            })
            .collect()
    };
    frame.render_widget(
        Paragraph::new(list_lines).block(pane_block(
            format!(" {} {} ", theme.glyphs.skills, i18n.nav_skills()),
            focused,
            theme,
        )),
        list_area,
    );

    let skill_preview = skills.get(state.list_index).map_or_else(
        || vec![Line::from(Span::styled("-", theme.muted_style()))],
        |skill| {
            let key = format!("{}/SKILL.md", skill.source);
            let body = state
                .workspace
                .source_files
                .get(&key)
                .map_or("<SKILL.md not found in .lazyagents/>", String::as_str);
            body.lines()
                .take(12)
                .map(|l| {
                    Line::from(Span::styled(
                        l.to_owned(),
                        Style::default().fg(theme.text_secondary),
                    ))
                })
                .collect()
        },
    );
    frame.render_widget(
        Paragraph::new(skill_preview)
            .block(pane_block(
                format!(" {} SKILL.md ", theme.glyphs.instructions),
                false,
                theme,
            ))
            .wrap(Wrap { trim: false }),
        preview_area,
    );

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            i18n.pick(
                " Space Enable/Disable   i Edit SKILL.md Inline   e Open in $EDITOR",
                " Space 启停技能   i 内置编辑 SKILL.md   e 外部编辑器打开",
            ),
            theme.muted_style(),
        )))
        .style(theme.surface_style()),
        hint_area,
    );
}

fn render_subagents(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
    let theme = &state.theme;
    let i18n = state.i18n();
    let focused = state.focus == FocusPane::Workspace;
    let [list_area, detail_area, hint_area] = Layout::vertical([
        Constraint::Percentage(50),
        Constraint::Min(6),
        Constraint::Length(1),
    ])
    .areas(area);

    let agents = &state.workspace.manifest.agents;
    let list_lines: Vec<Line<'static>> = if agents.is_empty() {
        vec![Line::from(Span::styled(
            i18n.pick("No custom subagents configured.", "暂未配置自定义子代理。"),
            theme.muted_style(),
        ))]
    } else {
        agents
            .iter()
            .enumerate()
            .map(|(idx, agent)| {
                let sel = idx == state.list_index;
                let marker = if sel { theme.glyphs.cursor } else { " " };
                let toggle = if agent.enabled {
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
                    Span::styled(format!(" {marker} {toggle} {:<18} ", agent.name), style),
                    Span::styled(
                        format!(" {:?} ", agent.model),
                        theme.badge_style(theme.accent_secondary),
                    ),
                    Span::styled(format!("  {}", agent.description), theme.muted_style()),
                ])
            })
            .collect()
    };
    frame.render_widget(
        Paragraph::new(list_lines).block(pane_block(
            format!(" {} {} ", theme.glyphs.agents, i18n.nav_agents()),
            focused,
            theme,
        )),
        list_area,
    );

    let detail_lines = agents.get(state.list_index).map_or_else(
        || vec![Line::from(Span::styled("-", theme.muted_style()))],
        |agent| {
            let caps: Vec<String> = agent
                .capabilities
                .iter()
                .map(|c| format!("{c:?}"))
                .collect();
            vec![
                Line::from(vec![
                    Span::styled("Capabilities: ", theme.muted_style()),
                    Span::styled(
                        if caps.is_empty() {
                            "-".to_owned()
                        } else {
                            caps.join(" | ")
                        },
                        Style::default().fg(theme.accent_primary),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Antigravity: ", theme.muted_style()),
                    Span::styled(
                        format!(
                            "main_agent={}  subagent={}",
                            agent.targets.antigravity.main_agent,
                            agent.targets.antigravity.subagent
                        ),
                        Style::default().fg(theme.accent_glow),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Prompt Source: ", theme.muted_style()),
                    Span::styled(
                        agent.source.clone(),
                        Style::default().fg(theme.text_secondary),
                    ),
                ]),
            ]
        },
    );
    frame.render_widget(
        Paragraph::new(detail_lines)
            .block(pane_block(
                format!(
                    " {} {} ",
                    theme.glyphs.agents,
                    i18n.pick("SUBAGENT PROFILE", "子代理配置")
                ),
                false,
                theme,
            ))
            .wrap(Wrap { trim: false }),
        detail_area,
    );

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            i18n.pick(
                " Space Enable/Disable   c Cycle Model Tier   i Edit System Prompt   e $EDITOR",
                " Space 启停子代理   c 切换模型档位   i 内置编辑提示词   e 外部编辑",
            ),
            theme.muted_style(),
        )))
        .style(theme.surface_style()),
        hint_area,
    );
}

fn render_hooks_and_playground(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
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

fn render_mcp_servers(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
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

fn render_manifests(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
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

fn render_plugins(state: &mut AppState, frame: &mut Frame<'_>, area: Rect) {
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

fn render_sync_diff(state: &mut AppState, frame: &mut Frame<'_>, area: Rect) {
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
