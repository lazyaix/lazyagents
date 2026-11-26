//! Presentation helpers mapping `la-core` session states into compact status badges.

use la_core::SessionStatus;

/// Returns a concise human-readable status badge label for a [`SessionStatus`].
#[must_use]
pub const fn status_label(status: SessionStatus) -> &'static str {
    match status {
        SessionStatus::Idle => "IDLE",
        SessionStatus::Running => "RUNNING",
        SessionStatus::AwaitingApproval => "APPROVAL",
        SessionStatus::Completed => "DONE",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_labels_cover_all_states() {
        assert_eq!(status_label(SessionStatus::Idle), "IDLE");
        assert_eq!(status_label(SessionStatus::Running), "RUNNING");
        assert_eq!(status_label(SessionStatus::AwaitingApproval), "APPROVAL");
        assert_eq!(status_label(SessionStatus::Completed), "DONE");
    }
}
