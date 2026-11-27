//! Opaque secret wrappers, stateless GitHub installation-token redaction,
//! and deterministic plaintext secret scanning for tracked configuration files.

use core::fmt;

/// Minimum suffix length after a GitHub `ghs_` installation-token prefix to treat as a secret.
const MIN_GHS_TOKEN_SUFFIX_LEN: usize = 36;

/// Minimum suffix length for classic GitHub PATs (`ghp_`, `gho_`, `ghu_`).
const MIN_GH_CLASSIC_SUFFIX_LEN: usize = 36;

/// Minimum suffix length for fine-grained GitHub PATs (`github_pat_`).
const MIN_GH_FINE_GRAINED_SUFFIX_LEN: usize = 22;

/// Minimum suffix length for `OpenAI` / Anthropic `sk-` API keys.
const MIN_SK_KEY_SUFFIX_LEN: usize = 20;

/// Exact suffix length after `AIza` for standard Google API keys.
const MIN_GOOGLE_API_KEY_SUFFIX_LEN: usize = 35;

/// Minimum token length for a literal `Bearer <token>` header value.
const MIN_BEARER_LITERAL_LEN: usize = 16;

/// Category of plaintext credential detected by [`scan_secrets`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SecretKind {
    /// GitHub App installation token (`ghs_[A-Za-z0-9._-]{36,}`, including ~520-char stateless JWTs).
    GitHubStatelessToken,
    /// GitHub personal/OAuth access token (`ghp_`, `gho_`, `ghu_`, or `github_pat_`).
    GitHubPersonalToken,
    /// Anthropic API key (`sk-ant-...`).
    AnthropicApiKey,
    /// OpenAI-style secret key (`sk-...`).
    OpenAiApiKey,
    /// Google API key (`AIza...`).
    GoogleApiKey,
    /// Plaintext `Bearer <literal>` authorization token (excluding `${VAR}` placeholders).
    BearerLiteral,
}

/// Non-sensitive diagnostic record of a detected plaintext secret.
///
/// Intentionally omits the raw secret value so findings are always safe to log or display.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretFinding {
    /// Classification of the detected credential.
    pub kind: SecretKind,
    /// 1-based line number within the scanned text.
    pub line: usize,
    /// 1-based byte column within that line.
    pub column: usize,
    /// Short non-sensitive prefix preview (e.g., `"ghs_..."` or `"Bearer ..."`).
    pub preview: &'static str,
}

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
        let token_suffix_len = count_token_bytes(suffix, true);

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

/// Scans arbitrary text (manifests, MCP configs, rules, hooks) for plaintext credentials.
///
/// Recognizes variable-length stateless `ghs_*` tokens, GitHub PATs, provider API keys, and
/// literal `Bearer <token>` values while intentionally allowing safe placeholders such as
/// `${GITHUB_TOKEN}`, `$TOKEN`, `<token>`, and `[REDACTED]`.
///
/// # Examples
///
/// ```
/// use la_core::{SecretKind, scan_secrets};
///
/// let safe = "Authorization = \"Bearer ${GITHUB_MCP_TOKEN}\"";
/// assert!(scan_secrets(safe).is_empty());
///
/// let leaked = format!("token = \"ghs_{}.{}\"", "a".repeat(25), "b".repeat(25));
/// let findings = scan_secrets(&leaked);
/// assert_eq!(findings.len(), 1);
/// assert_eq!(findings[0].kind, SecretKind::GitHubStatelessToken);
/// ```
#[must_use]
pub fn scan_secrets(input: &str) -> Vec<SecretFinding> {
    let mut findings = Vec::new();
    for (idx, line) in input.lines().enumerate() {
        let line_no = idx.saturating_add(1);
        scan_line_for_prefixed_tokens(line, line_no, &mut findings);
        scan_line_for_bearer_literals(line, line_no, &mut findings);
    }
    findings
}

fn scan_line_for_prefixed_tokens(line: &str, line_no: usize, findings: &mut Vec<SecretFinding>) {
    let patterns: [(&str, usize, bool, SecretKind, &str); 8] = [
        (
            "ghs_",
            MIN_GHS_TOKEN_SUFFIX_LEN,
            true,
            SecretKind::GitHubStatelessToken,
            "ghs_...",
        ),
        (
            "github_pat_",
            MIN_GH_FINE_GRAINED_SUFFIX_LEN,
            false,
            SecretKind::GitHubPersonalToken,
            "github_pat_...",
        ),
        (
            "ghp_",
            MIN_GH_CLASSIC_SUFFIX_LEN,
            false,
            SecretKind::GitHubPersonalToken,
            "ghp_...",
        ),
        (
            "gho_",
            MIN_GH_CLASSIC_SUFFIX_LEN,
            false,
            SecretKind::GitHubPersonalToken,
            "gho_...",
        ),
        (
            "ghu_",
            MIN_GH_CLASSIC_SUFFIX_LEN,
            false,
            SecretKind::GitHubPersonalToken,
            "ghu_...",
        ),
        (
            "sk-ant-",
            MIN_SK_KEY_SUFFIX_LEN,
            false,
            SecretKind::AnthropicApiKey,
            "sk-ant-...",
        ),
        (
            "sk-",
            MIN_SK_KEY_SUFFIX_LEN,
            false,
            SecretKind::OpenAiApiKey,
            "sk-...",
        ),
        (
            "AIza",
            MIN_GOOGLE_API_KEY_SUFFIX_LEN,
            false,
            SecretKind::GoogleApiKey,
            "AIza...",
        ),
    ];

    for (prefix, min_len, allow_dot, kind, preview) in patterns {
        let mut offset = 0usize;
        while let Some(slice) = line.get(offset..) {
            let Some(rel_pos) = slice.find(prefix) else {
                break;
            };
            let abs_pos = offset.saturating_add(rel_pos);
            let after_prefix_idx = abs_pos.saturating_add(prefix.len());
            let suffix = line.get(after_prefix_idx..).unwrap_or_default();
            let suffix_len = count_token_bytes(suffix, allow_dot);
            let is_ant_submatch = prefix == "sk-"
                && line
                    .get(abs_pos..)
                    .is_some_and(|s| s.starts_with("sk-ant-"));
            let has_boundary = abs_pos == 0
                || line
                    .as_bytes()
                    .get(abs_pos.saturating_sub(1))
                    .is_none_or(|b| !b.is_ascii_alphanumeric() && *b != b'_');

            if !is_ant_submatch && has_boundary && suffix_len >= min_len {
                findings.push(SecretFinding {
                    kind,
                    line: line_no,
                    column: abs_pos.saturating_add(1),
                    preview,
                });
                offset = after_prefix_idx.saturating_add(suffix_len);
            } else {
                offset = after_prefix_idx;
            }
        }
    }
}

fn scan_line_for_bearer_literals(line: &str, line_no: usize, findings: &mut Vec<SecretFinding>) {
    let mut offset = 0usize;
    while let Some(slice) = line.get(offset..) {
        let Some(rel_pos) = slice.find("Bearer ") else {
            break;
        };
        let abs_pos = offset.saturating_add(rel_pos);
        let value_start = abs_pos.saturating_add("Bearer ".len());
        let rest = line.get(value_start..).unwrap_or_default();
        let token_len = rest
            .bytes()
            .take_while(|b| !b.is_ascii_whitespace() && !matches!(b, b'"' | b'\'' | b',' | b'}'))
            .count();
        let candidate = rest.get(..token_len).unwrap_or_default();

        let already_matched_prefix = candidate.starts_with("ghs_")
            || candidate.starts_with("ghp_")
            || candidate.starts_with("gho_")
            || candidate.starts_with("ghu_")
            || candidate.starts_with("github_pat_")
            || candidate.starts_with("sk-")
            || candidate.starts_with("AIza");

        if !already_matched_prefix
            && token_len >= MIN_BEARER_LITERAL_LEN
            && !is_safe_placeholder(candidate)
        {
            findings.push(SecretFinding {
                kind: SecretKind::BearerLiteral,
                line: line_no,
                column: abs_pos.saturating_add(1),
                preview: "Bearer ...",
            });
        }
        offset = value_start.saturating_add(token_len.max(1));
    }
}

fn is_safe_placeholder(candidate: &str) -> bool {
    candidate.starts_with("${")
        || candidate.starts_with('$')
        || candidate.starts_with("{{")
        || (candidate.starts_with('<') && candidate.ends_with('>'))
        || candidate.eq_ignore_ascii_case("[REDACTED]")
        || candidate.eq_ignore_ascii_case("REDACTED")
        || candidate.eq_ignore_ascii_case("YOUR_TOKEN_HERE")
}

fn count_token_bytes(input: &str, allow_dot: bool) -> usize {
    input
        .bytes()
        .take_while(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'_' | b'-')
                || (allow_dot && *byte == b'.')
        })
        .count()
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

    #[test]
    fn scan_secrets_detects_real_tokens_and_ignores_placeholders() {
        let stateless = format!("ghs_{}.{}", "a".repeat(25), "b".repeat(25));
        let pat = format!("github_pat_{}", "x".repeat(30));
        let ant = format!("sk-ant-{}", "z".repeat(24));
        let raw = format!(
            "ok1 = \"Bearer ${{GITHUB_TOKEN}}\"\nok2 = \"Bearer [REDACTED]\"\nleak1 = \"{stateless}\"\nleak2 = \"{pat}\"\nleak3 = \"{ant}\"\nleak4 = \"Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9\"\n"
        );
        let findings = scan_secrets(&raw);
        let kinds: Vec<SecretKind> = findings.iter().map(|f| f.kind).collect();
        assert_eq!(
            kinds,
            vec![
                SecretKind::GitHubStatelessToken,
                SecretKind::GitHubPersonalToken,
                SecretKind::AnthropicApiKey,
                SecretKind::BearerLiteral,
            ]
        );
        assert_eq!(findings[0].line, 3);
        assert_eq!(findings[3].line, 6);
    }
}
