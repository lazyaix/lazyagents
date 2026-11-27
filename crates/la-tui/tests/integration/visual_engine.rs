//! Deterministic `TestBackend` + `insta` snapshot and visual degradation tests for the PR 1
//! L4 visual engine (`Theme`, `I18n`, `StudioEffects`, `PreviewRenderer`, and `TextEditor`).

use core::time::Duration;

use la_core::PolicyDecision;
use la_tui::{
    ColorProfile, EditorAction, GlyphSet, I18n, MotionLevel, PreviewRenderer, PreviewSyntax,
    ResolvedTheme, ShaderPreset, StudioEffects, TextEditor, Theme, UiLang,
    render_unified_diff_styled,
};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::{Block, Paragraph};

fn buffer_to_text(terminal: &Terminal<TestBackend>) -> String {
    let buf = terminal.backend().buffer();
    let area = buf.area;
    let mut out = String::new();
    for y in area.top()..area.bottom() {
        let mut line = String::new();
        for x in area.left()..area.right() {
            line.push_str(buf[(x, y)].symbol());
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

#[test]
fn snapshot_markdown_and_editor_with_cjk_emoji_and_shadow_block() {
    let theme = Theme::build(
        ResolvedTheme::CyberNeon,
        ColorProfile::TrueColor,
        GlyphSet::Unicode,
    );
    let zh = I18n::new(UiLang::Zh);
    let mut renderer = PreviewRenderer::new();
    let md = "# 赛博规范 🚀\n\n- 禁止直推 `main`\n- 启用 **Worktree 隔离**\n\n```toml\nschema_version = 1\n```\n";
    let md_lines = renderer
        .render_cached(md, PreviewSyntax::Markdown, 36, &theme)
        .to_vec();

    let mut editor =
        TextEditor::from_text("第一行：赛博配置🚀\n第二行：git-guard 拦截\nThird line: OK");
    editor.apply(EditorAction::MoveDown);
    editor.apply(EditorAction::MoveRight);
    editor.apply(EditorAction::MoveRight);

    let backend = TestBackend::new(80, 14);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let [left, right] =
                Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .areas(frame.area());

            let preview_block = Block::bordered()
                .title(format!(
                    " {} {} ",
                    theme.glyphs.instructions,
                    zh.inspector_preview()
                ))
                .border_type(theme.glyphs.muted_border)
                .border_style(theme.border_style(false))
                .style(theme.surface_style());
            frame.render_widget(Paragraph::new(md_lines).block(preview_block), left);

            let editor_block = Block::bordered()
                .title(format!(
                    " {} {} ",
                    theme.glyphs.cursor,
                    zh.editor_mode_insert()
                ))
                .border_type(theme.glyphs.focused_border)
                .border_style(theme.border_style(true))
                .style(theme.surface_style())
                .shadow(theme.modal_shadow());
            editor.render_into(right, frame.buffer_mut(), Some(editor_block), &theme, true);
        })
        .unwrap();

    let snapshot_text = buffer_to_text(&terminal);
    insta::assert_snapshot!("cjk_preview_and_editor_80x14", snapshot_text);
}

#[test]
fn snapshot_no_color_ascii_unified_diff_and_deterministic_shader_step() {
    let theme = Theme::build(
        ResolvedTheme::CyberNeon,
        ColorProfile::NoColor,
        GlyphSet::Ascii,
    );
    let en = I18n::new(UiLang::En);
    let diff = "--- a/AGENTS.md\n+++ b/AGENTS.md\n@@ -1,2 +1,2 @@\n-old rule\n+new cyber rule\n";
    let styled = render_unified_diff_styled(diff, &theme);

    let mut effects = StudioEffects::new(ShaderPreset::CyberOverdrive, MotionLevel::Full);
    let backend = TestBackend::new(60, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    let area = terminal.size().unwrap();
    let rect = ratatui::layout::Rect::new(0, 0, area.width, area.height);
    effects.trigger_verdict_flash(rect, PolicyDecision::Deny, &theme);

    terminal
        .draw(|frame| {
            let view_area = frame.area();
            let block = Block::bordered()
                .title(format!(" {} {} ", theme.glyphs.diff, en.inspector_diff()))
                .border_type(theme.glyphs.focused_border)
                .border_style(theme.border_style(true))
                .style(theme.surface_style());
            frame.render_widget(Paragraph::new(styled).block(block), view_area);
            effects.process_frame(
                Duration::from_millis(16),
                frame.buffer_mut(),
                view_area,
                Some(view_area),
                &theme,
            );
        })
        .unwrap();

    let snapshot_text = buffer_to_text(&terminal);
    insta::assert_snapshot!("no_color_ascii_diff_60x10", snapshot_text);
}
