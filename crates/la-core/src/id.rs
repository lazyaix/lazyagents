//! Strongly typed domain identifiers, target descriptors, capabilities, and policy decisions.

use core::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::CoreError;

/// Validated non-empty identifier for manifests, rules, hooks, MCP servers, and packs.
///
/// Permits ASCII alphanumerics, `-`, `_`, `.`, `:`, and `/` with no leading/trailing whitespace.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ResourceId(String);

/// Validated lowercase kebab-case identifier required by Antigravity skills, subagents,
/// and plugins (`[a-z0-9]` segments separated by single `-`).
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct KebabName(String);

/// Supported and reserved AI coding agent compilation targets.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    /// Google Antigravity (`AGENTS.md`, `.agents/**`). Active in V1.
    Antigravity,
    /// Anthropic Claude Code (`CLAUDE.md`, `.mcp.json`, `.claude/**`). Reserved.
    ClaudeCode,
    /// `OpenAI` Codex (`AGENTS.md`, `.agents/skills/`, `.codex/config.toml`). Reserved.
    Codex,
    /// GitHub Copilot (`.github/copilot-instructions.md`). Reserved.
    GitHubCopilot,
}

/// Vendor-neutral abstract tool capability mapped to target-specific tool names at compile time.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCapability {
    /// Reading repository files.
    FileRead,
    /// Creating or modifying repository files.
    FileWrite,
    /// Executing terminal or shell commands.
    Shell,
    /// Searching file names or contents in the workspace.
    Search,
    /// Performing external web searches or HTTP lookups.
    WebSearch,
    /// Controlling an automated browser tool.
    Browser,
    /// Delegating work to subagents.
    Subagent,
    /// Target-specific or custom tool identifier passed through directly.
    #[serde(untagged)]
    Custom(String),
}

/// Canonical agent lifecycle hook event modeled across supported targets.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    /// Runs once when a new conversation session starts.
    SessionStart,
    /// Runs before a tool step executes.
    PreToolUse,
    /// Runs after a tool step completes.
    PostToolUse,
    /// Runs before the model is invoked (Antigravity-native).
    PreInvocation,
    /// Runs after model tool calls complete (Antigravity-native).
    PostInvocation,
    /// Runs when the agent execution loop terminates.
    Stop,
}

/// Deterministic authorization decision returned by guardrail hooks and path policies.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDecision {
    /// Allow the operation automatically.
    Allow,
    /// Prompt the user for confirmation (respecting cached approvals).
    #[default]
    Ask,
    /// Always prompt the user for confirmation, ignoring cached approvals.
    ForceAsk,
    /// Block the operation immediately.
    Deny,
}

impl ResourceId {
    /// Parses and validates a [`ResourceId`].
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidResourceId`] if `value` is empty, padded with whitespace,
    /// or contains unsupported characters.
    pub fn parse(value: impl Into<String>) -> Result<Self, CoreError> {
        let value = value.into();
        validate_resource_id(&value).map_err(|reason| CoreError::InvalidResourceId {
            value: value.clone(),
            reason,
        })?;
        Ok(Self(value))
    }

    /// Returns the validated identifier string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ResourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ResourceId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(raw).map_err(serde::de::Error::custom)
    }
}

impl KebabName {
    /// Parses and validates a lowercase kebab-case [`KebabName`].
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidKebabName`] if `value` is empty, contains uppercase or
    /// non-alphanumeric characters other than `-`, or has leading/trailing/consecutive hyphens.
    ///
    /// # Examples
    ///
    /// ```
    /// use la_core::KebabName;
    ///
    /// let name = KebabName::parse("release-prep").expect("valid kebab-case name");
    /// assert_eq!(name.as_str(), "release-prep");
    /// assert!(KebabName::parse("Release_Prep").is_err());
    /// ```
    pub fn parse(value: impl Into<String>) -> Result<Self, CoreError> {
        let value = value.into();
        validate_kebab_name(&value).map_err(|reason| CoreError::InvalidKebabName {
            value: value.clone(),
            reason,
        })?;
        Ok(Self(value))
    }

    /// Returns the validated kebab-case name string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for KebabName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for KebabName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(raw).map_err(serde::de::Error::custom)
    }
}

impl TargetKind {
    /// Returns the canonical snake-case identifier for this target.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Antigravity => "antigravity",
            Self::ClaudeCode => "claude_code",
            Self::Codex => "codex",
            Self::GitHubCopilot => "github_copilot",
        }
    }

    /// Returns `true` when this target has an active V1 compiler implementation.
    #[must_use]
    pub const fn is_active_in_v1(self) -> bool {
        matches!(self, Self::Antigravity)
    }
}

impl fmt::Display for TargetKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl HookEvent {
    /// Returns `true` when this event requires a tool-level `matcher` group in Antigravity.
    #[must_use]
    pub const fn uses_tool_matcher(self) -> bool {
        matches!(self, Self::PreToolUse | Self::PostToolUse)
    }

    /// Returns the Antigravity `hooks.json` event key name.
    #[must_use]
    pub const fn antigravity_key(self) -> &'static str {
        match self {
            Self::SessionStart => "SessionStart",
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::PreInvocation => "PreInvocation",
            Self::PostInvocation => "PostInvocation",
            Self::Stop => "Stop",
        }
    }
}

impl PolicyDecision {
    /// Returns the stricter of two policy decisions (`Deny > ForceAsk > Ask > Allow`).
    #[must_use]
    pub const fn stricter(self, other: Self) -> Self {
        if self.strictness_rank() >= other.strictness_rank() {
            self
        } else {
            other
        }
    }

    /// Returns `true` if `self` is at least as strict as `baseline`.
    #[must_use]
    pub const fn is_at_least_as_strict_as(self, baseline: Self) -> bool {
        self.strictness_rank() >= baseline.strictness_rank()
    }

    /// Returns the Antigravity hook JSON wire value for this decision.
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Ask => "ask",
            Self::ForceAsk => "force_ask",
            Self::Deny => "deny",
        }
    }

    const fn strictness_rank(self) -> u8 {
        match self {
            Self::Allow => 0,
            Self::Ask => 1,
            Self::ForceAsk => 2,
            Self::Deny => 3,
        }
    }
}

fn validate_resource_id(value: &str) -> Result<(), &'static str> {
    if value.is_empty() {
        return Err("must not be empty");
    }
    if value.trim() != value {
        return Err("must not have leading or trailing whitespace");
    }
    if value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/')
    }) {
        Ok(())
    } else {
        Err("only ASCII alphanumerics and `-`, `_`, `.`, `:`, `/` are allowed")
    }
}

fn validate_kebab_name(value: &str) -> Result<(), &'static str> {
    if value.is_empty() {
        return Err("must not be empty");
    }
    let bytes = value.as_bytes();
    if bytes.first() == Some(&b'-') || bytes.last() == Some(&b'-') {
        return Err("must not start or end with `-`");
    }
    let mut prev_hyphen = false;
    for &byte in bytes {
        let is_valid_char = byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-';
        if !is_valid_char {
            return Err("must contain only lowercase ASCII letters, digits, and single `-`");
        }
        if byte == b'-' && prev_hyphen {
            return Err("must not contain consecutive `--`");
        }
        prev_hyphen = byte == b'-';
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use core::assert_matches;

    use super::*;

    #[test]
    fn resource_id_accepts_structured_ids_and_rejects_invalid() {
        let id = ResourceId::parse("builtin:rust/toolchain-1.0").unwrap();
        assert_eq!(id.as_str(), "builtin:rust/toolchain-1.0");
        assert_eq!(id.to_string(), "builtin:rust/toolchain-1.0");

        assert_matches!(
            ResourceId::parse(""),
            Err(CoreError::InvalidResourceId { .. })
        );
        assert_matches!(
            ResourceId::parse(" spaced "),
            Err(CoreError::InvalidResourceId { .. })
        );
        assert_matches!(
            ResourceId::parse("bad id"),
            Err(CoreError::InvalidResourceId { .. })
        );
    }

    #[test]
    fn kebab_name_enforces_strict_lowercase_kebab_case() {
        let ok = KebabName::parse("ci-remediation-2").unwrap();
        assert_eq!(ok.as_str(), "ci-remediation-2");

        for bad in [
            "",
            "-lead",
            "trail-",
            "double--dash",
            "Upper",
            "under_score",
        ] {
            assert_matches!(
                KebabName::parse(bad),
                Err(CoreError::InvalidKebabName { .. }),
                "expected `{bad}` to be rejected"
            );
        }
    }

    #[test]
    fn policy_decision_strictness_orders_correctly() {
        assert_eq!(
            PolicyDecision::Ask.stricter(PolicyDecision::Deny),
            PolicyDecision::Deny
        );
        assert_eq!(
            PolicyDecision::ForceAsk.stricter(PolicyDecision::Allow),
            PolicyDecision::ForceAsk
        );
        assert!(PolicyDecision::Deny.is_at_least_as_strict_as(PolicyDecision::ForceAsk));
        assert!(!PolicyDecision::Ask.is_at_least_as_strict_as(PolicyDecision::ForceAsk));
    }

    #[test]
    fn target_kind_and_hook_event_metadata_match_spec() {
        assert!(TargetKind::Antigravity.is_active_in_v1());
        assert!(!TargetKind::ClaudeCode.is_active_in_v1());
        assert!(!TargetKind::Codex.is_active_in_v1());
        assert!(!TargetKind::GitHubCopilot.is_active_in_v1());

        assert!(HookEvent::PreToolUse.uses_tool_matcher());
        assert!(HookEvent::PostToolUse.uses_tool_matcher());
        assert!(!HookEvent::Stop.uses_tool_matcher());
        assert_eq!(HookEvent::PreInvocation.antigravity_key(), "PreInvocation");
    }
}
