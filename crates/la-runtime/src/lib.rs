//! Asynchronous filesystem and process runtime for `lazyagents` (workspace discovery,
//! repository introspection, atomic writers, read-only `gh` auditing, and editor handoff).
//!
//! This crate depends strictly on [`la_core`] and must never depend on `la-tui`.

mod config;
pub mod editor;
mod error;
pub mod gh_audit;
mod handle;
pub mod hook_exec;
pub mod importer;
pub mod pack_loader;
pub mod prober;
pub mod workspace;
pub mod writer;

pub use crate::config::{DEFAULT_EVENT_CHANNEL_CAPACITY, RuntimeConfig};
pub use crate::editor::{EditorCommand, open_in_editor, resolve_editor_command};
pub use crate::error::RuntimeError;
pub use crate::gh_audit::{
    AuditCheckStatus, GitHubAuditCheck, GitHubAuditReport, audit_github_repo, compare_github_state,
};
pub use crate::handle::RuntimeHandle;
pub use crate::hook_exec::{FormatExecOutcome, execute_format_plan};
pub use crate::importer::{
    ImportSource, ImportedWorkspace, import_workspace, write_imported_workspace,
};
pub use crate::pack_loader::load_local_packs;
pub use crate::prober::{probe_git_hook_context, probe_repository};
pub use crate::workspace::{
    LOCAL_MANIFEST_REL_PATH, LOCKFILE_REL_PATH, LoadedWorkspace, MANIFEST_REL_PATH,
    build_initial_manifest, discover_workspace_root, load_workspace, save_manifest,
};
pub use crate::writer::{ApplyOptions, ApplySummary, apply_write_plan, write_atomic};
