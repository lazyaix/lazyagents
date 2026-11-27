//! Presentation helpers mapping `la-core` session, budget, and hook-policy states into
//! compact, theme-styled status labels.

use la_core::{BudgetStatus, PolicyDecision, SessionStatus};
use ratatui::style::Color;

use crate::theme::Theme;

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

/// Returns the semantic badge label and accent color for a [`PolicyDecision`].
#[must_use]
pub const fn policy_decision_badge(
    decision: PolicyDecision,
    theme: &Theme,
) -> (&'static str, Color) {
    match decision {
        PolicyDecision::Allow => ("ALLOW", theme.status_success),
        PolicyDecision::Ask => ("ASK", theme.status_approval),
        PolicyDecision::ForceAsk => ("FORCE_ASK", theme.status_warn),
        PolicyDecision::Deny => ("DENY", theme.status_error),
    }
}

/// Returns the semantic accent color for an Antigravity [`BudgetStatus`].
#[must_use]
pub const fn budget_status_color(status: BudgetStatus, theme: &Theme) -> Color {
    match status {
        BudgetStatus::Ok => theme.status_success,
        BudgetStatus::Warning => theme.status_warn,
        BudgetStatus::Exceeded => theme.status_error,
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{ColorProfile, GlyphSet};
    use crate::theme::ResolvedTheme;

    use super::*;

    #[test]
    fn status_and_policy_labels_cover_all_states() {
        assert_eq!(status_label(SessionStatus::Idle), "IDLE");
        assert_eq!(status_label(SessionStatus::Running), "RUNNING");
        assert_eq!(status_label(SessionStatus::AwaitingApproval), "APPROVAL");
        assert_eq!(status_label(SessionStatus::Completed), "DONE");

        let theme = Theme::build(
            ResolvedTheme::CyberNeon,
            ColorProfile::TrueColor,
            GlyphSet::NerdFont,
        );
        assert_eq!(
            policy_decision_badge(PolicyDecision::Deny, &theme),
            ("DENY", theme.status_error)
        );
        assert_eq!(
            budget_status_color(BudgetStatus::Warning, &theme),
            theme.status_warn
        );
    }
}
