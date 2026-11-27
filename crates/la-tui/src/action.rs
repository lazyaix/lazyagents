//! Typed unidirectional TEA actions (`Action`) and side-effect intents (`StudioCommand`)
//! decoupling `la-tui` cleanly from `la-runtime` and terminal backend I/O.

use core::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::{MotionLevel, ShaderPreset, ThemeId, UiLang};
use crate::editor::EditorAction;
use crate::state::{
    FocusPane, GitHubAuditSnapshot, InspectorTab, LogEntry, ModalKind, NavSection, ToastLevel,
    WorkspaceSnapshot,
};

/// User-persisted studio visual preferences written to platform config (`studio.toml`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StudioPreferences {
    /// Active built-in theme identifier.
    pub theme: ThemeId,
    /// Active `TachyonFX` shader intensity preset.
    pub shader_preset: ShaderPreset,
    /// Visual motion tier.
    pub motion: MotionLevel,
    /// Active UI language (`en` or `zh`).
    pub lang: UiLang,
}

/// High-level state transitions dispatched into [`crate::state::AppState::update`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    /// Dismiss the animated startup splash immediately.
    SkipSplash,
    /// Select a specific Command Deck navigation section.
    SelectNav(NavSection),
    /// Move down to the next navigation item in the left rail.
    NextNav,
    /// Move up to the previous navigation item in the left rail.
    PrevNav,
    /// Move down inside the active pane's list, table, or form.
    MoveDown,
    /// Move up inside the active pane's list, table, or form.
    MoveUp,
    /// Move left (switch sub-tabs or focus left pane).
    MoveLeft,
    /// Move right (switch sub-tabs or focus right pane).
    MoveRight,
    /// Jump to top of current list/preview.
    JumpTop,
    /// Jump to bottom of current list/preview.
    JumpBottom,
    /// Cycle focus across visible Command Deck panes (`Tab`).
    CycleFocusPane,
    /// Focus a specific pane directly.
    FocusPane(FocusPane),
    /// Toggle left navigation rail collapse (`[`).
    ToggleRail,
    /// Toggle right live inspector pod (`]`).
    ToggleInspector,
    /// Switch active tab inside the right live inspector pod.
    SelectInspectorTab(InspectorTab),
    /// Toggle side-by-side vs unified diff mode inside Diff inspector/view.
    ToggleDiffMode,
    /// Toggle bottom live telemetry & redacted logs drawer (`~`).
    ToggleLogsDrawer,
    /// Scroll the active preview/diff/logs pane vertically.
    ScrollLines(i16),
    /// Toggle boolean or cycle enum on the currently selected setting/resource (`Space` / `Enter`).
    ActivateSelected,
    /// Open external `$VISUAL`/`$EDITOR` on the selected resource's backing source file (`e`).
    OpenExternalEditor,
    /// Forward an editing keystroke into the active native grapheme editor.
    EditorInput(EditorAction),
    /// Request deterministic sync/generate (`s`, or force overwrite from Conflict Guard).
    RequestSync {
        /// Overwrite externally modified managed files when `true`.
        force: bool,
    },
    /// Trigger an asynchronous read-only `gh` CLI audit (`r` on GitHub Audit view).
    TriggerGitHubAudit,
    /// Copy a text snippet (e.g., `gh api` remediation command) via OSC 52 (`y`).
    CopySelected,
    /// Request reloading the workspace from disk (`R`).
    ReloadWorkspace,
    /// Cycle built-in theme (`t`).
    CycleTheme,
    /// Cycle `TachyonFX` shader preset (`p`).
    CycleShaderPreset,
    /// Cycle motion accessibility tier (`m`).
    CycleMotion,
    /// Toggle interface language between `en` and `zh` (`L`).
    ToggleLang,
    /// Open a floating modal (`Ctrl+K` Command Palette, `?` Help, Conflict Guard, etc.).
    OpenModal(ModalKind),
    /// Close the topmost floating modal (`Esc`).
    CloseModal,
    /// Mouse left-click at terminal cell coordinates `(col, row)`.
    MouseClick {
        /// 0-based terminal column.
        col: u16,
        /// 0-based terminal row.
        row: u16,
    },
    /// Mouse wheel scroll at terminal cell coordinates `(col, row)`.
    MouseScroll {
        /// 0-based terminal column.
        col: u16,
        /// 0-based terminal row.
        row: u16,
        /// `true` for scroll up, `false` for scroll down.
        up: bool,
    },
    /// Advance `TachyonFX` shader clock and splash timer by deterministic duration `dt`.
    Tick(Duration),
    /// Terminal window focus changed (throttles ambient animations when unfocused).
    WindowFocus(bool),
    /// Terminal resized to `(width, height)`.
    Resize(u16, u16),
    /// Async workspace reload completed by `la-cli` runner.
    WorkspaceLoaded(Box<WorkspaceSnapshot>),
    /// Async read-only GitHub audit completed.
    GitHubAuditLoaded(GitHubAuditSnapshot),
    /// Async read-only GitHub audit failed with a user-facing diagnostic.
    GitHubAuditFailed(String),
    /// Async sync/generate completed.
    SyncCompleted {
        /// Number of newly created files.
        created: usize,
        /// Number of updated files.
        updated: usize,
        /// Number of deleted stale files.
        deleted: usize,
    },
    /// Async sync/generate failed.
    SyncFailed(String),
    /// Incoming structured tracing log entry routed to the in-TUI redacted ring buffer.
    LogReceived(LogEntry),
    /// Display a transient toast banner.
    Notify {
        /// Severity styling tier.
        level: ToastLevel,
        /// Concise user-facing message.
        message: String,
    },
    /// Exit the Config Studio cleanly (`q` / `Ctrl+C`).
    Quit,
}

/// Pure side-effect commands emitted by [`crate::state::AppState::update`] for `la-cli` to run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StudioCommand {
    /// Atomically persist `.lazyagents/lazyagents.toml` via `la-runtime`.
    SaveManifest(Box<la_core::ProjectManifest>),
    /// Atomically persist a `.lazyagents/` source file (section, rule, skill, agent).
    SaveSourceFile {
        /// `.lazyagents/`-relative POSIX path.
        rel_path: String,
        /// Normalized UTF-8 text body.
        content: String,
    },
    /// Execute `apply_write_plan` against disk (with optional `--force`).
    ApplySync {
        /// Overwrite externally modified files when `true`.
        force: bool,
    },
    /// Spawn read-only `gh` CLI audit off the render thread.
    RunGitHubAudit,
    /// Restore terminal via `DropGuard`, launch `$VISUAL`/`$EDITOR`, then re-enter & reload.
    OpenExternalEditor {
        /// Workspace-relative file path to open.
        rel_path: String,
    },
    /// Emit an OSC 52 clipboard sequence (with `tmux` passthrough when applicable).
    CopyToClipboard(String),
    /// Persist updated UI preferences (`theme`, `shader_preset`, `motion`, `lang`).
    SavePreferences(StudioPreferences),
    /// Reload `.lazyagents/` and workspace files from disk off the render thread.
    ReloadWorkspace,
    /// Terminate the TUI event loop cleanly.
    Quit,
}
