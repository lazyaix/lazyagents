//! Markdown-backed Antigravity resource views: `Instructions` (`AGENTS.md`), `Rules`,
//! `Skills`, and `Subagents`.

use la_core::manifest::{InstructionSection, RuleTrigger};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::state::{AppState, FocusPane};
use crate::text::truncate_with_ellipsis;
use crate::views::pane_block;

pub(super) fn render_instructions(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
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

pub(super) fn render_rules(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
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

pub(super) fn render_skills(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
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

pub(super) fn render_subagents(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
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
