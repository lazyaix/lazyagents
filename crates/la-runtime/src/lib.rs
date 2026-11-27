//! Asynchronous filesystem and process runtime for `lazyagents` (workspace discovery,
//! repository introspection, atomic writers, read-only `gh` auditing, and editor handoff).
//!
//! This crate depends strictly on [`la_core`] and must never depend on `la-tui`.

mod config;
mod handle;

pub use crate::config::{DEFAULT_EVENT_CHANNEL_CAPACITY, RuntimeConfig};
pub use crate::handle::RuntimeHandle;
