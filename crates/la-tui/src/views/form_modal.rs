//! Shadowed modal renderers for the reusable Structured Form Modal (`FormModal`),
//! Delete Confirmation Modal (`ConfirmDelete`), and Offline Built-in Catalog Picker (`CatalogPicker`).

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::form::{
    FieldKind, FormField, KeyValueListState, ModalFormState, StringListState, TextInputState,
};
use crate::state::{AppState, CatalogCategory};
use crate::text::truncate_with_ellipsis;
use crate::theme::Theme;
use crate::views::modals::shadowed_modal_block;

pub(super) fn render_form_modal(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
    let Some(form) = &state.active_form else {
        return;
    };
    let theme = &state.theme;
    let i18n = state.i18n();
    let title = format!(" {} {} ", theme.glyphs.brand, form.title);
    let block = shadowed_modal_block(title, theme.border_focused, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let has_tabs = form.tabs.len() > 1;
    let [tabs_area, body_area, err_area, footer_area] = Layout::vertical([
        Constraint::Length(if has_tabs { 2 } else { 0 }),
        Constraint::Min(4),
        Constraint::Length(if form.submit_error.is_some() { 2 } else { 0 }),
        Constraint::Length(1),
    ])
    .areas(inner);

    if has_tabs {
        render_form_tabs(form, frame, tabs_area, theme);
    }

    let lines = render_form_fields(form, theme);
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), body_area);

    if let Some(err) = &form.submit_error {
        let err_line = Line::from(vec![
            Span::styled(
                format!(" {} ", theme.glyphs.err),
                Style::default()
                    .fg(theme.status_error)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(err.clone(), Style::default().fg(theme.status_error)),
        ]);
        frame.render_widget(
            Paragraph::new(err_line).wrap(Wrap { trim: false }),
            err_area,
        );
    }

    let hint = if has_tabs {
        i18n.pick(
            " Tab/Up/Down Field   Alt+Left/Right Tab   Enter Activate/Add   Ctrl+S Save   Esc Cancel",
            " Tab/上下 切换字段   Alt+左右 切换目标页   Enter 编辑/添加   Ctrl+S 保存   Esc 取消",
        )
    } else {
        i18n.pick(
            " Tab/Up/Down Field   Space/Enter Edit or Toggle   d Remove Item   Ctrl+S Save   Esc Cancel",
            " Tab/上下 切换字段   Space/Enter 编辑或切换   d 删除子项   Ctrl+S 保存   Esc 取消",
        )
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            truncate_with_ellipsis(hint, usize::from(footer_area.width), theme.glyphs.ellipsis),
            theme.muted_style(),
        ))),
        footer_area,
    );
}

pub(super) fn render_delete_confirm_modal(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
    let Some(confirm) = &state.delete_confirm else {
        return;
    };
    let theme = &state.theme;
    let i18n = state.i18n();
    let block = shadowed_modal_block(
        format!(
            " {} {} ",
            theme.glyphs.warn,
            i18n.pick("CONFIRM DELETE", "确认删除资源")
        ),
        theme.status_error,
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [body_area, footer_area] =
        Layout::vertical([Constraint::Min(4), Constraint::Length(1)]).areas(inner);

    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                i18n.pick(" Remove ", " 即将移除 "),
                Style::default().fg(theme.text_primary),
            ),
            Span::styled(
                format!(" {} ", confirm.resource_kind),
                theme.badge_style(theme.status_warn),
            ),
            Span::raw(" "),
            Span::styled(
                confirm.item_label.clone(),
                Style::default()
                    .fg(theme.status_error)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                i18n.pick(
                    " from .lazyagents/lazyagents.toml?",
                    "（从配置清单中删除）？",
                ),
                Style::default().fg(theme.text_primary),
            ),
        ]),
        Line::default(),
    ];

    if let Some(path) = &confirm.backing_path {
        let toggle = if confirm.purge_source_file {
            theme.glyphs.toggle_on
        } else {
            theme.glyphs.toggle_off
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {toggle} "),
                Style::default().fg(if confirm.purge_source_file {
                    theme.status_error
                } else {
                    theme.text_muted
                }),
            ),
            Span::styled(
                i18n.pick(
                    "Also purge backing source file on disk: ",
                    "同时清理磁盘源文件：",
                ),
                Style::default().fg(theme.text_secondary),
            ),
            Span::styled(
                format!(".lazyagents/{path}"),
                Style::default().fg(theme.accent_glow),
            ),
        ]));
        lines.push(Line::default());
    }

    lines.push(Line::from(Span::styled(
        i18n.pick(
            " Managed target outputs will update cleanly on your next Sync (`s`).",
            " 受管产物文件将在下次安全同步（`s`）时自动收敛。",
        ),
        theme.muted_style(),
    )));

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), body_area);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            i18n.pick(
                " [Enter / y] Confirm Delete   [Space] Toggle Purge Source   [Esc] Cancel",
                " [Enter / y] 确认删除   [Space] 切换清理源文件   [Esc] 取消",
            ),
            theme.muted_style(),
        ))),
        footer_area,
    );
}

pub(super) fn render_catalog_picker_modal(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
    let theme = &state.theme;
    let i18n = state.i18n();
    let block = shadowed_modal_block(
        format!(
            " {} {} ",
            theme.glyphs.skills,
            i18n.pick("OFFLINE CATALOG & PRESETS", "内置预设与能力目录")
        ),
        theme.accent_secondary,
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [filter_area, query_area, list_area, hint_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Min(4),
        Constraint::Length(1),
    ])
    .areas(inner);

    let categories = [
        CatalogCategory::All,
        CatalogCategory::Presets,
        CatalogCategory::Skills,
        CatalogCategory::Mcp,
        CatalogCategory::Scaffolds,
    ];
    let tab_spans: Vec<Span<'static>> = categories
        .into_iter()
        .flat_map(|cat| {
            let style = if cat == state.catalog.category {
                theme.badge_style(theme.accent_primary)
            } else {
                theme.muted_style()
            };
            [
                Span::styled(format!(" {} ", cat.tag()), style),
                Span::raw(" "),
            ]
        })
        .collect();
    frame.render_widget(Paragraph::new(Line::from(tab_spans)), filter_area);

    let query_line = if state.catalog.query.is_empty() {
        Line::from(vec![
            Span::styled(" > ", theme.keycap_style()),
            Span::styled(
                i18n.pick(
                    "Filter built-in presets, skills, GitHub MCP, scaffolds...",
                    "输入关键词筛选内置预设、技能、GitHub MCP 或脚手架...",
                ),
                theme.muted_style(),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(" > ", theme.keycap_style()),
            Span::styled(
                state.catalog.query.clone(),
                Style::default()
                    .fg(theme.text_primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("█", Style::default().fg(theme.accent_primary)),
        ])
    };
    frame.render_widget(Paragraph::new(query_line), query_area);

    render_catalog_list(state, frame, list_area);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            i18n.pick(
                " Type to Filter   Tab Category   Up/Down Select   Enter Install   Esc Close",
                " 输入筛选   Tab 切换分类   上下选择   Enter 一键安装   Esc 关闭",
            ),
            theme.muted_style(),
        ))),
        hint_area,
    );
}

fn render_catalog_list(state: &AppState, frame: &mut Frame<'_>, list_area: Rect) {
    let theme = &state.theme;
    let i18n = state.i18n();
    let matches = state.catalog.filtered(&state.workspace.manifest);
    let max_rows = usize::from(list_area.height);
    let mut lines = Vec::new();
    for (idx, entry) in matches.iter().take(max_rows).enumerate() {
        let sel = idx == state.catalog.selected;
        let marker = if sel { theme.glyphs.cursor } else { " " };
        let base_style = if sel {
            theme.selection_style(true)
        } else {
            Style::default().fg(theme.text_primary)
        };
        let match_style = Style::default()
            .fg(theme.accent_secondary)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);

        let mut spans = vec![
            Span::styled(format!(" {marker} "), base_style),
            Span::styled(
                format!("[{:<6}] ", entry.item.category.tag()),
                Style::default().fg(theme.accent_primary),
            ),
        ];
        for (ch_idx, ch) in entry.item.id.chars().enumerate() {
            let st = if entry.matched_indices.contains(&ch_idx) {
                match_style
            } else {
                base_style
            };
            spans.push(Span::styled(ch.to_string(), st));
        }
        if entry.item.installed {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                " INSTALLED ",
                theme.badge_style(theme.status_success),
            ));
        }
        let max_summary = usize::from(list_area.width).saturating_sub(42).max(12);
        spans.push(Span::styled(
            format!(
                "  {}",
                truncate_with_ellipsis(&entry.item.summary, max_summary, theme.glyphs.ellipsis)
            ),
            theme.muted_style(),
        ));
        lines.push(Line::from(spans));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            i18n.pick(
                "  No matching catalog entries.",
                "  未找到匹配的内置目录条目。",
            ),
            theme.muted_style(),
        )));
    }
    frame.render_widget(Paragraph::new(lines), list_area);
}

fn render_form_tabs(form: &ModalFormState, frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let mut spans = vec![Span::raw(" ")];
    for (idx, tab) in form.tabs.iter().enumerate() {
        let active = idx == form.active_tab;
        let label = tab.target_badge.map_or_else(
            || format!(" {} ", tab.label),
            |target| format!(" {} [{}] ", tab.label, target),
        );
        let style = if active {
            theme.badge_style(theme.accent_primary)
        } else {
            theme.muted_style()
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
    }
    if let Some(sub) = &form.subtitle {
        spans.push(Span::styled(format!("  {sub}"), theme.muted_style()));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_form_fields(form: &ModalFormState, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for idx in form.visible_field_indices() {
        let Some(field) = form.fields.get(idx) else {
            continue;
        };
        let focused = idx == form.focused_field;
        append_field_lines(field, focused, theme, &mut lines);
    }
    lines
}

fn append_field_lines(
    field: &FormField,
    focused: bool,
    theme: &Theme,
    lines: &mut Vec<Line<'static>>,
) {
    let marker = if focused { theme.glyphs.cursor } else { " " };
    let label_style = if focused {
        Style::default()
            .fg(theme.accent_primary)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text_secondary)
    };

    match &field.kind {
        FieldKind::Text(input) => {
            lines.push(render_text_field_line(
                marker,
                &field.label,
                input,
                focused,
                label_style,
                theme,
            ));
        }
        FieldKind::Toggle(value) => {
            let icon = if *value {
                theme.glyphs.toggle_on
            } else {
                theme.glyphs.toggle_off
            };
            let badge_color = if *value {
                theme.status_success
            } else {
                theme.text_muted
            };
            lines.push(Line::from(vec![
                Span::styled(format!(" {marker} {:<18} ", field.label), label_style),
                Span::styled(
                    format!(" {icon} {} ", if *value { "ON" } else { "OFF" }),
                    theme.badge_style(badge_color),
                ),
            ]));
        }
        FieldKind::Enum { selected, options } => {
            let current = options.get(*selected).map_or("-", String::as_str);
            lines.push(Line::from(vec![
                Span::styled(format!(" {marker} {:<18} ", field.label), label_style),
                Span::styled(
                    format!(" < {current} > "),
                    if focused {
                        theme.badge_style(theme.accent_secondary)
                    } else {
                        Style::default().fg(theme.text_primary)
                    },
                ),
            ]));
        }
        FieldKind::StringList(list) => {
            append_string_list_lines(
                marker,
                &field.label,
                list,
                focused,
                label_style,
                theme,
                lines,
            );
        }
        FieldKind::KeyValueList(kv) => {
            append_kv_list_lines(marker, &field.label, kv, focused, label_style, theme, lines);
        }
    }

    if let Some(err) = &field.error {
        lines.push(Line::from(Span::styled(
            format!("     {} {err}", theme.glyphs.err),
            Style::default().fg(theme.status_error),
        )));
    } else if focused && let Some(hint) = &field.hint {
        lines.push(Line::from(Span::styled(
            format!("     {hint}"),
            theme.muted_style(),
        )));
    }
}

fn render_text_field_line(
    marker: &str,
    label: &str,
    input: &TextInputState,
    focused: bool,
    label_style: Style,
    theme: &Theme,
) -> Line<'static> {
    let mut spans = vec![Span::styled(format!(" {marker} {label:<18} "), label_style)];
    if input.value.is_empty() && !focused {
        spans.push(Span::styled(
            format!("[{}]", input.placeholder),
            theme.muted_style(),
        ));
    } else if focused {
        let (before, at, after) = input.split_for_cursor();
        spans.push(Span::styled(
            before.to_owned(),
            Style::default().fg(theme.text_primary),
        ));
        spans.push(Span::styled(
            at.to_owned(),
            theme.badge_style(theme.accent_primary),
        ));
        spans.push(Span::styled(
            after.to_owned(),
            Style::default().fg(theme.text_primary),
        ));
    } else {
        spans.push(Span::styled(
            input.value.clone(),
            Style::default().fg(theme.text_primary),
        ));
    }
    Line::from(spans)
}

fn append_string_list_lines(
    marker: &str,
    label: &str,
    list: &StringListState,
    focused: bool,
    label_style: Style,
    theme: &Theme,
    lines: &mut Vec<Line<'static>>,
) {
    let mut spans = vec![Span::styled(format!(" {marker} {label:<18} "), label_style)];
    if list.items.is_empty() {
        spans.push(Span::styled("(empty — Enter to add)", theme.muted_style()));
    } else {
        for (idx, item) in list.items.iter().enumerate() {
            let selected = focused && !list.editing_draft && idx == list.selected;
            let st = if selected {
                theme.badge_style(theme.accent_primary)
            } else {
                Style::default()
                    .fg(theme.text_primary)
                    .bg(theme.bg_surface_elevated)
            };
            spans.push(Span::styled(format!(" {item} "), st));
            spans.push(Span::raw(" "));
        }
    }
    lines.push(Line::from(spans));

    if list.editing_draft {
        let (before, at, after) = list.draft.split_for_cursor();
        lines.push(Line::from(vec![
            Span::styled("     + ", theme.keycap_style()),
            Span::styled(before.to_owned(), Style::default().fg(theme.text_primary)),
            Span::styled(at.to_owned(), theme.badge_style(theme.accent_secondary)),
            Span::styled(after.to_owned(), Style::default().fg(theme.text_primary)),
        ]));
    }
}

fn append_kv_list_lines(
    marker: &str,
    label: &str,
    kv: &KeyValueListState,
    focused: bool,
    label_style: Style,
    theme: &Theme,
    lines: &mut Vec<Line<'static>>,
) {
    lines.push(Line::from(vec![
        Span::styled(format!(" {marker} {label:<18} "), label_style),
        Span::styled(
            format!("({} entries — Enter to add, d remove)", kv.entries.len()),
            theme.muted_style(),
        ),
    ]));

    for (idx, entry) in kv.entries.iter().enumerate() {
        let selected = focused && kv.editing_col.is_none() && idx == kv.selected;
        let row_style = if selected {
            theme.selection_style(true)
        } else {
            Style::default().fg(theme.text_primary)
        };
        let detail = entry.extra.as_ref().map_or_else(
            || format!("     {} = {}", entry.key, entry.value),
            |extra| format!("     {} | {} | {extra}", entry.key, entry.value),
        );
        lines.push(Line::from(Span::styled(detail, row_style)));
    }

    if let Some(col) = kv.editing_col {
        let (col_name, draft) = match col {
            0 => (kv.key_label.as_str(), &kv.draft_key),
            1 => (kv.value_label.as_str(), &kv.draft_value),
            _ => (
                kv.extra_label.as_deref().unwrap_or("Extra"),
                &kv.draft_extra,
            ),
        };
        let (before, at, after) = draft.split_for_cursor();
        lines.push(Line::from(vec![
            Span::styled(format!("     + [{col_name}]: "), theme.keycap_style()),
            Span::styled(before.to_owned(), Style::default().fg(theme.text_primary)),
            Span::styled(at.to_owned(), theme.badge_style(theme.accent_secondary)),
            Span::styled(after.to_owned(), Style::default().fg(theme.text_primary)),
        ]));
    }
}
