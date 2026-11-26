//! Agent execution session lifecycle states.

/// High-level lifecycle status of an agent execution turn.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SessionStatus {
    /// Waiting for user input or commands.
    #[default]
    Idle,
    /// Actively reasoning or streaming output.
    Running,
    /// Paused pending explicit user approval for a sensitive action.
    AwaitingApproval,
    /// Completed successfully.
    Completed,
}

#[cfg(test)]
mod tests {
    use core::assert_matches;

    use super::*;

    #[test]
    fn default_session_status_is_idle() {
        assert_matches!(SessionStatus::default(), SessionStatus::Idle);
    }
}
