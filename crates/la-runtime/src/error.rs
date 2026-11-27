//! Canonical structured error type for filesystem, process, and audit I/O in `la-runtime`.

use la_core::CoreError;
use thiserror::Error;

/// Structured error type returned by all `la-runtime` operations.
///
/// Intentionally avoids leaking raw `std::io::Error` or third-party error types across the
/// public crate boundary (`M-DONT-LEAK-TYPES`).
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeError {
    /// Error propagated from pure domain validation, compilation, or lockfile logic in `la-core`.
    #[error(transparent)]
    Core(#[from] CoreError),

    /// Filesystem read, write, directory creation, or atomic persist failure.
    #[error("filesystem error at `{path}`: {message}")]
    Io {
        /// Workspace-relative or absolute path associated with the I/O failure.
        path: String,
        /// Sanitized OS error description.
        message: String,
    },

    /// Required `.lazyagents/lazyagents.toml` manifest was not found in the workspace.
    #[error("no `.lazyagents/lazyagents.toml` found under `{root}`; run `lazyagents init` first")]
    ManifestNotFound {
        /// Searched workspace root path.
        root: String,
    },

    /// Atomic generation refused to overwrite externally modified files without `--force`.
    #[error(
        "refusing to overwrite {count} externally modified file(s): {paths}; re-run with `--force` or run `lazyagents import`"
    )]
    ExternalConflict {
        /// Number of conflicting files.
        count: usize,
        /// Comma-separated list of conflicting workspace-relative paths.
        paths: String,
    },

    /// Local pack directory could not be resolved or filtered.
    #[error("pack `{pack_id}` error: {message}")]
    Pack {
        /// Pack identifier declared in `[[packs]]`.
        pack_id: String,
        /// Human-readable reason.
        message: String,
    },

    /// Read-only `gh` CLI audit could not execute or parse GitHub API responses.
    #[error("GitHub audit error: {message}")]
    GitHubAudit {
        /// Sanitized explanation (never echoes raw credentials or tokens).
        message: String,
    },

    /// External `$VISUAL`/`$EDITOR` invocation failed.
    #[error("editor error: {message}")]
    Editor {
        /// Human-readable failure reason.
        message: String,
    },
}

impl RuntimeError {
    pub(crate) fn io(path: impl Into<String>, err: &std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            message: err.to_string(),
        }
    }
}
