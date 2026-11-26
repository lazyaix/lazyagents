//! Viewport, motion, glyph-fallback, and color-profile configuration for `la-tui`.

/// Default target animation refresh rate (frames per second) when shaders or spinners are active.
pub const DEFAULT_ACTIVE_FPS: u16 = 60;

/// Viewport presentation mode for the terminal workbench.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ViewportMode {
    /// Fullscreen alternate-screen multi-pane workbench.
    #[default]
    Fullscreen,
    /// Inline conversation viewport preserving native terminal scrollback.
    Inline {
        /// Fixed viewport height in terminal rows.
        height: u16,
    },
}

/// Visual motion intensity for `TachyonFX` shader pipelines and transitions.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MotionLevel {
    /// Full L4 shader animations, transitions, and ambient glow.
    #[default]
    Full,
    /// Reduced motion favoring crisp instant transitions.
    Reduced,
    /// Disables all frame-driven animations (strictly event-driven redraws).
    None,
}

/// Icon and box-drawing glyph fallback tier.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GlyphSet {
    /// Nerd Font icons and rich Unicode glyphs.
    #[default]
    NerdFont,
    /// Standard Unicode box-drawing and Braille glyphs without Nerd Font requirements.
    Unicode,
    /// Pure ASCII fallback for constrained terminals.
    Ascii,
}

/// Terminal color capability degradation tier.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ColorProfile {
    /// 24-bit RGB `TrueColor`.
    #[default]
    TrueColor,
    /// 256-color xterm palette.
    Ansi256,
    /// Basic 16-color ANSI palette.
    Ansi16,
    /// Monochrome output honoring `NO_COLOR`.
    NoColor,
}

/// Runtime configuration for `la-tui`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TuiConfig {
    /// Presentation viewport strategy.
    pub viewport: ViewportMode,
    /// Animation and shader motion tier.
    pub motion: MotionLevel,
    /// Icon and border glyph set.
    pub glyphs: GlyphSet,
    /// Active terminal color profile.
    pub color_profile: ColorProfile,
    /// Target active frame rate when animations are running.
    pub active_fps: u16,
    /// Whether to display the skippable animated startup splash banner.
    pub show_splash: bool,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            viewport: ViewportMode::Fullscreen,
            motion: MotionLevel::Full,
            glyphs: GlyphSet::NerdFont,
            color_profile: ColorProfile::TrueColor,
            active_fps: DEFAULT_ACTIVE_FPS,
            show_splash: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use core::assert_matches;

    use super::*;

    #[test]
    fn default_tui_config_targets_l4_fullscreen() {
        let config = TuiConfig::default();
        assert_matches!(config.viewport, ViewportMode::Fullscreen);
        assert_matches!(config.motion, MotionLevel::Full);
        assert_matches!(config.glyphs, GlyphSet::NerdFont);
        assert_matches!(config.color_profile, ColorProfile::TrueColor);
        assert_eq!(config.active_fps, DEFAULT_ACTIVE_FPS);
        assert!(config.show_splash);
    }
}
