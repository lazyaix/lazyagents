//! Shadowed floating modals (`Block::shadow` + `TachyonFX` background scrim):
//! `Ctrl+K` Fuzzy Command Palette, Guided 3-Step Init & Import Wizard, External Conflict Guard,
//! and Bilingual Keybindings Cheat Sheet.

use la_core::FileAction;
use la_core::manifest::GitOutputMode;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

use crate::state::{AppState, ModalKind};
use crate::views::form_modal;

/// Renders the active floating modal centered over a `TachyonFX`-dimmed background scrim.
pub(super) fn render(state: &AppState, frame: &mut Frame<'_>, viewport: Rect, kind: ModalKind) {
    state
        .effects
        .apply_modal_scrim(frame.buffer_mut(), viewport, &state.theme);

    let (pct_w, pct_h) = match kind {
        ModalKind::CommandPalette => (64, 62),
        ModalKind::InitWizard => (68, 60),
        ModalKind::ConflictGuard => (66, 54),
        ModalKind::Help => (72, 68),
        ModalKind::FormModal => (76, 74),
        ModalKind::ConfirmDelete => (64, 38),
        ModalKind::CatalogPicker => (76, 70),
    };
    let modal_area = centered_rect(viewport, pct_w, pct_h);
    frame.render_widget(Clear, modal_area);

    match kind {
        ModalKind::CommandPalette => render_command_palette(state, frame, modal_area),
        ModalKind::InitWizard => render_init_wizard(state, frame, modal_area),
        ModalKind::ConflictGuard => render_conflict_guard(state, frame, modal_area),
        ModalKind::Help => render_help_modal(state, frame, modal_area),
        ModalKind::FormModal => form_modal::render_form_modal(state, frame, modal_area),
        ModalKind::ConfirmDelete => {
            form_modal::render_delete_confirm_modal(state, frame, modal_area);
        }
        ModalKind::CatalogPicker => {
            form_modal::render_catalog_picker_modal(state, frame, modal_area);
        }
    }
}

fn render_command_palette(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
    let theme = &state.theme;
    let i18n = state.i18n();
    let block = shadowed_modal_block(
        format!(" {} {} ", theme.glyphs.brand, i18n.command_palette_title()),
        theme.border_focused,
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [query_area, list_area, hint_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(4),
        Constraint::Length(1),
    ])
    .areas(inner);

    let query_line = if state.palette.query.is_empty() {
        Line::from(vec![
            Span::styled(" > ", theme.keycap_style()),
            Span::styled(i18n.command_palette_placeholder(), theme.muted_style()),
        ])
    } else {
        Line::from(vec![
            Span::styled(" > ", theme.keycap_style()),
            Span::styled(
                state.palette.query.clone(),
                Style::default()
                    .fg(theme.text_primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("█", Style::default().fg(theme.accent_primary)),
        ])
    };
    frame.render_widget(Paragraph::new(query_line), query_area);

    let matches = state.palette.filtered(i18n);
    let max_rows = usize::from(list_area.height);
    let mut lines = Vec::new();
    for (idx, entry) in matches.iter().take(max_rows).enumerate() {
        let sel = idx == state.palette.selected;
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
                format!("[{:<7}] ", entry.command.category),
                Style::default().fg(theme.accent_primary),
            ),
        ];
        for (ch_idx, ch) in entry.command.label.chars().enumerate() {
            let st = if entry.matched_indices.contains(&ch_idx) {
                match_style
            } else {
                base_style
            };
            spans.push(Span::styled(ch.to_string(), st));
        }
        if !entry.command.shortcut.is_empty() {
            spans.push(Span::styled(
                format!("  <{}>", entry.command.shortcut),
                theme.muted_style(),
            ));
        }
        lines.push(Line::from(spans));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            i18n.pick("  No matching commands.", "  未找到匹配命令。"),
            theme.muted_style(),
        )));
    }
    frame.render_widget(Paragraph::new(lines), list_area);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            i18n.pick(
                " Type to fuzzy filter   Up/Down Select   Enter Run   Esc Close",
                " 输入关键词模糊搜索   上下选择   Enter 执行   Esc 关闭",
            ),
            theme.muted_style(),
        ))),
        hint_area,
    );
}

fn render_init_wizard(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
    let theme = &state.theme;
    let i18n = state.i18n();
    let wiz = &state.wizard;
    let block = shadowed_modal_block(
        format!(
            " {} {} [{}/3] ",
            theme.glyphs.brand,
            i18n.init_wizard_title(),
            wiz.step.saturating_add(1)
        ),
        theme.accent_primary,
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [body_area, footer_area] =
        Layout::vertical([Constraint::Min(6), Constraint::Length(2)]).areas(inner);

    let facts = &state.workspace.facts;
    let lines = match wiz.step {
        0 => vec![
            Line::from(Span::styled(
                i18n.pick("Step 1/3 — Probed Repository Facts", "第 1/3 步 — 仓库技术栈自动探测"),
                Style::default()
                    .fg(theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::default(),
            Line::from(format!("  Project: {}", facts.name)),
            Line::from(format!("  Languages: {:?}", facts.languages)),
            Line::from(format!(
                "  Toolchain: {}",
                facts.toolchain_version.as_deref().unwrap_or("-")
            )),
            Line::from(format!(
                "  GitHub Remote: {}",
                facts.git_remote_repo.as_deref().unwrap_or("-")
            )),
            Line::from(format!("  CI Workflows: {}", facts.ci_workflows.len())),
        ],
        1 => {
            let mode_label = match wiz.git_mode {
                GitOutputMode::Commit => "commit (Git-tracked outputs + CI drift gate)",
                GitOutputMode::Ignore => "ignore (local build artifacts in .gitignore)",
            };
            vec![
                Line::from(Span::styled(
                    i18n.pick("Step 2/3 — Ownership & Import Strategy", "第 2/3 步 — 托管模式与导入策略"),
                    Style::default()
                        .fg(theme.accent_primary)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::default(),
                Line::from(Span::styled(
                    format!(
                        " {} Git Output Mode: {mode_label}",
                        if wiz.option_index == 0 { theme.glyphs.cursor } else { " " }
                    ),
                    if wiz.option_index == 0 {
                        theme.selection_style(true)
                    } else {
                        Style::default().fg(theme.text_primary)
                    },
                )),
                Line::from(Span::styled(
                    format!(
                        " {} Import Existing AGENTS.md / .agents/**: {}",
                        if wiz.option_index == 1 { theme.glyphs.cursor } else { " " },
                        if wiz.import_existing { "YES" } else { "NO" }
                    ),
                    if wiz.option_index == 1 {
                        theme.selection_style(true)
                    } else {
                        Style::default().fg(theme.text_primary)
                    },
                )),
            ]
        }
        _ => vec![
            Line::from(Span::styled(
                i18n.pick("Step 3/3 — Ready to Bootstrap .lazyagents/", "第 3/3 步 — 确认初始化 .lazyagents/"),
                Style::default()
                    .fg(theme.status_success)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::default(),
            Line::from(format!("  git_mode = {:?}", wiz.git_mode)),
            Line::from(format!("  import_existing = {}", wiz.import_existing)),
            Line::default(),
            Line::from(Span::styled(
                i18n.pick(
                    "Press Enter to write `.lazyagents/lazyagents.toml` atomically and launch the Command Deck.",
                    "按 Enter 原子写入 `.lazyagents/lazyagents.toml` 并进入赛博指挥舱。",
                ),
                Style::default().fg(theme.accent_glow),
            )),
        ],
    };

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), body_area);

    let footer = Paragraph::new(Line::from(Span::styled(
        i18n.pick(
            " [Enter/Right] Next/Confirm   [Left/Back] Previous   [Space] Toggle Option   [Esc] Dismiss",
            " [Enter/右] 下一步/确认   [左] 上一步   [Space] 切换选项   [Esc] 暂不初始化",
        ),
        theme.muted_style(),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(footer, footer_area);
}

fn render_conflict_guard(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
    let theme = &state.theme;
    let i18n = state.i18n();
    let block = shadowed_modal_block(
        format!(" {} {} ", theme.glyphs.warn, i18n.conflict_guard_title()),
        theme.status_error,
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = vec![
        Line::from(Span::styled(
            i18n.pick(
                "Externally modified managed files detected — silent overwrite blocked:",
                "检测到受管文件被外部手工修改，已阻断静默覆盖：",
            ),
            Style::default()
                .fg(theme.status_error)
                .add_modifier(Modifier::BOLD),
        )),
        Line::default(),
    ];
    for file in state
        .workspace
        .write_plan
        .files
        .iter()
        .filter(|f| f.action == FileAction::ExternalConflict)
    {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {} ", theme.glyphs.err),
                Style::default().fg(theme.status_error),
            ),
            Span::styled(file.path.clone(), Style::default().fg(theme.text_primary)),
        ]));
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        i18n.pick(
            "Press [F] to force overwrite from .lazyagents/ SSOT, or [Esc] to cancel and inspect Diff.",
            "按 [F] 从 .lazyagents/ 强制覆盖写入，或按 [Esc] 取消并返回查看差异。",
        ),
        Style::default().fg(theme.status_warn),
    )));

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn render_help_modal(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
    let theme = &state.theme;
    let i18n = state.i18n();
    let block = shadowed_modal_block(
        format!(" {} {} ", theme.glyphs.brand, i18n.help_modal_title()),
        theme.accent_secondary,
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = [
        (
            "Ctrl+K",
            i18n.pick("Open Fuzzy Command Palette", "打开模糊搜索命令面板"),
        ),
        (
            "Tab / [ / ]",
            i18n.pick(
                "Cycle Focus Pane / Toggle Rail / Toggle Inspector",
                "切换焦点窗格 / 折叠导航轨 / 检视舱",
            ),
        ),
        (
            "1..9, 0",
            i18n.pick("Quick-Jump Command Deck Views", "数字键直达各工作室视图"),
        ),
        (
            "j / k / h / l",
            i18n.pick(
                "Navigate Lists, Tabs & Governance Domains",
                "Vim 风格列表、标签与治理领域导航",
            ),
        ),
        (
            "Space / c",
            i18n.pick(
                "Toggle Policy or Enabled / Cycle Trigger or Model",
                "切换启用状态 / 循环规则触发器或模型",
            ),
        ),
        (
            "J / K",
            i18n.pick(
                "Reorder AGENTS.md Instruction Sections",
                "调整 AGENTS.md 区块编译顺序",
            ),
        ),
        (
            "a / A / E / d / C",
            i18n.pick(
                "Add Resource / New Plan / Edit Form / Delete / Catalog",
                "新建资源 / 新建计划 / 表单编辑 / 删除 / 内置目录",
            ),
        ),
        (
            "i / e",
            i18n.pick(
                "Native Grapheme Inline Editor / External $EDITOR",
                "内置字素级编辑器 / 外部 $EDITOR 移交",
            ),
        ),
        (
            "s / v",
            i18n.pick(
                "Safe Sync (with Conflict Guard) / Side-by-Side Diff",
                "安全同步（冲突守卫）/ 并排差异模式",
            ),
        ),
        (
            "r / y",
            i18n.pick(
                "Run Live gh Audit / Copy Fix via OSC 52",
                "执行实时只读 gh 审计 / OSC 52 复制修复",
            ),
        ),
        (
            "t / p / m / L",
            i18n.pick(
                "Cycle Theme / Shader Preset / Motion / EN-ZH",
                "切换主题 / 着色器 / 动效 / 中英文",
            ),
        ),
        (
            "~ / ? / q",
            i18n.pick(
                "Toggle Redacted Logs Drawer / Help / Quit",
                "脱敏日志抽屉 / 帮助 / 退出",
            ),
        ),
    ];

    let lines: Vec<Line<'static>> = rows
        .into_iter()
        .map(|(key, desc)| {
            Line::from(vec![
                Span::styled(format!("  {key:<16}"), theme.keycap_style()),
                Span::styled(desc, Style::default().fg(theme.text_primary)),
            ])
        })
        .collect();

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

pub(super) fn shadowed_modal_block(
    title: String,
    border_color: ratatui::style::Color,
    theme: &crate::theme::Theme,
) -> Block<'static> {
    Block::bordered()
        .title(Line::from(Span::styled(
            title,
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD),
        )))
        .border_type(theme.glyphs.focused_border)
        .border_style(
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD),
        )
        .style(theme.modal_style())
        .shadow(theme.modal_shadow())
}

fn centered_rect(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let [_, vertical, _] = Layout::vertical([
        Constraint::Percentage(100u16.saturating_sub(percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage(100u16.saturating_sub(percent_y) / 2),
    ])
    .areas(area);
    let [_, center, _] = Layout::horizontal([
        Constraint::Percentage(100u16.saturating_sub(percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage(100u16.saturating_sub(percent_x) / 2),
    ])
    .areas(vertical);
    center
}
