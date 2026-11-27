//! Semantic design token theme engine with `TrueColor -> Ansi256 -> Ansi16 -> NoColor`
//! degradation and `NerdFont -> Unicode -> Ascii` glyph palettes.
//!
//! Widgets MUST read all colors and glyphs from [`Theme`] and never hardcode `Color::*`.

use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{BorderType, Shadow};

use crate::config::{ColorProfile, GlyphSet, ThemeId};

/// Resolved concrete theme variant (after resolving [`ThemeId::Auto`]).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResolvedTheme {
    /// Flagship Ultra Cyberpunk HUD dark obsidian + electric cyan + neon magenta theme.
    CyberNeon,
    /// Deep indigo night theme inspired by Tokyo Night.
    TokyoNight,
    /// Warm dark pastel theme inspired by Catppuccin Mocha.
    CatppuccinMocha,
    /// High-contrast daylight theme inspired by Catppuccin Latte.
    CatppuccinLatte,
}

/// Complete icon and box-glyph vocabulary adapted to the active [`GlyphSet`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GlyphPalette {
    /// Primary brand mark shown in the top HUD bar.
    pub brand: &'static str,
    /// Focus indicator arrow on selected list rows.
    pub cursor: &'static str,
    /// Icon for HUD Dashboard / Overview.
    pub dashboard: &'static str,
    /// Icon for Engineering Governance & GH Audit.
    pub governance: &'static str,
    /// Icon for `AGENTS.md` Instructions.
    pub instructions: &'static str,
    /// Icon for Conditional Rules.
    pub rules: &'static str,
    /// Icon for Skills.
    pub skills: &'static str,
    /// Icon for Custom Subagents.
    pub agents: &'static str,
    /// Icon for Lifecycle Hooks & Playground.
    pub hooks: &'static str,
    /// Icon for MCP Servers.
    pub mcp: &'static str,
    /// Icon for Index Manifests.
    pub manifests: &'static str,
    /// Icon for Project Plugins.
    pub plugins: &'static str,
    /// Icon for Sync & Diff view.
    pub diff: &'static str,
    /// Badge glyph for healthy / pass status.
    pub ok: &'static str,
    /// Badge glyph for warning / ask status.
    pub warn: &'static str,
    /// Badge glyph for error / deny status.
    pub err: &'static str,
    /// Badge glyph for locked / protected items.
    pub lock: &'static str,
    /// Toggle-on indicator.
    pub toggle_on: &'static str,
    /// Toggle-off indicator.
    pub toggle_off: &'static str,
    /// Section separator bar.
    pub sep: &'static str,
    /// Truncation ellipsis string.
    pub ellipsis: &'static str,
    /// Horizontal meter fill block.
    pub bar_fill: &'static str,
    /// Horizontal meter empty track block.
    pub bar_empty: &'static str,
    /// Border style for active/focused panes.
    pub focused_border: BorderType,
    /// Border style for inactive panes.
    pub muted_border: BorderType,
}

/// Semantic color and glyph design tokens shared across all `la-tui` widgets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Theme {
    /// Concrete theme identity.
    pub variant: ResolvedTheme,
    /// Active terminal color capability tier.
    pub color_profile: ColorProfile,
    /// Active glyph fallback tier.
    pub glyph_set: GlyphSet,
    /// Glyph vocabulary for icons, indicators, and borders.
    pub glyphs: GlyphPalette,

    /// Base canvas background.
    pub bg_canvas: Color,
    /// Primary pane/card surface background.
    pub bg_surface: Color,
    /// Elevated floating modal or command palette surface background.
    pub bg_surface_elevated: Color,
    /// Dimmed overlay scrim background behind modals.
    pub bg_overlay_scrim: Color,
    /// Highlighted row / active selection background.
    pub bg_selection: Color,

    /// Focused pane border color (cyber neon accent).
    pub border_focused: Color,
    /// Inactive pane border color.
    pub border_muted: Color,
    /// Secondary highlight border color (e.g., HUD badges, modals).
    pub border_accent: Color,
    /// Drop shadow fill color used by `Block::shadow(...)`.
    pub shadow_color: Color,

    /// High-contrast primary foreground text.
    pub text_primary: Color,
    /// Secondary body foreground text.
    pub text_secondary: Color,
    /// De-emphasized hints, captions, and line numbers.
    pub text_muted: Color,
    /// Inverted foreground text rendered on vibrant accent pills.
    pub text_inverse: Color,

    /// Primary brand accent (electric cyan in `CyberNeon`).
    pub accent_primary: Color,
    /// Secondary brand accent (neon magenta in `CyberNeon`).
    pub accent_secondary: Color,
    /// Luminous telemetry / chart glow accent.
    pub accent_glow: Color,

    /// Idle / neutral state color.
    pub status_idle: Color,
    /// Thinking / background computation color.
    pub status_thinking: Color,
    /// Running / active task color.
    pub status_running: Color,
    /// Human approval / `ask` / `force_ask` color.
    pub status_approval: Color,
    /// Success / `allow` / clean pass color.
    pub status_success: Color,
    /// Warning / drift / >=80% budget color.
    pub status_warn: Color,
    /// Error / `deny` / exceeded budget color.
    pub status_error: Color,

    /// Added diff line foreground.
    pub diff_add_fg: Color,
    /// Added diff line background.
    pub diff_add_bg: Color,
    /// Removed diff line foreground.
    pub diff_del_fg: Color,
    /// Removed diff line background.
    pub diff_del_bg: Color,
    /// Diff hunk header color.
    pub diff_hunk: Color,

    /// Preferred built-in `syntect` theme name for code previews.
    pub syntax_theme: &'static str,
}

#[derive(Clone, Copy)]
struct RawPalette {
    bg_canvas: (u8, u8, u8),
    bg_surface: (u8, u8, u8),
    bg_surface_elevated: (u8, u8, u8),
    bg_overlay_scrim: (u8, u8, u8),
    bg_selection: (u8, u8, u8),
    border_focused: (u8, u8, u8),
    border_muted: (u8, u8, u8),
    border_accent: (u8, u8, u8),
    shadow_color: (u8, u8, u8),
    text_primary: (u8, u8, u8),
    text_secondary: (u8, u8, u8),
    text_muted: (u8, u8, u8),
    text_inverse: (u8, u8, u8),
    accent_primary: (u8, u8, u8),
    accent_secondary: (u8, u8, u8),
    accent_glow: (u8, u8, u8),
    status_idle: (u8, u8, u8),
    status_thinking: (u8, u8, u8),
    status_running: (u8, u8, u8),
    status_approval: (u8, u8, u8),
    status_success: (u8, u8, u8),
    status_warn: (u8, u8, u8),
    status_error: (u8, u8, u8),
    diff_add_fg: (u8, u8, u8),
    diff_add_bg: (u8, u8, u8),
    diff_del_fg: (u8, u8, u8),
    diff_del_bg: (u8, u8, u8),
    diff_hunk: (u8, u8, u8),
    syntax_theme: &'static str,
}

impl ResolvedTheme {
    /// Resolves a user [`ThemeId`] using an OSC 11 dark/light background hint.
    #[must_use]
    pub const fn resolve(id: ThemeId, terminal_is_light: bool) -> Self {
        match id {
            ThemeId::Auto => {
                if terminal_is_light {
                    Self::CatppuccinLatte
                } else {
                    Self::CyberNeon
                }
            }
            ThemeId::CyberNeon => Self::CyberNeon,
            ThemeId::TokyoNight => Self::TokyoNight,
            ThemeId::CatppuccinMocha => Self::CatppuccinMocha,
            ThemeId::CatppuccinLatte => Self::CatppuccinLatte,
        }
    }

    /// Returns `true` for light-background daylight themes.
    #[must_use]
    pub const fn is_light(self) -> bool {
        matches!(self, Self::CatppuccinLatte)
    }

    /// Returns the human-readable theme title.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::CyberNeon => "Cyber Neon",
            Self::TokyoNight => "Tokyo Night",
            Self::CatppuccinMocha => "Catppuccin Mocha",
            Self::CatppuccinLatte => "Catppuccin Latte",
        }
    }
}

impl GlyphPalette {
    /// Returns the glyph palette corresponding to `set`.
    #[must_use]
    pub const fn for_set(set: GlyphSet) -> Self {
        match set {
            GlyphSet::NerdFont => NERD_FONT_GLYPHS,
            GlyphSet::Unicode => UNICODE_GLYPHS,
            GlyphSet::Ascii => ASCII_GLYPHS,
        }
    }
}

impl Theme {
    /// Constructs a resolved [`Theme`] adapted to the given color profile and glyph set.
    #[must_use]
    pub const fn build(
        variant: ResolvedTheme,
        color_profile: ColorProfile,
        glyph_set: GlyphSet,
    ) -> Self {
        let raw = raw_palette(variant);
        Self {
            variant,
            color_profile,
            glyph_set,
            glyphs: GlyphPalette::for_set(glyph_set),
            bg_canvas: degrade_rgb(raw.bg_canvas, color_profile, Ansi16Role::BgDark),
            bg_surface: degrade_rgb(raw.bg_surface, color_profile, Ansi16Role::BgDark),
            bg_surface_elevated: degrade_rgb(
                raw.bg_surface_elevated,
                color_profile,
                Ansi16Role::BgElevated,
            ),
            bg_overlay_scrim: degrade_rgb(raw.bg_overlay_scrim, color_profile, Ansi16Role::BgDark),
            bg_selection: degrade_rgb(raw.bg_selection, color_profile, Ansi16Role::Selection),
            border_focused: degrade_rgb(raw.border_focused, color_profile, Ansi16Role::Cyan),
            border_muted: degrade_rgb(raw.border_muted, color_profile, Ansi16Role::Muted),
            border_accent: degrade_rgb(raw.border_accent, color_profile, Ansi16Role::Magenta),
            shadow_color: degrade_rgb(raw.shadow_color, color_profile, Ansi16Role::BgDark),
            text_primary: degrade_rgb(raw.text_primary, color_profile, Ansi16Role::TextPrimary),
            text_secondary: degrade_rgb(
                raw.text_secondary,
                color_profile,
                Ansi16Role::TextSecondary,
            ),
            text_muted: degrade_rgb(raw.text_muted, color_profile, Ansi16Role::Muted),
            text_inverse: degrade_rgb(raw.text_inverse, color_profile, Ansi16Role::BgDark),
            accent_primary: degrade_rgb(raw.accent_primary, color_profile, Ansi16Role::Cyan),
            accent_secondary: degrade_rgb(raw.accent_secondary, color_profile, Ansi16Role::Magenta),
            accent_glow: degrade_rgb(raw.accent_glow, color_profile, Ansi16Role::LightCyan),
            status_idle: degrade_rgb(raw.status_idle, color_profile, Ansi16Role::Muted),
            status_thinking: degrade_rgb(raw.status_thinking, color_profile, Ansi16Role::Magenta),
            status_running: degrade_rgb(raw.status_running, color_profile, Ansi16Role::Cyan),
            status_approval: degrade_rgb(raw.status_approval, color_profile, Ansi16Role::Yellow),
            status_success: degrade_rgb(raw.status_success, color_profile, Ansi16Role::Green),
            status_warn: degrade_rgb(raw.status_warn, color_profile, Ansi16Role::Yellow),
            status_error: degrade_rgb(raw.status_error, color_profile, Ansi16Role::Red),
            diff_add_fg: degrade_rgb(raw.diff_add_fg, color_profile, Ansi16Role::Green),
            diff_add_bg: degrade_rgb(raw.diff_add_bg, color_profile, Ansi16Role::BgDark),
            diff_del_fg: degrade_rgb(raw.diff_del_fg, color_profile, Ansi16Role::Red),
            diff_del_bg: degrade_rgb(raw.diff_del_bg, color_profile, Ansi16Role::BgDark),
            diff_hunk: degrade_rgb(raw.diff_hunk, color_profile, Ansi16Role::Cyan),
            syntax_theme: raw.syntax_theme,
        }
    }

    /// Adapts an arbitrary 24-bit RGB color (such as from `syntect`) to the active [`ColorProfile`].
    #[must_use]
    pub const fn adapt_rgb(&self, r: u8, g: u8, b: u8) -> Color {
        match self.color_profile {
            ColorProfile::TrueColor => Color::Rgb(r, g, b),
            ColorProfile::Ansi256 => Color::Indexed(rgb_to_ansi256(r, g, b)),
            ColorProfile::Ansi16 => rgb_to_ansi16(r, g, b),
            ColorProfile::NoColor => Color::Reset,
        }
    }

    /// Returns the base canvas [`Style`].
    #[must_use]
    pub fn canvas_style(&self) -> Style {
        if self.color_profile == ColorProfile::NoColor {
            Style::default()
        } else {
            Style::default().fg(self.text_primary).bg(self.bg_canvas)
        }
    }

    /// Returns the standard pane surface [`Style`].
    #[must_use]
    pub fn surface_style(&self) -> Style {
        if self.color_profile == ColorProfile::NoColor {
            Style::default()
        } else {
            Style::default().fg(self.text_primary).bg(self.bg_surface)
        }
    }

    /// Returns the elevated modal surface [`Style`].
    #[must_use]
    pub fn modal_style(&self) -> Style {
        if self.color_profile == ColorProfile::NoColor {
            Style::default()
        } else {
            Style::default()
                .fg(self.text_primary)
                .bg(self.bg_surface_elevated)
        }
    }

    /// Returns the focused or muted border [`Style`], maintaining hierarchy in `NO_COLOR`.
    #[must_use]
    pub fn border_style(&self, focused: bool) -> Style {
        if self.color_profile == ColorProfile::NoColor {
            return if focused {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default().add_modifier(Modifier::DIM)
            };
        }
        if focused {
            Style::default()
                .fg(self.border_focused)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.border_muted)
        }
    }

    /// Returns the selected row [`Style`], using `REVERSED` in `NO_COLOR` mode.
    #[must_use]
    pub fn selection_style(&self, focused: bool) -> Style {
        if self.color_profile == ColorProfile::NoColor {
            let modifier = if focused {
                Modifier::REVERSED | Modifier::BOLD
            } else {
                Modifier::UNDERLINED
            };
            return Style::default().add_modifier(modifier);
        }
        if focused {
            Style::default()
                .fg(self.accent_primary)
                .bg(self.bg_selection)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.text_primary).bg(self.bg_selection)
        }
    }

    /// Returns a vibrant status pill [`Style`] (or bold-reversed in `NO_COLOR`).
    #[must_use]
    pub fn badge_style(&self, accent: Color) -> Style {
        if self.color_profile == ColorProfile::NoColor {
            Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        } else {
            Style::default()
                .fg(self.text_inverse)
                .bg(accent)
                .add_modifier(Modifier::BOLD)
        }
    }

    /// Returns the `Ratatui` 0.30.2 [`Shadow`] configured with semantic theme tokens.
    #[must_use]
    pub fn modal_shadow(&self) -> Shadow {
        let base = if self.glyph_set == GlyphSet::Ascii {
            Shadow::overlay()
        } else {
            Shadow::dark_shade()
        }
        .offset(ratatui::layout::Offset::new(2, 1));
        if self.color_profile == ColorProfile::NoColor {
            base.style(Style::default().add_modifier(Modifier::DIM))
        } else {
            base.style(Style::default().fg(self.shadow_color).bg(self.bg_canvas))
        }
    }

    /// Returns a muted text [`Style`].
    #[must_use]
    pub fn muted_style(&self) -> Style {
        if self.color_profile == ColorProfile::NoColor {
            Style::default().add_modifier(Modifier::DIM)
        } else {
            Style::default().fg(self.text_muted)
        }
    }

    /// Returns a keycap shortcut highlight [`Style`].
    #[must_use]
    pub fn keycap_style(&self) -> Style {
        if self.color_profile == ColorProfile::NoColor {
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default()
                .fg(self.accent_primary)
                .add_modifier(Modifier::BOLD)
        }
    }
}

#[derive(Clone, Copy)]
enum Ansi16Role {
    BgDark,
    BgElevated,
    Selection,
    TextPrimary,
    TextSecondary,
    Muted,
    Cyan,
    LightCyan,
    Magenta,
    Green,
    Yellow,
    Red,
}

const fn degrade_rgb(rgb: (u8, u8, u8), profile: ColorProfile, role: Ansi16Role) -> Color {
    match profile {
        ColorProfile::TrueColor => Color::Rgb(rgb.0, rgb.1, rgb.2),
        ColorProfile::Ansi256 => Color::Indexed(rgb_to_ansi256(rgb.0, rgb.1, rgb.2)),
        ColorProfile::Ansi16 => match role {
            Ansi16Role::BgDark => Color::Black,
            Ansi16Role::BgElevated | Ansi16Role::Muted => Color::DarkGray,
            Ansi16Role::Selection => Color::Blue,
            Ansi16Role::TextPrimary => Color::White,
            Ansi16Role::TextSecondary => Color::Gray,
            Ansi16Role::Cyan => Color::Cyan,
            Ansi16Role::LightCyan => Color::LightCyan,
            Ansi16Role::Magenta => Color::Magenta,
            Ansi16Role::Green => Color::Green,
            Ansi16Role::Yellow => Color::Yellow,
            Ansi16Role::Red => Color::Red,
        },
        ColorProfile::NoColor => Color::Reset,
    }
}

/// Quantizes a 24-bit RGB triple into the nearest xterm-256 color index (6x6x6 cube + 24 grayscale).
#[must_use]
pub const fn rgb_to_ansi256(r: u8, g: u8, b: u8) -> u8 {
    if r == g && g == b {
        if r < 8 {
            return 16;
        }
        if r > 248 {
            return 231;
        }
        let step = ((r as u16).saturating_sub(8).saturating_mul(24)) / 247;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "step is bounded by 23 ((248 - 8) * 24 / 247 = 23)"
        )]
        return 232u8.saturating_add(step as u8);
    }
    let ri = ((r as u16).saturating_mul(5).saturating_add(127)) / 255;
    let gi = ((g as u16).saturating_mul(5).saturating_add(127)) / 255;
    let bi = ((b as u16).saturating_mul(5).saturating_add(127)) / 255;
    let idx = 16u16
        .saturating_add(ri.saturating_mul(36))
        .saturating_add(gi.saturating_mul(6))
        .saturating_add(bi);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "6x6x6 cube index is bounded by 16 + 5*36 + 5*6 + 5 = 231 <= u8::MAX"
    )]
    {
        idx as u8
    }
}

const fn rgb_to_ansi16(r: u8, g: u8, b: u8) -> Color {
    let bright = r > 170 || g > 170 || b > 170;
    let has_r = r >= 96;
    let has_g = g >= 96;
    let has_b = b >= 96;
    match (has_r, has_g, has_b, bright) {
        (false, false, false, _) => Color::DarkGray,
        (true, false, false, true) => Color::LightRed,
        (true, false, false, false) => Color::Red,
        (false, true, false, true) => Color::LightGreen,
        (false, true, false, false) => Color::Green,
        (false, false, true, true) => Color::LightBlue,
        (false, false, true, false) => Color::Blue,
        (true, true, false, true) => Color::LightYellow,
        (true, true, false, false) => Color::Yellow,
        (true, false, true, true) => Color::LightMagenta,
        (true, false, true, false) => Color::Magenta,
        (false, true, true, true) => Color::LightCyan,
        (false, true, true, false) => Color::Cyan,
        (true, true, true, true) => Color::White,
        (true, true, true, false) => Color::Gray,
    }
}

const fn raw_palette(variant: ResolvedTheme) -> RawPalette {
    match variant {
        ResolvedTheme::CyberNeon => CYBER_NEON_PALETTE,
        ResolvedTheme::TokyoNight => TOKYO_NIGHT_PALETTE,
        ResolvedTheme::CatppuccinMocha => CATPPUCCIN_MOCHA_PALETTE,
        ResolvedTheme::CatppuccinLatte => CATPPUCCIN_LATTE_PALETTE,
    }
}

const CYBER_NEON_PALETTE: RawPalette = RawPalette {
    bg_canvas: (7, 9, 19),
    bg_surface: (13, 17, 33),
    bg_surface_elevated: (20, 27, 50),
    bg_overlay_scrim: (4, 5, 11),
    bg_selection: (24, 48, 76),
    border_focused: (0, 240, 255),
    border_muted: (45, 58, 92),
    border_accent: (255, 42, 133),
    shadow_color: (2, 3, 8),
    text_primary: (234, 246, 255),
    text_secondary: (166, 189, 219),
    text_muted: (98, 118, 154),
    text_inverse: (7, 9, 19),
    accent_primary: (0, 240, 255),
    accent_secondary: (255, 42, 133),
    accent_glow: (125, 249, 255),
    status_idle: (106, 126, 163),
    status_thinking: (189, 110, 255),
    status_running: (0, 240, 255),
    status_approval: (255, 184, 0),
    status_success: (57, 255, 136),
    status_warn: (255, 184, 0),
    status_error: (255, 56, 100),
    diff_add_fg: (92, 255, 163),
    diff_add_bg: (12, 42, 31),
    diff_del_fg: (255, 102, 138),
    diff_del_bg: (48, 15, 28),
    diff_hunk: (0, 212, 255),
    syntax_theme: "base16-ocean.dark",
};

const TOKYO_NIGHT_PALETTE: RawPalette = RawPalette {
    bg_canvas: (22, 22, 30),
    bg_surface: (26, 27, 38),
    bg_surface_elevated: (36, 40, 59),
    bg_overlay_scrim: (15, 15, 22),
    bg_selection: (41, 46, 66),
    border_focused: (122, 162, 247),
    border_muted: (59, 66, 97),
    border_accent: (187, 154, 247),
    shadow_color: (10, 10, 16),
    text_primary: (192, 202, 245),
    text_secondary: (169, 177, 214),
    text_muted: (86, 95, 137),
    text_inverse: (22, 22, 30),
    accent_primary: (122, 162, 247),
    accent_secondary: (187, 154, 247),
    accent_glow: (125, 207, 255),
    status_idle: (86, 95, 137),
    status_thinking: (187, 154, 247),
    status_running: (125, 207, 255),
    status_approval: (224, 175, 104),
    status_success: (158, 206, 106),
    status_warn: (224, 175, 104),
    status_error: (247, 118, 142),
    diff_add_fg: (158, 206, 106),
    diff_add_bg: (29, 43, 33),
    diff_del_fg: (247, 118, 142),
    diff_del_bg: (51, 27, 37),
    diff_hunk: (125, 207, 255),
    syntax_theme: "base16-ocean.dark",
};

const CATPPUCCIN_MOCHA_PALETTE: RawPalette = RawPalette {
    bg_canvas: (24, 24, 37),
    bg_surface: (30, 30, 46),
    bg_surface_elevated: (49, 50, 68),
    bg_overlay_scrim: (17, 17, 27),
    bg_selection: (69, 71, 90),
    border_focused: (137, 220, 235),
    border_muted: (88, 91, 112),
    border_accent: (203, 166, 247),
    shadow_color: (12, 12, 20),
    text_primary: (205, 214, 244),
    text_secondary: (186, 194, 222),
    text_muted: (108, 112, 134),
    text_inverse: (24, 24, 37),
    accent_primary: (137, 220, 235),
    accent_secondary: (203, 166, 247),
    accent_glow: (148, 226, 213),
    status_idle: (108, 112, 134),
    status_thinking: (203, 166, 247),
    status_running: (137, 180, 250),
    status_approval: (249, 226, 175),
    status_success: (166, 227, 161),
    status_warn: (250, 179, 135),
    status_error: (243, 139, 168),
    diff_add_fg: (166, 227, 161),
    diff_add_bg: (32, 48, 38),
    diff_del_fg: (243, 139, 168),
    diff_del_bg: (54, 31, 41),
    diff_hunk: (137, 220, 235),
    syntax_theme: "base16-mocha.dark",
};

const CATPPUCCIN_LATTE_PALETTE: RawPalette = RawPalette {
    bg_canvas: (239, 241, 245),
    bg_surface: (230, 233, 239),
    bg_surface_elevated: (255, 255, 255),
    bg_overlay_scrim: (204, 208, 218),
    bg_selection: (212, 224, 247),
    border_focused: (4, 165, 229),
    border_muted: (172, 176, 190),
    border_accent: (136, 57, 239),
    shadow_color: (156, 160, 176),
    text_primary: (76, 79, 105),
    text_secondary: (92, 95, 119),
    text_muted: (124, 127, 147),
    text_inverse: (255, 255, 255),
    accent_primary: (4, 165, 229),
    accent_secondary: (136, 57, 239),
    accent_glow: (23, 146, 153),
    status_idle: (124, 127, 147),
    status_thinking: (136, 57, 239),
    status_running: (30, 102, 245),
    status_approval: (223, 142, 29),
    status_success: (64, 160, 43),
    status_warn: (254, 100, 11),
    status_error: (210, 15, 57),
    diff_add_fg: (64, 160, 43),
    diff_add_bg: (219, 242, 219),
    diff_del_fg: (210, 15, 57),
    diff_del_bg: (247, 220, 226),
    diff_hunk: (4, 165, 229),
    syntax_theme: "InspiredGitHub",
};

const NERD_FONT_GLYPHS: GlyphPalette = GlyphPalette {
    brand: "⚡",
    cursor: "▸",
    dashboard: "󰕮",
    governance: "󰒃",
    instructions: "󰈙",
    rules: "󰓹",
    skills: "󰯙",
    agents: "󰚩",
    hooks: "󰛢",
    mcp: "󰒍",
    manifests: "󰘦",
    plugins: "󰐱",
    diff: "󰦓",
    ok: "✔",
    warn: "▲",
    err: "✖",
    lock: "󰌾",
    toggle_on: "●",
    toggle_off: "○",
    sep: "│",
    ellipsis: "…",
    bar_fill: "█",
    bar_empty: "░",
    focused_border: BorderType::Thick,
    muted_border: BorderType::Rounded,
};

const UNICODE_GLYPHS: GlyphPalette = GlyphPalette {
    brand: "◆",
    cursor: "▸",
    dashboard: "◈",
    governance: "❖",
    instructions: "▤",
    rules: "§",
    skills: "★",
    agents: "◎",
    hooks: "⚡",
    mcp: "⌘",
    manifests: "≡",
    plugins: "⊞",
    diff: "±",
    ok: "✓",
    warn: "▲",
    err: "✕",
    lock: "⛨",
    toggle_on: "●",
    toggle_off: "○",
    sep: "│",
    ellipsis: "…",
    bar_fill: "█",
    bar_empty: "░",
    focused_border: BorderType::Thick,
    muted_border: BorderType::Rounded,
};

const ASCII_GLYPHS: GlyphPalette = GlyphPalette {
    brand: "*",
    cursor: ">",
    dashboard: "#",
    governance: "G",
    instructions: "A",
    rules: "R",
    skills: "S",
    agents: "@",
    hooks: "H",
    mcp: "M",
    manifests: "J",
    plugins: "P",
    diff: "D",
    ok: "OK",
    warn: "!",
    err: "X",
    lock: "L",
    toggle_on: "[x]",
    toggle_off: "[ ]",
    sep: "|",
    ellipsis: "...",
    bar_fill: "#",
    bar_empty: "-",
    focused_border: BorderType::Plain,
    muted_border: BorderType::Plain,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_auto_theme_from_osc11_hint_and_degrades_cleanly_to_no_color() {
        assert_eq!(
            ResolvedTheme::resolve(ThemeId::Auto, false),
            ResolvedTheme::CyberNeon
        );
        assert_eq!(
            ResolvedTheme::resolve(ThemeId::Auto, true),
            ResolvedTheme::CatppuccinLatte
        );
        assert!(ResolvedTheme::CatppuccinLatte.is_light());
        assert!(!ResolvedTheme::CyberNeon.is_light());

        let no_color = Theme::build(
            ResolvedTheme::CyberNeon,
            ColorProfile::NoColor,
            GlyphSet::Ascii,
        );
        assert_eq!(no_color.accent_primary, Color::Reset);
        assert_eq!(no_color.bg_canvas, Color::Reset);
        assert_eq!(no_color.adapt_rgb(0, 240, 255), Color::Reset);
        assert!(
            no_color
                .selection_style(true)
                .add_modifier
                .contains(Modifier::REVERSED)
        );
        assert_eq!(no_color.glyphs.ellipsis, "...");
    }

    #[test]
    fn ansi256_quantization_maps_pure_primaries_and_grays_deterministically() {
        assert_eq!(rgb_to_ansi256(0, 0, 0), 16);
        assert_eq!(rgb_to_ansi256(255, 255, 255), 231);
        let cyan = rgb_to_ansi256(0, 240, 255);
        assert!((16..=231).contains(&cyan));
        let gray = rgb_to_ansi256(128, 128, 128);
        assert!((232..=255).contains(&gray));

        let theme_256 = Theme::build(
            ResolvedTheme::TokyoNight,
            ColorProfile::Ansi256,
            GlyphSet::Unicode,
        );
        assert!(matches!(theme_256.accent_primary, Color::Indexed(_)));
    }
}
