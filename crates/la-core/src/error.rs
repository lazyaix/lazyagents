//! Structured domain error definitions for `la-core`.

use thiserror::Error;

use crate::scope::ConfigScope;

/// Canonical structured error type for pure domain operations in `la-core`.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CoreError {
    /// A resource identifier was empty or contained invalid characters.
    #[error("invalid resource identifier `{value}`: {reason}")]
    InvalidResourceId {
        /// The rejected identifier value.
        value: String,
        /// Human-readable reason explaining why validation failed.
        reason: &'static str,
    },

    /// A kebab-case name (skill, subagent, or plugin) violated naming rules.
    #[error("invalid kebab-case name `{value}`: {reason}")]
    InvalidKebabName {
        /// The rejected name value.
        value: String,
        /// Human-readable reason explaining why validation failed.
        reason: &'static str,
    },

    /// A TOML manifest failed to parse into the canonical schema.
    #[error("failed to parse manifest TOML: {message}")]
    ManifestParse {
        /// Sanitized parse error description without leaking upstream error types.
        message: String,
    },

    /// A manifest failed to serialize back to TOML.
    #[error("failed to serialize manifest TOML: {message}")]
    ManifestSerialize {
        /// Sanitized serialization error description.
        message: String,
    },

    /// The manifest declared an unsupported `schema_version`.
    #[error("unsupported manifest schema_version {found}; expected {expected}")]
    UnsupportedSchemaVersion {
        /// Version declared by the input manifest.
        found: u32,
        /// Version supported by this build of `lazyagents`.
        expected: u32,
    },

    /// A manifest contained a semantic invariant violation.
    #[error("invalid manifest configuration: {message}")]
    InvalidManifest {
        /// Human-readable explanation of the violation.
        message: String,
    },

    /// A lower-precedence scope attempted to weaken a setting locked by an upper scope.
    #[error("scope `{attempted}` cannot override `{setting}` locked by `{locked_by}`")]
    LockedPolicyViolation {
        /// Setting path or policy identifier that is locked.
        setting: String,
        /// Scope that enforced the lock.
        locked_by: ConfigScope,
        /// Lower scope that attempted to relax or override the locked setting.
        attempted: ConfigScope,
    },

    /// Scope resolution received layers without a required project scope or in invalid state.
    #[error("scope resolution failed: {message}")]
    ScopeResolution {
        /// Human-readable description of the scope resolution failure.
        message: String,
    },
}
