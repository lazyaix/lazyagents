//! Opaque secret wrappers and stateless GitHub installation-token redaction utilities.

use core::fmt;

/// Minimum suffix length after a GitHub `ghs_` installation-token prefix to treat as a secret.
const MIN_GHS_TOKEN_SUFFIX_LEN: usize = 36;

/// Variable-length opaque secret wrapper whose [`fmt::Debug`] and [`fmt::Display`]
/// implementations always emit a fixed redacted placeholder.
///
/// Supports arbitrary-length tokens, including ~520-character stateless GitHub App
/// installation tokens (`ghs_*`).
#[derive(Clone, Eq, PartialEq)]
pub struct RedactedSecret(String);

impl RedactedSecret {
    /// Wraps an opaque secret string without imposing any length limit.
    #[must_use]
    pub fn new(secret: impl Into<String>) -> Self {
        Self(secret.into())
    }

    /// Explicitly exposes the raw secret value at audited call sites.
    #[must_use]
    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    /// Returns `true` when the underlying secret string is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for RedactedSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RedactedSecret([REDACTED])")
    }
}

impl fmt::Display for RedactedSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

/// Redacts GitHub App installation tokens matching `ghs_[A-Za-z0-9._-]{36,}` from arbitrary
/// text such as logs, traces, or LLM context payloads.
///
/// This handles both legacy 40-character tokens and variable-length ~520-character stateless
/// JWT-shaped `ghs_*` tokens without decoding their payloads.
///
/// # Examples
///
/// ```
/// use la_core::redact_ghs_tokens;
///
/// let token = format!("ghs_{}.{}", "a".repeat(20), "b".repeat(20));
/// let line = format!("Authorization: Bearer {token} done");
/// assert_eq!(redact_ghs_tokens(&line), "Authorization: Bearer [REDACTED] done");
/// ```
#[must_use]
pub fn redact_ghs_tokens(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;

    while let Some(pos) = rest.find("ghs_") {
        let Some((before, after_prefix_start)) = rest.split_at_checked(pos) else {
            break;
        };
        output.push_str(before);

        let Some(suffix) = after_prefix_start.strip_prefix("ghs_") else {
            break;
        };
        let token_suffix_len = suffix
            .bytes()
            .take_while(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
            .count();

        if token_suffix_len >= MIN_GHS_TOKEN_SUFFIX_LEN {
            output.push_str("[REDACTED]");
            rest = suffix.get(token_suffix_len..).unwrap_or_default();
        } else {
            output.push_str("ghs_");
            rest = suffix;
        }
    }

    output.push_str(rest);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacted_secret_masks_debug_and_display() {
        let stateless = format!(
            "ghs_{}.{}.{}",
            "x".repeat(120),
            "y".repeat(250),
            "z".repeat(140)
        );
        let secret = RedactedSecret::new(stateless.clone());
        assert!(!secret.is_empty());
        assert_eq!(secret.expose_secret(), stateless);
        assert_eq!(format!("{secret:?}"), "RedactedSecret([REDACTED])");
        assert_eq!(format!("{secret}"), "[REDACTED]");
    }

    #[test]
    fn redact_ghs_tokens_scrubs_stateless_and_legacy_tokens() {
        let stateless = format!(
            "ghs_{}.{}.{}",
            "A".repeat(100),
            "B".repeat(260),
            "C".repeat(150)
        );
        let legacy = format!("ghs_{}", "9".repeat(36));
        let input = format!("first={stateless} second={legacy} short=ghs_abc123");
        let redacted = redact_ghs_tokens(&input);
        assert_eq!(
            redacted,
            "first=[REDACTED] second=[REDACTED] short=ghs_abc123"
        );
    }
}
