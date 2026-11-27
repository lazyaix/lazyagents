//! Viewport, theme, shader-preset, motion, language, glyph-fallback, and color-profile
//! configuration for `la-tui`.

use core::fmt;

use serde::{Deserialize, Serialize};

/// Default target animation refresh rate (frames per second) when shaders or spinners are active.
pub const DEFAULT_ACTIVE_FPS: u16 = 60;

/// Default inline viewport height in terminal rows when `--inline` is requested without an explicit value.
pub const DEFAULT_INLINE_HEIGHT: u16 = 18;

/// Viewport presentation mode for the terminal workbench.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ViewportMode {
    /// Fullscreen alternate-screen multi-pane Command Deck.
    #[default]
    Fullscreen,
    /// Inline compact viewport preserving native terminal scrollback.
    Inline {
        /// Fixed viewport height in terminal rows.
        height: u16,
    },
}

/// Visual motion intensity for `TachyonFX` shader pipelines and transitions.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MotionLevel {
    /// Full L4 shader animations, transitions, and ambient neon glow.
    #[default]
    Full,
    /// Reduced motion favoring crisp instant color transitions.
    Reduced,
    /// Disables all frame-driven animations (strictly event-driven redraws).
    None,
}

/// Curated `TachyonFX` visual shader intensity preset.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShaderPreset {
    /// Ultra Cyberpunk HUD: scanline/matrix coalesce intro, neon pulsing focused borders, chromatic transitions.
    #[default]
    CyberOverdrive,
    /// Smooth bi-directional HSL aurora sweeps and subtle breathing highlights.
    AuroraSweep,
    /// Ultra-crisp minimal transitions with modal dimming scrim only.
    CrispMinimal,
}

/// Selectable built-in color theme identifier.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeId {
    /// Automatically choose dark (`CyberNeon`) or light (`CatppuccinLatte`) via OSC 11 probing.
    #[default]
    Auto,
    /// Flagship Ultra Cyberpunk HUD obsidian + electric cyan + neon magenta palette.
    CyberNeon,
    /// Deep indigo night palette inspired by Tokyo Night.
    TokyoNight,
    /// Warm dark pastel palette inspired by Catppuccin Mocha.
    CatppuccinMocha,
    /// High-contrast daylight palette inspired by Catppuccin Latte.
    CatppuccinLatte,
}

/// Built-in TUI interface language (`en` default, live toggleable to `zh`).
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UiLang {
    /// English interface labels.
    #[default]
    En,
    /// Simplified Chinese interface labels.
    Zh,
}

/// Icon and box-drawing glyph fallback tier.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GlyphSet {
    /// Nerd Font icons and rich Unicode box/geometric glyphs.
    #[default]
    NerdFont,
    /// Standard Unicode box-drawing and Braille glyphs without Nerd Font requirements.
    Unicode,
    /// Pure ASCII fallback for constrained terminals.
    Ascii,
}

/// Terminal color capability degradation tier.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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

/// Runtime visual and behavioral configuration for `la-tui`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TuiConfig {
    /// Presentation viewport strategy.
    pub viewport: ViewportMode,
    /// Active built-in theme selection (or `Auto` via OSC 11).
    pub theme: ThemeId,
    /// Curated `TachyonFX` shader intensity preset.
    pub shader_preset: ShaderPreset,
    /// Animation and shader motion tier.
    pub motion: MotionLevel,
    /// Active interface language (`en` or `zh`).
    pub lang: UiLang,
    /// Icon and border glyph set.
    pub glyphs: GlyphSet,
    /// Active terminal color profile.
    pub color_profile: ColorProfile,
    /// Target active frame rate when animations are running.
    pub active_fps: u16,
    /// Whether to display the skippable animated startup splash banner.
    pub show_splash: bool,
    /// Whether mouse capture and click/scroll interaction are enabled.
    pub mouse_enabled: bool,
}

impl ShaderPreset {
    /// Cycles to the next shader preset (`CyberOverdrive -> AuroraSweep -> CrispMinimal -> CyberOverdrive`).
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::CyberOverdrive => Self::AuroraSweep,
            Self::AuroraSweep => Self::CrispMinimal,
            Self::CrispMinimal => Self::CyberOverdrive,
        }
    }

    /// Returns the canonical kebab-case preset identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CyberOverdrive => "cyber-overdrive",
            Self::AuroraSweep => "aurora-sweep",
            Self::CrispMinimal => "crisp-minimal",
        }
    }
}

impl fmt::Display for ShaderPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl ThemeId {
    /// Cycles through concrete themes (`CyberNeon -> TokyoNight -> CatppuccinMocha -> CatppuccinLatte -> CyberNeon`).
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Auto | Self::CatppuccinLatte => Self::CyberNeon,
            Self::CyberNeon => Self::TokyoNight,
            Self::TokyoNight => Self::CatppuccinMocha,
            Self::CatppuccinMocha => Self::CatppuccinLatte,
        }
    }

    /// Returns the canonical kebab-case identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::CyberNeon => "cyber-neon",
            Self::TokyoNight => "tokyo-night",
            Self::CatppuccinMocha => "catppuccin-mocha",
            Self::CatppuccinLatte => "catppuccin-latte",
        }
    }
}

impl fmt::Display for ThemeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl UiLang {
    /// Toggles between English (`En`) and Simplified Chinese (`Zh`).
    #[must_use]
    pub const fn toggle(self) -> Self {
        match self {
            Self::En => Self::Zh,
            Self::Zh => Self::En,
        }
    }

    /// Returns the short BCP-47 code (`"en"` or `"zh"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Zh => "zh",
        }
    }

    /// Detects the preferred language from an environment locale string (e.g., `"zh_CN.UTF-8"`).
    #[must_use]
    pub fn from_locale_hint(locale: &str) -> Self {
        let lower = locale.trim().to_ascii_lowercase();
        if lower.starts_with("zh") {
            Self::Zh
        } else {
            Self::En
        }
    }
}

impl fmt::Display for UiLang {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl MotionLevel {
    /// Cycles motion intensity (`Full -> Reduced -> None -> Full`).
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Full => Self::Reduced,
            Self::Reduced => Self::None,
            Self::None => Self::Full,
        }
    }

    /// Returns the canonical kebab-case label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Reduced => "reduced",
            Self::None => "none",
        }
    }
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            viewport: ViewportMode::Fullscreen,
            theme: ThemeId::Auto,
            shader_preset: ShaderPreset::CyberOverdrive,
            motion: MotionLevel::Full,
            lang: UiLang::En,
            glyphs: GlyphSet::NerdFont,
            color_profile: ColorProfile::TrueColor,
            active_fps: DEFAULT_ACTIVE_FPS,
            show_splash: true,
            mouse_enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use core::assert_matches;

    use super::*;

    #[test]
    fn default_tui_config_targets_ultra_cyberpunk_fullscreen() {
        let config = TuiConfig::default();
        assert_matches!(config.viewport, ViewportMode::Fullscreen);
        assert_matches!(config.theme, ThemeId::Auto);
        assert_matches!(config.shader_preset, ShaderPreset::CyberOverdrive);
        assert_matches!(config.motion, MotionLevel::Full);
        assert_matches!(config.lang, UiLang::En);
        assert_matches!(config.glyphs, GlyphSet::NerdFont);
        assert_matches!(config.color_profile, ColorProfile::TrueColor);
        assert_eq!(config.active_fps, DEFAULT_ACTIVE_FPS);
        assert!(config.show_splash);
        assert!(config.mouse_enabled);
    }

    #[test]
    fn cycles_presets_themes_motion_and_detects_locale_hint() {
        assert_eq!(
            ShaderPreset::CyberOverdrive.next(),
            ShaderPreset::AuroraSweep
        );
        assert_eq!(ShaderPreset::AuroraSweep.next(), ShaderPreset::CrispMinimal);
        assert_eq!(
            ShaderPreset::CrispMinimal.next(),
            ShaderPreset::CyberOverdrive
        );

        assert_eq!(ThemeId::CyberNeon.next(), ThemeId::TokyoNight);
        assert_eq!(ThemeId::CatppuccinLatte.next(), ThemeId::CyberNeon);

        assert_eq!(MotionLevel::Full.next(), MotionLevel::Reduced);
        assert_eq!(MotionLevel::Reduced.next(), MotionLevel::None);
        assert_eq!(MotionLevel::None.next(), MotionLevel::Full);

        assert_eq!(UiLang::from_locale_hint("zh_CN.UTF-8"), UiLang::Zh);
        assert_eq!(UiLang::from_locale_hint("en_US.UTF-8"), UiLang::En);
        assert_eq!(UiLang::En.toggle(), UiLang::Zh);
    }
}
