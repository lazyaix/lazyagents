//! Agent identity types enforcing non-blank domain identifiers at construction boundaries.

use core::fmt;

/// Strongly typed identifier for an agent instance within a session.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AgentId(String);

impl AgentId {
    /// Creates a new [`AgentId`] if the provided identifier is non-empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use la_core::AgentId;
    ///
    /// let id = AgentId::new("planner-1");
    /// assert!(id.is_some());
    /// assert!(AgentId::new("   ").is_none());
    /// ```
    #[must_use]
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        if value.trim().is_empty() {
            None
        } else {
            Some(Self(value))
        }
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use core::assert_matches;

    use super::*;

    #[test]
    fn agent_id_rejects_blank_strings() {
        assert_matches!(AgentId::new(""), None);
        assert_matches!(AgentId::new("   "), None);
        let id = AgentId::new("coder-1").unwrap();
        assert_eq!(id.as_str(), "coder-1");
        assert_eq!(id.to_string(), "coder-1");
    }
}
