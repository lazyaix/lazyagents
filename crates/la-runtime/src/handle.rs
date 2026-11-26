//! Runtime lifecycle handle encapsulating active configuration and session state.

use la_core::SessionStatus;

use crate::config::RuntimeConfig;

/// Minimal runtime status snapshot shared with callers before full async orchestration lands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeHandle {
    config: RuntimeConfig,
    status: SessionStatus,
}

impl RuntimeHandle {
    /// Creates a new idle [`RuntimeHandle`] from the supplied configuration.
    #[must_use]
    pub const fn new(config: RuntimeConfig) -> Self {
        Self {
            config,
            status: SessionStatus::Idle,
        }
    }

    /// Returns the active configuration for this runtime handle.
    #[must_use]
    pub const fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    /// Returns the current lifecycle status.
    #[must_use]
    pub const fn status(&self) -> SessionStatus {
        self.status
    }
}

#[cfg(test)]
mod tests {
    use core::assert_matches;

    use crate::config::DEFAULT_EVENT_CHANNEL_CAPACITY;

    use super::*;

    #[test]
    fn runtime_handle_starts_idle_with_default_capacity() {
        let handle = RuntimeHandle::new(RuntimeConfig::default());
        assert_eq!(
            handle.config().event_channel_capacity,
            DEFAULT_EVENT_CHANNEL_CAPACITY
        );
        assert_matches!(handle.status(), SessionStatus::Idle);
    }
}
