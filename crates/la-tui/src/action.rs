//! Typed unidirectional TEA actions (`Action`) and side-effect intents (`StudioCommand`)
//! decoupling `la-tui` cleanly from `la-runtime` and terminal backend I/O.

use core::time::Duration;

use la_core::manifest::GitOutputMode;
use serde::{Deserialize, Serialize};

use crate::config::{MotionLevel, ShaderPreset, ThemeId, UiLang};
use crate::editor::EditorAction;
use crate::form::{FormAction, ModalFormState};
use crate::media::DecodedRgbaImage;
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
    /// Cycle the primary enum property on a selected resource (Rule trigger, Subagent model).
    CycleItemProperty,
    /// Reorder the selected `AGENTS.md` instruction section up (`K`) or down (`J`).
    ReorderItem {
        /// Move toward index 0 when `true`, otherwise move down.
        up: bool,
    },
    /// Open the built-in native grapheme multiline editor on the selected Markdown/prompt body (`i`).
    StartInlineEdit,
    /// Persist the active inline editor buffer back to `.lazyagents/` and recompile (`Ctrl+S`).
    SaveInlineEdit,
    /// Discard/close the active inline editor session (`Esc`).
    CancelInlineEdit,
    /// Toggle optional Vim Normal/Insert mode inside the active inline editor (`Ctrl+V`).
    ToggleVimMode,
    /// Open external `$VISUAL`/`$EDITOR` on the selected resource's backing source file (`e`).
    OpenExternalEditor,
    /// Forward an editing keystroke into the active native grapheme editor.
    EditorInput(EditorAction),
    /// Open the shadowed Delete Confirmation modal for the selected resource (`d`).
    OpenDeleteConfirm,
    /// Toggle purging the backing `.lazyagents/` source file in the Delete Confirmation modal.
    ToggleDeletePurge,
    /// Confirm and execute the queued resource deletion.
    ConfirmDelete,
    /// Open the fuzzy-searchable offline Built-in Catalog & Preset Picker (`C`).
    OpenCatalogPicker,
    /// Append a character into the Built-in Catalog Picker filter query.
    CatalogInput(char),
    /// Delete the last character in the Built-in Catalog Picker filter query.
    CatalogBackspace,
    /// Cycle the category filter pill inside the Built-in Catalog Picker (`Tab`).
    CatalogCycleCategory,
    /// Install the highlighted entry from the Built-in Catalog Picker (`Enter`).
    CatalogConfirm,
    /// Open a structured shadowed modal form populated with `ModalFormState`.
    OpenForm(Box<ModalFormState>),
    /// Route a keyboard or paste action into the active shadowed modal form.
    FormInput(FormAction),
    /// Append a character into the `Ctrl+K` Command Palette filter query.
    PaletteInput(char),
    /// Delete the last character in the `Ctrl+K` Command Palette filter query.
    PaletteBackspace,
    /// Execute the highlighted command inside the `Ctrl+K` Command Palette.
    PaletteConfirm,
    /// Advance to the next step inside the Guided Init Wizard.
    WizardNext,
    /// Step back inside the Guided Init Wizard.
    WizardPrev,
    /// Toggle the highlighted option inside the Guided Init Wizard.
    WizardToggleOption,
    /// Confirm and execute workspace initialization from the Guided Init Wizard.
    WizardConfirm,
    /// Toggle focus on the Hook Policy Playground live input field.
    PlaygroundToggleFocus,
    /// Append a character into the Hook Policy Playground payload input.
    PlaygroundInput(char),
    /// Remove the last character from the Hook Policy Playground payload input.
    PlaygroundBackspace,
    /// Cycle to the next built-in hook guard inside the Hook Policy Playground.
    PlaygroundCycleGuard,
    /// Load the next curated test sample inside the Hook Policy Playground.
    PlaygroundNextSample,
    /// Evaluate the current Hook Policy Playground payload via `la-core::hook_eval`.
    PlaygroundEvaluate,
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
    /// Off-thread decoded plugin logo image ready for `ratatui-image` rendering.
    PluginLogoDecoded {
        /// Kebab-case plugin name.
        plugin_name: String,
        /// Pre-decoded RGBA8 pixels.
        image: DecodedRgbaImage,
    },
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
    /// Remove a `.lazyagents/` backing source file or directory when `--purge-files` is confirmed.
    DeleteSourceFile {
        /// `.lazyagents/`-relative POSIX path.
        rel_path: String,
    },
    /// Execute `apply_write_plan` against disk (with optional `--force`).
    ApplySync {
        /// Overwrite externally modified files when `true`.
        force: bool,
    },
    /// Bootstrap `.lazyagents/` from the Guided First-Run Init Wizard.
    InitializeWorkspace {
        /// Chosen Git output policy (`commit` or `ignore`).
        git_mode: GitOutputMode,
        /// Whether to ingest existing `AGENTS.md` / `.agents/**` files.
        import_existing: bool,
    },
    /// Decode a plugin logo file off-thread via `tokio::task::spawn_blocking`.
    LoadPluginLogo {
        /// Kebab-case plugin name.
        plugin_name: String,
        /// Relative path to logo image asset.
        rel_path: String,
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
