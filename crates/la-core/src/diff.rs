//! Pure Sans-I/O write planner, drift detector, and unified diff generator powered by `similar`.

use std::collections::BTreeMap;

use similar::TextDiff;

use crate::compiler::GeneratedFile;
use crate::error::CoreError;
use crate::lock::{
    LockEntry, Lockfile, OwnershipMode, extract_managed_region, render_managed_block, sha256_hex,
    splice_managed_region,
};
use crate::template::normalize_lf;

/// Planned filesystem operation for a single workspace path.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FileAction {
    /// File does not yet exist on disk and will be created.
    Create,
    /// Existing managed file (or managed region) differs from compiled output and will be updated.
    Update,
    /// Existing file matches compiled output byte-for-byte.
    Unchanged,
    /// Existing file was modified externally (hash differs from lockfile or is unmanaged).
    ExternalConflict,
    /// Previously managed whole-file artifact is no longer produced and can be removed.
    Delete,
}

/// Per-file diff and ownership plan item produced by [`plan_writes`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilePlan {
    /// Workspace-relative POSIX path.
    pub path: String,
    /// Ownership strategy for this path.
    pub ownership: OwnershipMode,
    /// Planned action for this path.
    pub action: FileAction,
    /// Full resulting file content to write (present for `Create`, `Update`, `ExternalConflict`, `Unchanged`).
    pub final_content: Option<String>,
    /// Unified diff comparing disk content to `final_content` (empty when `Unchanged`).
    pub unified_diff: String,
    /// Whether this path has drifted relative to compiled output or `.lazyagents/lock.json`.
    pub drifted: bool,
}

/// Complete deterministic write plan paired with the next `.lazyagents/lock.json` state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WritePlan {
    /// Ordered per-file plans (sorted by path).
    pub files: Vec<FilePlan>,
    /// Updated lockfile reflecting all newly compiled outputs.
    pub next_lock: Lockfile,
}

impl WritePlan {
    /// Returns `true` when any managed path has drifted (needs create/update/delete or conflicts).
    #[must_use]
    pub fn has_drift(&self) -> bool {
        self.files.iter().any(|file| file.drifted)
    }

    /// Returns `true` when any path was externally modified and requires `--force` or `import`.
    #[must_use]
    pub fn has_conflicts(&self) -> bool {
        self.files
            .iter()
            .any(|file| file.action == FileAction::ExternalConflict)
    }
}

/// Computes a deterministic [`WritePlan`] comparing compiled outputs against the previous
/// [`Lockfile`] and current in-memory workspace disk contents.
///
/// # Errors
///
/// Returns [`CoreError::Lockfile`] if a shared file contains corrupted/unpaired fence markers.
pub fn plan_writes(
    generated: &[GeneratedFile],
    previous_lock: Option<&Lockfile>,
    disk_files: &BTreeMap<String, String>,
) -> Result<WritePlan, CoreError> {
    let mut by_path: BTreeMap<String, FilePlan> = BTreeMap::new();
    let mut next_lock = Lockfile::default();

    for file in generated {
        let disk_raw = disk_files.get(&file.path).map(String::as_str);
        let prev_entry = previous_lock.and_then(|lock| lock.entries.get(&file.path));

        let (final_content, managed_hash, action) = match file.ownership {
            OwnershipMode::WholeFile => {
                let target = normalize_lf(&file.content);
                let target_hash = sha256_hex(target.as_bytes());
                let action = classify_whole_file(disk_raw, prev_entry, &target_hash);
                (target, target_hash, action)
            }
            OwnershipMode::ManagedRegion { style } => {
                let block = render_managed_block(style, &file.content);
                let block_hash = sha256_hex(block.as_bytes());
                let spliced = splice_managed_region(disk_raw, style, &file.content)?;
                let action = classify_managed_region(disk_raw, prev_entry, style, &block_hash)?;
                (spliced, block_hash, action)
            }
        };

        let unified_diff = if action == FileAction::Unchanged {
            String::new()
        } else {
            let before = disk_raw.map_or_default(normalize_lf);
            render_unified_diff(&file.path, &before, &final_content)
        };

        let drifted = action != FileAction::Unchanged;
        next_lock.entries.insert(
            file.path.clone(),
            LockEntry {
                ownership: file.ownership,
                sha256: managed_hash,
            },
        );

        by_path.insert(
            file.path.clone(),
            FilePlan {
                path: file.path.clone(),
                ownership: file.ownership,
                action,
                final_content: Some(final_content),
                unified_diff,
                drifted,
            },
        );
    }

    if let Some(prev_lock) = previous_lock {
        for (stale_path, stale_entry) in &prev_lock.entries {
            if by_path.contains_key(stale_path) {
                continue;
            }
            if stale_entry.ownership != OwnershipMode::WholeFile {
                continue;
            }
            if let Some(disk_raw) = disk_files.get(stale_path) {
                let normalized = normalize_lf(disk_raw);
                let disk_hash = sha256_hex(normalized.as_bytes());
                let action = if disk_hash == stale_entry.sha256 {
                    FileAction::Delete
                } else {
                    FileAction::ExternalConflict
                };
                let unified_diff = render_unified_diff(stale_path, &normalized, "");
                by_path.insert(
                    stale_path.clone(),
                    FilePlan {
                        path: stale_path.clone(),
                        ownership: stale_entry.ownership,
                        action,
                        final_content: None,
                        unified_diff,
                        drifted: true,
                    },
                );
            }
        }
    }

    Ok(WritePlan {
        files: by_path.into_values().collect(),
        next_lock,
    })
}

fn classify_whole_file(
    disk_raw: Option<&str>,
    prev_entry: Option<&LockEntry>,
    target_hash: &str,
) -> FileAction {
    let Some(raw) = disk_raw else {
        return FileAction::Create;
    };
    let disk_hash = sha256_hex(normalize_lf(raw).as_bytes());
    if disk_hash == target_hash {
        return FileAction::Unchanged;
    }
    match prev_entry {
        Some(entry) if entry.sha256 == disk_hash => FileAction::Update,
        _ => FileAction::ExternalConflict,
    }
}

fn classify_managed_region(
    disk_raw: Option<&str>,
    prev_entry: Option<&LockEntry>,
    style: crate::lock::ManagedRegionStyle,
    target_block_hash: &str,
) -> Result<FileAction, CoreError> {
    let Some(raw) = disk_raw else {
        return Ok(FileAction::Create);
    };
    let existing_block = extract_managed_region(raw, style)?;
    existing_block.map_or(Ok(FileAction::Update), |block| {
        let block_hash = sha256_hex(block.as_bytes());
        if block_hash == target_block_hash {
            Ok(FileAction::Unchanged)
        } else if prev_entry.is_some_and(|e| e.sha256 == block_hash) {
            Ok(FileAction::Update)
        } else {
            Ok(FileAction::ExternalConflict)
        }
    })
}

fn render_unified_diff(path: &str, before: &str, after: &str) -> String {
    TextDiff::from_lines(before, after)
        .unified_diff()
        .context_radius(3)
        .header(&format!("a/{path}"), &format!("b/{path}"))
        .to_string()
}

#[cfg(test)]
mod tests {
    use crate::lock::ManagedRegionStyle;

    use super::*;

    #[test]
    fn detects_create_unchanged_update_external_conflict_and_delete() {
        let gen1 = vec![
            GeneratedFile::whole_file("AGENTS.md", "# v1\n"),
            GeneratedFile::whole_file(".agents/rules/old.md", "old\n"),
        ];
        let empty_disk = BTreeMap::new();
        let plan1 = plan_writes(&gen1, None, &empty_disk).unwrap();
        assert!(plan1.has_drift());
        assert!(!plan1.has_conflicts());
        assert!(plan1.files.iter().all(|f| f.action == FileAction::Create));

        let mut disk = BTreeMap::new();
        disk.insert("AGENTS.md".to_owned(), "# v1\n".to_owned());
        disk.insert(".agents/rules/old.md".to_owned(), "old\n".to_owned());

        let clean = plan_writes(&gen1, Some(&plan1.next_lock), &disk).unwrap();
        assert!(!clean.has_drift());

        let gen2 = vec![GeneratedFile::whole_file("AGENTS.md", "# v2\n")];
        let update_and_delete = plan_writes(&gen2, Some(&plan1.next_lock), &disk).unwrap();
        assert_eq!(update_and_delete.files[0].action, FileAction::Delete);
        assert_eq!(update_and_delete.files[1].action, FileAction::Update);
        assert!(update_and_delete.files[1].unified_diff.contains("+# v2"));

        disk.insert("AGENTS.md".to_owned(), "# hand-edited\n".to_owned());
        let conflict = plan_writes(&gen2, Some(&plan1.next_lock), &disk).unwrap();
        assert!(conflict.has_conflicts());
        assert_eq!(conflict.files[1].action, FileAction::ExternalConflict);
    }

    #[test]
    fn managed_region_updates_cleanly_without_clobbering_custom_lines() {
        let generated = vec![GeneratedFile::managed_region(
            ".github/CODEOWNERS",
            ManagedRegionStyle::HashComment,
            "/.lazyagents/ @owner\n",
        )];
        let mut disk = BTreeMap::new();
        disk.insert(".github/CODEOWNERS".to_owned(), "* @default\n".to_owned());

        let plan = plan_writes(&generated, None, &disk).unwrap();
        assert_eq!(plan.files[0].action, FileAction::Update);
        let written = plan.files[0].final_content.as_ref().unwrap();
        assert!(written.contains("* @default"));
        assert!(written.contains("/.lazyagents/ @owner"));

        disk.insert(".github/CODEOWNERS".to_owned(), written.clone());
        let clean = plan_writes(&generated, Some(&plan.next_lock), &disk).unwrap();
        assert!(!clean.has_drift());
    }
}
