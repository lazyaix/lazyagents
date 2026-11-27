//! Canonical structured error types for `la-tui`.

use thiserror::Error;

/// Structured errors returned by `la-tui` rendering, highlighting, or state operations.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum TuiError {
    /// Syntax highlighting failed for the requested source payload.
    #[error("syntax highlight error: {message}")]
    Highlight {
        /// Human-readable diagnostic summary.
        message: String,
    },
    /// Invalid terminal protocol payload (e.g., malformed OSC control sequence parameters).
    #[error("invalid terminal protocol payload: {message}")]
    Protocol {
        /// Human-readable diagnostic summary.
        message: String,
    },
    /// Domain error bubbled up from `la-core`.
    #[error("core domain error: {message}")]
    Core {
        /// Underlying domain error description.
        message: String,
    },
}

impl From<la_core::CoreError> for TuiError {
    fn from(value: la_core::CoreError) -> Self {
        Self::Core {
            message: value.to_string(),
        }
    }
}
