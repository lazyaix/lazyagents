//! L4 Ultra Cyberpunk HUD Ratatui 0.30 + `TachyonFX` Config Studio for `lazyagents`.
//!
//! This crate depends strictly on [`la_core`] and must never depend on `la-runtime`,
//! ensuring fast compilation and deterministic headless visual tests.

pub mod action;
mod config;
pub mod editor;
pub mod effects;
mod error;
pub mod form;
pub mod highlight;
pub mod i18n;
pub mod media;
pub mod protocol;
pub mod state;
mod status;
pub mod text;
pub mod theme;
pub mod views;

pub use crate::action::{Action, StudioCommand, StudioPreferences};
pub use crate::config::{
    ColorProfile, DEFAULT_ACTIVE_FPS, DEFAULT_INLINE_HEIGHT, GlyphSet, MotionLevel, ShaderPreset,
    ThemeId, TuiConfig, UiLang, ViewportMode,
};
pub use crate::editor::{EditorAction, EditorMode, TextEditor};
pub use crate::effects::{EffectSlot, StudioEffects};
pub use crate::error::TuiError;
pub use crate::form::{
    FieldKind, FieldValidator, FormAction, FormField, FormOutcome, FormTab, KeyValueEntry,
    KeyValueListState, ModalFormState, StringListState, TextInputState,
};
pub use crate::highlight::{
    PreviewRenderer, PreviewSyntax, SideBySideRow, render_side_by_side_diff,
    render_unified_diff_styled,
};
pub use crate::i18n::I18n;
pub use crate::media::{DecodedRgbaImage, PluginLogoPreview};
pub use crate::protocol::{
    BackgroundTone, DEC_2026_BEGIN_SYNC, DEC_2026_END_SYNC, GraphicsProtocolKind, ProgressBarState,
    TerminalCapabilities, TerminalEnvHints, TransportTier,
};
pub use crate::state::{
    AppState, AuditCheckView, AuditStatusView, CatalogCategory, CatalogEntryKind, CatalogItem,
    CatalogPickerState, CommandPaletteState, ConfirmDeleteState, DeckVisibility, DeleteTarget,
    DiffStyle, FocusPane, FormTarget, GitHubAuditSnapshot, GovernanceDomain, HookPlaygroundState,
    HookVerdictView, InitWizardState, InlineEditState, InspectorTab, LogEntry, LogLevel, ModalKind,
    NavSection, ToastLevel, WorkspaceSnapshot,
};
pub use crate::status::{budget_status_color, policy_decision_badge, status_label};
pub use crate::theme::{GlyphPalette, ResolvedTheme, Theme};
pub use crate::views::render_studio;
