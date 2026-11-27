//! Triple-safety terminal lifecycle (`DropGuard`, panic hook, signal teardown),
//! DEC 2026 synchronized output flushing, Kitty Keyboard Protocol negotiation,
//! OSC 11 background luminance sensing, and `$VISUAL`/`$EDITOR` suspend/resume handoff.

use core::mem::DropGuard;
use core::time::Duration;
use std::io::{self, Stdout, Write as _};
use std::path::Path;

use crossterm::event::{
    DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
    EnableFocusChange, EnableMouseCapture, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    BeginSynchronizedUpdate, EndSynchronizedUpdate, EnterAlternateScreen, LeaveAlternateScreen,
    disable_raw_mode, enable_raw_mode, supports_keyboard_enhancement,
};
use la_tui::protocol::{
    ProgressBarState, format_desktop_notification, format_osc9_4_progress, format_osc52_clipboard,
};
use la_tui::{BackgroundTone, TerminalCapabilities, TerminalEnvHints, TuiConfig, ViewportMode};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::{Frame, Terminal, TerminalOptions, Viewport};
use terminal_colorsaurus::{QueryOptions, ThemeMode, theme_mode};

/// Probes OSC 11 terminal background tone (with tight 80 ms timeout) and terminal capabilities
/// BEFORE entering raw mode so non-responding terminals never stall or leak escape codes.
#[must_use]
pub(crate) fn probe_terminal_capabilities(env_hints: &TerminalEnvHints) -> TerminalCapabilities {
    let mut caps = TerminalCapabilities::from_env_hints(env_hints);
    caps.kitty_keyboard = supports_keyboard_enhancement().unwrap_or(false);

    if caps.synchronized_output {
        let mut opts = QueryOptions::default();
        opts.timeout = Duration::from_millis(80);
        if matches!(theme_mode(opts), Ok(ThemeMode::Light)) {
            caps.background = BackgroundTone::Light;
        }
    }
    caps
}

/// Active terminal session guarding raw mode, protocols, and viewport teardown via Rust 1.100 [`DropGuard`].
pub(crate) struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    modes: RestoreState,
    sync_output: bool,
    in_tmux: bool,
    _restore_guard: DropGuard<RestoreState, fn(RestoreState)>,
}

#[derive(Clone, Copy)]
struct RestoreState {
    fullscreen: bool,
    kitty_active: bool,
    mouse_active: bool,
}

impl TerminalSession {
    /// Enters raw mode, negotiates modern protocols, and initializes the Ratatui `0.30` terminal.
    pub(crate) fn enter(config: &TuiConfig, caps: TerminalCapabilities) -> io::Result<Self> {
        let modes = RestoreState {
            fullscreen: matches!(config.viewport, ViewportMode::Fullscreen),
            kitty_active: caps.kitty_keyboard,
            mouse_active: config.mouse_enabled,
        };

        activate_terminal_modes(modes)?;
        let restore_guard = DropGuard::new(modes, restore_terminal_modes as fn(RestoreState));

        let backend = CrosstermBackend::new(io::stdout());
        let viewport = match config.viewport {
            ViewportMode::Fullscreen => Viewport::Fullscreen,
            ViewportMode::Inline { height } => Viewport::Inline(height),
        };
        let terminal = Terminal::with_options(backend, TerminalOptions { viewport })?;

        Ok(Self {
            terminal,
            modes,
            sync_output: caps.synchronized_output,
            in_tmux: caps.in_tmux,
            _restore_guard: restore_guard,
        })
    }

    /// Returns the current terminal bounding rectangle.
    pub(crate) fn size(&self) -> io::Result<Rect> {
        self.terminal
            .size()
            .map(|sz| Rect::new(0, 0, sz.width, sz.height))
    }

    /// Draws one frame wrapped in DEC Private Mode 2026 Synchronized Output (`BSU`/`ESU`).
    pub(crate) fn draw_synchronized<F>(&mut self, render_fn: F) -> io::Result<()>
    where
        F: FnOnce(&mut Frame<'_>),
    {
        if self.sync_output {
            let _ = execute!(self.terminal.backend_mut(), BeginSynchronizedUpdate);
        }
        let result = self.terminal.draw(render_fn).map(|_| ());
        if self.sync_output {
            let _ = execute!(self.terminal.backend_mut(), EndSynchronizedUpdate);
        }
        result
    }

    /// Emits an OSC 52 clipboard sequence (with automatic `tmux` DCS passthrough when inside `tmux`).
    pub(crate) fn copy_osc52(&mut self, text: &str) {
        if let Ok(seq) = format_osc52_clipboard(text, self.in_tmux) {
            let _ = self.terminal.backend_mut().write_all(seq.as_bytes());
            let _ = self.terminal.backend_mut().flush();
        }
    }

    /// Emits an OSC 9;4 native taskbar/tab progress bar sequence.
    pub(crate) fn set_progress(&mut self, state: ProgressBarState) {
        let seq = format_osc9_4_progress(state);
        let _ = self.terminal.backend_mut().write_all(seq.as_bytes());
        let _ = self.terminal.backend_mut().flush();
    }

    /// Emits an OSC 777 desktop notification sequence.
    pub(crate) fn notify_desktop(&mut self, title: &str, body: &str) {
        let seq = format_desktop_notification(Some(title), body);
        let _ = self.terminal.backend_mut().write_all(seq.as_bytes());
        let _ = self.terminal.backend_mut().flush();
    }

    /// Cleanly suspends TUI modes, spawns `$VISUAL`/`$EDITOR` on `path`, then resumes & clears screen.
    pub(crate) fn run_external_editor(&mut self, path: &Path) -> Result<(), String> {
        restore_terminal_modes(self.modes);

        let editor_res = la_runtime::open_in_editor(path).map_err(|err| err.to_string());

        let _ = activate_terminal_modes(self.modes);
        let _ = self.terminal.clear();
        editor_res
    }
}

/// Restores terminal state unconditionally on emergency panic (chained with `color-eyre`).
pub(crate) fn emergency_restore_terminal() {
    restore_terminal_modes(RestoreState {
        fullscreen: true,
        kitty_active: true,
        mouse_active: true,
    });
}

fn activate_terminal_modes(state: RestoreState) -> io::Result<()> {
    enable_raw_mode()?;
    let mut out = io::stdout();
    if state.fullscreen {
        execute!(out, EnterAlternateScreen)?;
    }
    execute!(out, EnableBracketedPaste, EnableFocusChange)?;
    if state.mouse_active {
        execute!(out, EnableMouseCapture)?;
    }
    if state.kitty_active {
        let _ = execute!(
            out,
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        );
    }
    Ok(())
}

fn restore_terminal_modes(state: RestoreState) {
    let mut out = io::stdout();
    let clear_progress = format_osc9_4_progress(ProgressBarState::Hidden);
    let _ = out.write_all(clear_progress.as_bytes());
    if state.kitty_active {
        let _ = execute!(out, PopKeyboardEnhancementFlags);
    }
    if state.mouse_active {
        let _ = execute!(out, DisableMouseCapture);
    }
    let _ = execute!(out, DisableBracketedPaste, DisableFocusChange);
    if state.fullscreen {
        let _ = execute!(out, LeaveAlternateScreen);
    }
    let _ = out.flush();
    let _ = disable_raw_mode();
}
