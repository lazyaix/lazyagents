//! Runtime bootstrap configuration and bounded-channel defaults.

use la_core::AgentId;

/// Default upper bound for bounded event channels between runtime workers and consumers.
pub const DEFAULT_EVENT_CHANNEL_CAPACITY: usize = 256;

/// Configuration for bootstrapping a `lazyagents` runtime instance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeConfig {
    /// Bounded channel capacity for runtime-to-consumer event streams.
    pub event_channel_capacity: usize,
    /// Optional identifier for the primary orchestrator agent.
    pub root_agent_id: Option<AgentId>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            event_channel_capacity: DEFAULT_EVENT_CHANNEL_CAPACITY,
            root_agent_id: None,
        }
    }
}
