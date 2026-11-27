//! Pure Sans-I/O modern terminal protocol builders and capability descriptors.
//!
//! Covers DEC Private Mode 2026 (Synchronized Output), OSC 8 (Hyperlinks), OSC 52 (Clipboard
//! with optional `tmux` DCS passthrough), OSC 9;4 (Taskbar Progress), OSC 9 / 777 (Desktop
//! Notifications), and environment capability inference (SSH, `NO_COLOR`, `TERM=dumb`).

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;

use crate::config::{ColorProfile, GlyphSet, MotionLevel};
use crate::error::TuiError;

/// DEC Private Mode 2026 Begin Synchronized Update (`BSU`) escape sequence.
pub const DEC_2026_BEGIN_SYNC: &str = "\x1b[?2026h";

/// DEC Private Mode 2026 End Synchronized Update (`ESU`) escape sequence.
pub const DEC_2026_END_SYNC: &str = "\x1b[?2026l";

/// Maximum raw byte payload encoded into an OSC 52 clipboard sequence (74 KiB safe terminal cap).
pub const MAX_OSC52_PAYLOAD_BYTES: usize = 75_776;

/// Native taskbar/tab progress bar state for OSC 9;4 (Ghostty, Windows Terminal, `ConEmu`).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProgressBarState {
    /// Hide/clear progress bar (`0`).
    Hidden,
    /// Normal determinate progress `0..=100` (`1`).
    Normal(u8),
    /// Error progress `0..=100` (`2`).
    Error(u8),
    /// Indeterminate pulsing progress (`3`).
    Indeterminate,
    /// Paused progress `0..=100` (`4`).
    Paused(u8),
}

/// Terminal background luminance tone detected via OSC 11.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum BackgroundTone {
    /// Dark terminal background.
    #[default]
    Dark,
    /// Light terminal background.
    Light,
}

/// Terminal transport latency / capability tier.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum TransportTier {
    /// Local low-latency PTY session.
    #[default]
    Local,
    /// Remote SSH or `TERM=dumb` constrained session.
    Constrained,
}

/// Terminal graphics protocol tier detected for `ratatui-image`.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum GraphicsProtocolKind {
    /// Kitty Graphics Protocol.
    Kitty,
    /// iTerm2 / `WezTerm` inline image protocol.
    Iterm2,
    /// Sixel bitmap graphics.
    Sixel,
    /// Universal Unicode halfblock (`▀▄`) fallback.
    #[default]
    Halfblocks,
}

/// Detected terminal environment capabilities used to drive graceful L4 degradation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalCapabilities {
    /// Effective terminal color profile (`TrueColor` -> `Ansi256` -> `Ansi16` -> `NoColor`).
    pub color_profile: ColorProfile,
    /// Preferred icon/border glyph tier.
    pub glyph_set: GlyphSet,
    /// Detected terminal background luminance tone.
    pub background: BackgroundTone,
    /// Local vs constrained (SSH / dumb) transport tier.
    pub transport: TransportTier,
    /// Whether the session is running inside `tmux` (enables DCS passthrough for OSC 52).
    pub in_tmux: bool,
    /// Whether Kitty Keyboard Protocol (CSI-u) enhancement is supported.
    pub kitty_keyboard: bool,
    /// Whether DEC 2026 synchronized output frames should be emitted.
    pub synchronized_output: bool,
    /// Detected graphics protocol tier.
    pub graphics: GraphicsProtocolKind,
}

/// Read-only snapshot of terminal environment variables for pure Sans-I/O capability inference.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TerminalEnvHints {
    /// Value of `TERM`.
    pub term: Option<String>,
    /// Value of `COLORTERM`.
    pub colorterm: Option<String>,
    /// Value of `NO_COLOR` (present and non-empty disables color unconditionally).
    pub no_color: Option<String>,
    /// Value of `SSH_CONNECTION` or `SSH_TTY`.
    pub ssh_connection: Option<String>,
    /// Value of `TMUX`.
    pub tmux: Option<String>,
    /// Value of `TERM_PROGRAM`.
    pub term_program: Option<String>,
    /// Value of `LC_ALL` / `LANG`.
    pub locale: Option<String>,
}

impl Default for TerminalCapabilities {
    fn default() -> Self {
        Self {
            color_profile: ColorProfile::TrueColor,
            glyph_set: GlyphSet::NerdFont,
            background: BackgroundTone::Dark,
            transport: TransportTier::Local,
            in_tmux: false,
            kitty_keyboard: false,
            synchronized_output: true,
            graphics: GraphicsProtocolKind::Halfblocks,
        }
    }
}

impl TerminalCapabilities {
    /// Infers baseline terminal capabilities deterministically from environment variables.
    #[must_use]
    pub fn from_env_hints(hints: &TerminalEnvHints) -> Self {
        let term = hints
            .term
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let colorterm = hints
            .colorterm
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let term_program = hints
            .term_program
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();

        let no_color = hints.no_color.as_deref().is_some_and(|v| !v.is_empty());
        let is_dumb = term == "dumb";
        let in_ssh = hints
            .ssh_connection
            .as_deref()
            .is_some_and(|v| !v.trim().is_empty());
        let in_tmux = hints.tmux.as_deref().is_some_and(|v| !v.trim().is_empty());

        let color_profile = if no_color || is_dumb {
            ColorProfile::NoColor
        } else if colorterm.contains("truecolor")
            || colorterm.contains("24bit")
            || matches!(
                term_program.as_str(),
                "ghostty" | "wezterm" | "iterm.app" | "kitty" | "vscode"
            )
        {
            ColorProfile::TrueColor
        } else if term.contains("256color") {
            ColorProfile::Ansi256
        } else {
            ColorProfile::Ansi16
        };

        let unicode_ok = hints.locale.as_deref().is_none_or(|loc| {
            let lower = loc.to_ascii_lowercase();
            lower.contains("utf-8") || lower.contains("utf8") || lower.is_empty()
        });
        let glyph_set = if is_dumb || !unicode_ok {
            GlyphSet::Ascii
        } else if matches!(
            term_program.as_str(),
            "ghostty" | "wezterm" | "iterm.app" | "kitty" | "alacritty" | "vscode"
        ) {
            GlyphSet::NerdFont
        } else {
            GlyphSet::Unicode
        };

        let graphics = if term.contains("kitty") || term_program == "ghostty" {
            GraphicsProtocolKind::Kitty
        } else if term_program == "iterm.app" || term_program == "wezterm" {
            GraphicsProtocolKind::Iterm2
        } else {
            GraphicsProtocolKind::Halfblocks
        };

        let transport = if in_ssh || is_dumb {
            TransportTier::Constrained
        } else {
            TransportTier::Local
        };

        Self {
            color_profile,
            glyph_set,
            background: BackgroundTone::Dark,
            transport,
            in_tmux,
            kitty_keyboard: false,
            synchronized_output: !is_dumb,
            graphics,
        }
    }

    /// Computes the effective motion level after honoring `TERM=dumb` or SSH constraints.
    #[must_use]
    pub const fn effective_motion(&self, requested: MotionLevel) -> MotionLevel {
        if matches!(self.color_profile, ColorProfile::NoColor) && !self.synchronized_output {
            return MotionLevel::None;
        }
        if matches!(self.transport, TransportTier::Constrained)
            && matches!(requested, MotionLevel::Full)
        {
            return MotionLevel::Reduced;
        }
        requested
    }
}

/// Formats an OSC 8 terminal hyperlink (`\x1b]8;;URL\x1b\\LABEL\x1b]8;;\x1b\\`).
///
/// # Errors
///
/// Returns [`TuiError::Protocol`] if `url` is blank or contains ASCII control characters.
pub fn format_osc8_hyperlink(url: &str, label: &str) -> Result<String, TuiError> {
    if url.trim().is_empty() || url.chars().any(char::is_control) {
        return Err(TuiError::Protocol {
            message: "OSC 8 hyperlink URL must be non-empty and contain no control characters"
                .to_owned(),
        });
    }
    let clean_label = strip_control_chars(label);
    Ok(format!("\x1b]8;;{url}\x1b\\{clean_label}\x1b]8;;\x1b\\"))
}

/// Encodes `text` as an OSC 52 clipboard write sequence, optionally wrapped for `tmux` DCS passthrough.
///
/// # Errors
///
/// Returns [`TuiError::Protocol`] if `text` exceeds [`MAX_OSC52_PAYLOAD_BYTES`].
pub fn format_osc52_clipboard(text: &str, wrap_tmux_passthrough: bool) -> Result<String, TuiError> {
    if text.len() > MAX_OSC52_PAYLOAD_BYTES {
        return Err(TuiError::Protocol {
            message: format!(
                "OSC 52 payload ({} bytes) exceeds safe terminal cap ({MAX_OSC52_PAYLOAD_BYTES} bytes)",
                text.len()
            ),
        });
    }
    let encoded = BASE64_STANDARD.encode(text.as_bytes());
    if wrap_tmux_passthrough {
        Ok(format!("\x1bPtmux;\x1b\x1b]52;c;{encoded}\x07\x1b\\"))
    } else {
        Ok(format!("\x1b]52;c;{encoded}\x07"))
    }
}

/// Formats an OSC 9;4 taskbar/tab progress bar escape sequence.
#[must_use]
pub fn format_osc9_4_progress(state: ProgressBarState) -> String {
    match state {
        ProgressBarState::Hidden => "\x1b]9;4;0;0\x1b\\".to_owned(),
        ProgressBarState::Normal(pct) => format!("\x1b]9;4;1;{}\x1b\\", pct.min(100)),
        ProgressBarState::Error(pct) => format!("\x1b]9;4;2;{}\x1b\\", pct.min(100)),
        ProgressBarState::Indeterminate => "\x1b]9;4;3;0\x1b\\".to_owned(),
        ProgressBarState::Paused(pct) => format!("\x1b]9;4;4;{}\x1b\\", pct.min(100)),
    }
}

/// Formats a desktop notification sequence using OSC 777 (title + body) or OSC 9 (body only).
#[must_use]
pub fn format_desktop_notification(title: Option<&str>, body: &str) -> String {
    let safe_body = strip_control_chars(body);
    match title {
        Some(t) if !t.trim().is_empty() => {
            let safe_title = strip_control_chars(t).replace(';', " ");
            format!("\x1b]777;notify;{safe_title};{safe_body}\x07")
        }
        _ => format!("\x1b]9;{safe_body}\x07"),
    }
}

fn strip_control_chars(input: &str) -> String {
    input
        .chars()
        .filter(|ch| !ch.is_control() || *ch == ' ')
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_no_color_ssh_and_modern_terminal_capabilities() {
        let ghostty = TerminalCapabilities::from_env_hints(&TerminalEnvHints {
            term: Some("xterm-ghostty".to_owned()),
            colorterm: Some("truecolor".to_owned()),
            term_program: Some("ghostty".to_owned()),
            locale: Some("zh_CN.UTF-8".to_owned()),
            ..TerminalEnvHints::default()
        });
        assert_eq!(ghostty.color_profile, ColorProfile::TrueColor);
        assert_eq!(ghostty.glyph_set, GlyphSet::NerdFont);
        assert_eq!(ghostty.graphics, GraphicsProtocolKind::Kitty);
        assert_eq!(
            ghostty.effective_motion(MotionLevel::Full),
            MotionLevel::Full
        );

        let ssh_no_color = TerminalCapabilities::from_env_hints(&TerminalEnvHints {
            term: Some("xterm-256color".to_owned()),
            no_color: Some("1".to_owned()),
            ssh_connection: Some("1.2.3.4 22".to_owned()),
            ..TerminalEnvHints::default()
        });
        assert_eq!(ssh_no_color.color_profile, ColorProfile::NoColor);
        assert_eq!(
            ssh_no_color.effective_motion(MotionLevel::Full),
            MotionLevel::Reduced
        );
    }

    #[test]
    fn builds_valid_osc8_osc52_osc9_4_and_notification_sequences() {
        let link = format_osc8_hyperlink("https://github.com/lazyaix/lazyagents", "Repo").unwrap();
        assert!(link.starts_with("\x1b]8;;https://github.com/lazyaix/lazyagents\x1b\\"));
        assert!(link.ends_with("\x1b]8;;\x1b\\"));
        assert!(format_osc8_hyperlink("bad\x1burl", "x").is_err());

        let clip = format_osc52_clipboard("gh api repos/o/r", false).unwrap();
        assert!(clip.starts_with("\x1b]52;c;"));
        let tmux_clip = format_osc52_clipboard("hi", true).unwrap();
        assert!(tmux_clip.starts_with("\x1bPtmux;\x1b\x1b]52;c;"));

        assert_eq!(
            format_osc9_4_progress(ProgressBarState::Normal(150)),
            "\x1b]9;4;1;100\x1b\\"
        );
        assert_eq!(
            format_osc9_4_progress(ProgressBarState::Hidden),
            "\x1b]9;4;0;0\x1b\\"
        );

        let notif = format_desktop_notification(Some("LazyAgents"), "Sync complete");
        assert_eq!(notif, "\x1b]777;notify;LazyAgents;Sync complete\x07");
    }
}
