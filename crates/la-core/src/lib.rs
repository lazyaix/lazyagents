//! Pure Sans-I/O canonical configuration IR, target compilers, compliance/budget linters,
//! hook policy evaluator, and secret-redaction primitives for `lazyagents`.
//!
//! This crate intentionally performs zero network, filesystem, PTY, or terminal I/O so
//! both `la-runtime` and `la-tui` can share deterministic domain logic.

mod agent;
pub mod engineering;
mod error;
mod id;
pub mod manifest;
pub mod scope;
mod secret;
mod session;

pub use crate::agent::AgentId;
pub use crate::engineering::EngineeringPolicy;
pub use crate::error::CoreError;
pub use crate::id::{HookEvent, KebabName, PolicyDecision, ResourceId, TargetKind, ToolCapability};
pub use crate::manifest::{CURRENT_SCHEMA_VERSION, ProjectManifest};
pub use crate::scope::{ConfigScope, MergePolicy, ResolvedConfig, ScopeResolver, ScopedLayer};
pub use crate::secret::{
    RedactedSecret, SecretFinding, SecretKind, redact_ghs_tokens, scan_secrets,
};
pub use crate::session::SessionStatus;
