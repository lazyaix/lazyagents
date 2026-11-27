//! Managed SHA-256 lockfile (`.lazyagents/lock.json`) and fenced managed-region
//! splicing for shared repository files (`.github/CODEOWNERS`, `.gitignore`, PR templates).

use core::fmt::Write as _;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::CoreError;
use crate::template::normalize_lf;

/// Current `.lazyagents/lock.json` format version.
pub const LOCKFILE_SCHEMA_VERSION: u32 = 1;

/// Comment syntax used to delimit a managed fenced region inside a shared repository file.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedRegionStyle {
    /// Hash-prefixed markers (`# >>> lazyagents managed start` / `# <<< lazyagents managed end`).
    HashComment,
    /// HTML comment markers (`<!-- lazyagents:start -->` / `<!-- lazyagents:end -->`).
    HtmlComment,
}

/// Ownership mode tracked for each managed output path in `.lazyagents/lock.json`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum OwnershipMode {
    /// `lazyagents` owns the entire file content.
    WholeFile,
    /// `lazyagents` owns only the fenced managed block while preserving surrounding user lines.
    ManagedRegion {
        /// Marker syntax used to delimit the managed block.
        style: ManagedRegionStyle,
    },
}

/// Single managed file record inside `.lazyagents/lock.json`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LockEntry {
    /// Ownership mode for this path.
    pub ownership: OwnershipMode,
    /// Lowercase hex SHA-256 of the managed payload (whole file or fenced block content).
    pub sha256: String,
}

/// Deterministic `.lazyagents/lock.json` representation tracking managed outputs for drift
/// and external-edit protection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Lockfile {
    /// Lockfile schema version (currently [`LOCKFILE_SCHEMA_VERSION`]).
    pub schema_version: u32,
    /// Sorted map from workspace-relative POSIX path to its recorded ownership and SHA-256 hash.
    #[serde(default)]
    pub entries: BTreeMap<String, LockEntry>,
}

impl Default for Lockfile {
    fn default() -> Self {
        Self {
            schema_version: LOCKFILE_SCHEMA_VERSION,
            entries: BTreeMap::new(),
        }
    }
}

impl ManagedRegionStyle {
    /// Returns the opening and closing fence marker lines for this style.
    #[must_use]
    pub const fn markers(self) -> (&'static str, &'static str) {
        match self {
            Self::HashComment => (
                "# >>> lazyagents managed start",
                "# <<< lazyagents managed end",
            ),
            Self::HtmlComment => ("<!-- lazyagents:start -->", "<!-- lazyagents:end -->"),
        }
    }
}

impl Lockfile {
    /// Parses a `.lazyagents/lock.json` JSON document.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Lockfile`] on invalid JSON or an unsupported schema version.
    pub fn from_json_str(input: &str) -> Result<Self, CoreError> {
        let parsed: Self = serde_json::from_str(input).map_err(|err| CoreError::Lockfile {
            message: err.to_string(),
        })?;
        if parsed.schema_version != LOCKFILE_SCHEMA_VERSION {
            return Err(CoreError::Lockfile {
                message: format!(
                    "unsupported lockfile schema_version {}; expected {}",
                    parsed.schema_version, LOCKFILE_SCHEMA_VERSION
                ),
            });
        }
        Ok(parsed)
    }

    /// Serializes the lockfile to deterministically formatted pretty JSON with trailing LF.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Lockfile`] if JSON serialization fails.
    pub fn to_json_pretty(&self) -> Result<String, CoreError> {
        let mut json = serde_json::to_string_pretty(self).map_err(|err| CoreError::Lockfile {
            message: err.to_string(),
        })?;
        json.push('\n');
        Ok(json)
    }
}

/// Computes the lowercase hexadecimal SHA-256 digest of `bytes`.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    for byte in digest {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Wraps `body` in the opening and closing fence markers for `style`.
#[must_use]
pub fn render_managed_block(style: ManagedRegionStyle, body: &str) -> String {
    let (start, end) = style.markers();
    let normalized = normalize_lf(body);
    let trimmed = normalized.trim_end_matches('\n');
    if trimmed.is_empty() {
        format!("{start}\n{end}\n")
    } else {
        format!("{start}\n{trimmed}\n{end}\n")
    }
}

/// Extracts the normalized fenced block (including markers and trailing LF) from `content`,
/// returning `Ok(None)` when no marker is present.
///
/// # Errors
///
/// Returns [`CoreError::Lockfile`] when only one marker is present or markers appear out of order.
pub fn extract_managed_region(
    content: &str,
    style: ManagedRegionStyle,
) -> Result<Option<String>, CoreError> {
    let normalized = normalize_lf(content);
    let (start_marker, end_marker) = style.markers();
    let Some(start_idx) = normalized.find(start_marker) else {
        if normalized.contains(end_marker) {
            return Err(CoreError::Lockfile {
                message: format!("found `{end_marker}` without matching `{start_marker}`"),
            });
        }
        return Ok(None);
    };
    let after_start = normalized.get(start_idx..).unwrap_or_default();
    let Some(rel_end) = after_start.find(end_marker) else {
        return Err(CoreError::Lockfile {
            message: format!("found `{start_marker}` without matching `{end_marker}`"),
        });
    };
    let end_idx = start_idx
        .saturating_add(rel_end)
        .saturating_add(end_marker.len());
    let mut block = normalized
        .get(start_idx..end_idx)
        .unwrap_or_default()
        .to_owned();
    block.push('\n');
    Ok(Some(block))
}

/// Splices a managed region block into `existing` (or returns the rendered block if `existing`
/// is `None`), preserving all user content outside the fence markers.
///
/// # Errors
///
/// Returns [`CoreError::Lockfile`] if `existing` contains malformed/unpaired fence markers.
pub fn splice_managed_region(
    existing: Option<&str>,
    style: ManagedRegionStyle,
    body: &str,
) -> Result<String, CoreError> {
    let rendered_block = render_managed_block(style, body);
    let Some(raw_existing) = existing else {
        return Ok(rendered_block);
    };

    let normalized = normalize_lf(raw_existing);
    let (start_marker, end_marker) = style.markers();
    let Some(start_idx) = normalized.find(start_marker) else {
        if normalized.contains(end_marker) {
            return Err(CoreError::Lockfile {
                message: format!("found `{end_marker}` without matching `{start_marker}`"),
            });
        }
        if normalized.trim().is_empty() {
            return Ok(rendered_block);
        }
        let mut out = normalized;
        if !out.ends_with('\n') {
            out.push('\n');
        }
        if !out.ends_with("\n\n") {
            out.push('\n');
        }
        out.push_str(&rendered_block);
        return Ok(out);
    };

    let after_start = normalized.get(start_idx..).unwrap_or_default();
    let Some(rel_end) = after_start.find(end_marker) else {
        return Err(CoreError::Lockfile {
            message: format!("found `{start_marker}` without matching `{end_marker}`"),
        });
    };
    let mut end_idx = start_idx
        .saturating_add(rel_end)
        .saturating_add(end_marker.len());
    if normalized.as_bytes().get(end_idx) == Some(&b'\n') {
        end_idx = end_idx.saturating_add(1);
    }

    let prefix = normalized.get(..start_idx).unwrap_or_default();
    let suffix = normalized.get(end_idx..).unwrap_or_default();
    Ok(format!("{prefix}{rendered_block}{suffix}"))
}

#[cfg(test)]
mod tests {
    use core::assert_matches;

    use super::*;

    #[test]
    fn sha256_hex_matches_known_vector() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn splice_managed_region_preserves_surrounding_custom_lines() {
        let initial = "* @team\n\n# custom note\n";
        let first = splice_managed_region(
            Some(initial),
            ManagedRegionStyle::HashComment,
            "/.lazyagents/ @sec\n",
        )
        .unwrap();
        assert!(first.starts_with("* @team\n\n# custom note\n\n# >>> lazyagents managed start\n"));
        assert!(first.contains("/.lazyagents/ @sec\n"));

        let updated = splice_managed_region(
            Some(&first),
            ManagedRegionStyle::HashComment,
            "/.lazyagents/ @sec\n/.agents/ @sec\n",
        )
        .unwrap();
        assert!(
            updated.starts_with("* @team\n\n# custom note\n\n# >>> lazyagents managed start\n")
        );
        assert!(updated.contains("/.agents/ @sec\n"));
        assert_eq!(
            extract_managed_region(&updated, ManagedRegionStyle::HashComment)
                .unwrap()
                .unwrap(),
            render_managed_block(
                ManagedRegionStyle::HashComment,
                "/.lazyagents/ @sec\n/.agents/ @sec\n"
            )
        );
    }

    #[test]
    fn rejects_unpaired_managed_markers() {
        let broken = "# >>> lazyagents managed start\nfoo\n";
        assert_matches!(
            splice_managed_region(Some(broken), ManagedRegionStyle::HashComment, "bar"),
            Err(CoreError::Lockfile { .. })
        );
    }

    #[test]
    fn lockfile_roundtrips_json_cleanly() {
        let mut lock = Lockfile::default();
        lock.entries.insert(
            "AGENTS.md".to_owned(),
            LockEntry {
                ownership: OwnershipMode::WholeFile,
                sha256: sha256_hex(b"# AGENTS\n"),
            },
        );
        let raw = lock.to_json_pretty().unwrap();
        let parsed = Lockfile::from_json_str(&raw).unwrap();
        assert_eq!(lock, parsed);
    }
}
