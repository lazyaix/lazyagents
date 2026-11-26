//! Asynchronous runtime, LLM provider clients, tool runners, and embedded PTY orchestration
//! for `lazyagents`.
//!
//! This crate depends strictly on [`la_core`] and must never depend on `la-tui`.

mod config;
mod handle;

pub use crate::config::{DEFAULT_EVENT_CHANNEL_CAPACITY, RuntimeConfig};
pub use crate::handle::RuntimeHandle;
