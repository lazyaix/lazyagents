//! Animated Ultra Cyberpunk startup splash powered by `tui-big-text` 0.8 + `TachyonFX`.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Gauge, Paragraph};
use tui_big_text::{BigText, PixelSize};

use crate::config::ColorProfile;
use crate::state::AppState;

/// Renders the full-screen skippable Ultra Cyberpunk `LAZYAGENTS` startup splash.
pub(super) fn render(state: &AppState, frame: &mut Frame<'_>, area: Rect) {
    let theme = &state.theme;
    let i18n = state.i18n();

    let outer = Block::bordered()
        .border_type(theme.glyphs.focused_border)
        .border_style(theme.border_style(true))
        .style(theme.canvas_style());
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let [_, wordmark_area, subtitle_area, gauge_area, hint_area, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(6),
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Fill(1),
    ])
    .areas(inner);

    let pixel_size = if inner.width >= 84 {
        PixelSize::HalfHeight
    } else {
        PixelSize::Quadrant
    };

    let big_text = BigText::builder()
        .pixel_size(pixel_size)
        .centered()
        .lines(vec![Line::from(vec![
            Span::styled(
                "LAZY",
                Style::default()
                    .fg(theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "AGENTS",
                Style::default()
                    .fg(theme.accent_secondary)
                    .add_modifier(Modifier::BOLD),
            ),
        ])])
        .build();
    frame.render_widget(big_text, wordmark_area);

    let subtitle = Paragraph::new(Line::from(vec![
        Span::styled(
            format!("{} ", theme.glyphs.brand),
            Style::default().fg(theme.accent_primary),
        ),
        Span::styled(
            i18n.studio_subtitle(),
            Style::default()
                .fg(theme.text_secondary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  v{}", env!("CARGO_PKG_VERSION")),
            theme.muted_style(),
        ),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(subtitle, subtitle_area);

    let pct = u16::try_from(
        state
            .splash_elapsed_ms
            .saturating_mul(100)
            .checked_div(540)
            .unwrap_or(100)
            .min(100),
    )
    .unwrap_or(100);
    let [_, centered_gauge, _] = Layout::horizontal([
        Constraint::Percentage(20),
        Constraint::Percentage(60),
        Constraint::Percentage(20),
    ])
    .areas(gauge_area);
    let gauge_style = if theme.color_profile == ColorProfile::NoColor {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
            .fg(theme.accent_primary)
            .bg(theme.bg_surface_elevated)
    };
    let gauge = Gauge::default()
        .gauge_style(gauge_style)
        .percent(pct)
        .label(format!(
            "BOOT // SHADER:{} // {pct}%",
            state.config.shader_preset
        ));
    frame.render_widget(gauge, centered_gauge);

    let hint = Paragraph::new(Line::from(Span::styled(
        i18n.splash_hint(),
        theme.muted_style(),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(hint, hint_area);
}
