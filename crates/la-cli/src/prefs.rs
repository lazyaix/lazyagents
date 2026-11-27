//! Platform-native user Studio preferences persistence (`studio.toml`) and
//! `CLI > Environment > studio.toml > Defaults` resolution.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use la_runtime::write_atomic;
use la_tui::{
    StudioPreferences, TerminalCapabilities, TerminalEnvHints, TuiConfig, UiLang, ViewportMode,
};

/// Resolves the platform user config file path for persisted Studio visual preferences:
/// - Unix/macOS: `$XDG_CONFIG_HOME/lazyagents/studio.toml` or `$HOME/.config/lazyagents/studio.toml`
/// - Windows: `%APPDATA%\lazyagents\studio.toml`
#[must_use]
pub(crate) fn preferences_path() -> Option<PathBuf> {
    cfg_select! {
        windows => {
            env::var_os("APPDATA")
                .map(PathBuf::from)
                .map(|dir| dir.join("lazyagents").join("studio.toml"))
        },
        _ => {
            if let Some(xdg) = env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
                return Some(PathBuf::from(xdg).join("lazyagents").join("studio.toml"));
            }
            env::var_os("HOME")
                .filter(|v| !v.is_empty())
                .map(|home| PathBuf::from(home).join(".config").join("lazyagents").join("studio.toml"))
        },
    }
}

/// Loads persisted [`StudioPreferences`] from `path`, returning `None` if absent or unreadable.
#[must_use]
pub(crate) fn load_preferences_from(path: &Path) -> Option<StudioPreferences> {
    let raw = fs::read_to_string(path).ok()?;
    toml::from_str(&raw).ok()
}

/// Atomically persists [`StudioPreferences`] to `path`.
pub(crate) fn save_preferences_to(path: &Path, prefs: StudioPreferences) -> Result<(), String> {
    let serialized = toml::to_string_pretty(&prefs)
        .map_err(|err| format!("failed to serialize prefs: {err}"))?;
    write_atomic(path, &serialized).map_err(|err| err.to_string())
}

/// Captures terminal capability environment hints from the current process environment.
#[must_use]
pub(crate) fn collect_terminal_env_hints() -> TerminalEnvHints {
    let locale = env::var("LC_ALL")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| env::var("LANG").ok().filter(|v| !v.is_empty()));
    let ssh = env::var("SSH_CONNECTION")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| env::var("SSH_TTY").ok().filter(|v| !v.is_empty()));

    TerminalEnvHints {
        term: env::var("TERM").ok(),
        colorterm: env::var("COLORTERM").ok(),
        no_color: env::var("NO_COLOR").ok(),
        ssh_connection: ssh,
        tmux: env::var("TMUX").ok(),
        term_program: env::var("TERM_PROGRAM").ok(),
        locale,
    }
}

/// Merges CLI explicit overrides, environment hints, and persisted preferences into a [`TuiConfig`].
#[must_use]
pub(crate) fn resolve_tui_config(
    cli: &crate::cli::Cli,
    caps: TerminalCapabilities,
    env_hints: &TerminalEnvHints,
    saved: Option<StudioPreferences>,
) -> TuiConfig {
    let mut cfg = TuiConfig {
        glyphs: caps.glyph_set,
        color_profile: caps.color_profile,
        ..TuiConfig::default()
    };

    if let Some(locale) = env_hints.locale.as_deref() {
        cfg.lang = UiLang::from_locale_hint(locale);
    }

    if let Some(prefs) = saved {
        cfg.theme = prefs.theme;
        cfg.shader_preset = prefs.shader_preset;
        cfg.motion = prefs.motion;
        cfg.lang = prefs.lang;
    }

    if let Some(theme) = cli.theme {
        cfg.theme = theme.into();
    }
    if let Some(shader) = cli.shader {
        cfg.shader_preset = shader.into();
    }
    if let Some(motion) = cli.motion {
        cfg.motion = motion.into();
    }
    if let Some(lang) = cli.lang {
        cfg.lang = lang.into();
    }
    if let Some(glyphs) = cli.glyphs {
        cfg.glyphs = glyphs.into();
    }
    if let Some(height) = cli.inline {
        cfg.viewport = ViewportMode::Inline { height };
    }
    if cli.no_splash {
        cfg.show_splash = false;
    }
    if cli.no_mouse {
        cfg.mouse_enabled = false;
    }

    cfg.motion = caps.effective_motion(cfg.motion);
    cfg
}

#[cfg(test)]
mod tests {
    use la_tui::{MotionLevel, ShaderPreset, ThemeId};

    use super::*;

    #[test]
    fn roundtrips_studio_preferences_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("studio.toml");
        let prefs = StudioPreferences {
            theme: ThemeId::TokyoNight,
            shader_preset: ShaderPreset::AuroraSweep,
            motion: MotionLevel::Reduced,
            lang: UiLang::Zh,
        };
        save_preferences_to(&path, prefs).unwrap();
        let loaded = load_preferences_from(&path).unwrap();
        assert_eq!(loaded, prefs);
    }
}
