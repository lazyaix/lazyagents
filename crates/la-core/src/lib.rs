//! Pure Sans-I/O canonical configuration IR, target compilers, compliance/budget linters,
//! hook policy evaluator, and secret-redaction primitives for `lazyagents`.
//!
//! This crate intentionally performs zero network, filesystem, PTY, or terminal I/O so
//! both `la-runtime` and `la-tui` can share deterministic domain logic.

mod agent;
mod secret;
mod session;

pub use crate::agent::AgentId;
pub use crate::secret::{RedactedSecret, redact_ghs_tokens};
pub use crate::session::SessionStatus;
