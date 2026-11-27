//! Atomic write-plan executor backed by same-directory temporary files (`tempfile`)
//! and `.lazyagents/lock.json` persistence.

use std::fs;
use std::io::Write as _;
use std::path::Path;

use la_core::{FileAction, WritePlan};
use tempfile::NamedTempFile;
use tracing::info;

use crate::error::RuntimeError;

/// Options controlling how [`apply_write_plan`] executes filesystem changes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ApplyOptions {
    /// Overwrite externally modified files (`FileAction::ExternalConflict`) instead of failing.
    pub force: bool,
    /// Compute the summary without touching any files on disk.
    pub dry_run: bool,
}

/// Summary of filesystem actions performed (or planned under `dry_run`) by [`apply_write_plan`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ApplySummary {
    /// Newly created workspace-relative paths.
    pub created: Vec<String>,
    /// Updated workspace-relative paths.
    pub updated: Vec<String>,
    /// Deleted stale managed paths.
    pub deleted: Vec<String>,
    /// Unchanged paths left untouched.
    pub unchanged: Vec<String>,
}

/// Atomically writes `content` to `dest`, creating missing parent directories and persisting
/// via a same-directory temporary file so interrupted writes never corrupt existing targets.
///
/// # Errors
///
/// Returns [`RuntimeError::Io`] on directory creation, write, or atomic rename failure.
pub fn write_atomic(dest: &Path, content: &str) -> Result<(), RuntimeError> {
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|err| RuntimeError::io(parent.display().to_string(), &err))?;

    let mut tmp = NamedTempFile::new_in(parent)
        .map_err(|err| RuntimeError::io(dest.display().to_string(), &err))?;
    tmp.write_all(content.as_bytes())
        .map_err(|err| RuntimeError::io(dest.display().to_string(), &err))?;
    tmp.flush()
        .map_err(|err| RuntimeError::io(dest.display().to_string(), &err))?;
    tmp.persist(dest)
        .map_err(|err| RuntimeError::io(dest.display().to_string(), &err.error))?;
    Ok(())
}

/// Executes a [`WritePlan`] against `workspace_root` and atomically persists
/// `.lazyagents/lock.json` on completion.
///
/// # Errors
///
/// Returns [`RuntimeError::ExternalConflict`] if any file was modified outside `lazyagents`
/// and `options.force` is `false`, or [`RuntimeError::Io`] / [`RuntimeError::Core`] on I/O
/// or lockfile serialization failure.
pub fn apply_write_plan(
    workspace_root: &Path,
    plan: &WritePlan,
    options: ApplyOptions,
) -> Result<ApplySummary, RuntimeError> {
    if !options.force && plan.has_conflicts() {
        let conflict_paths: Vec<&str> = plan
            .files
            .iter()
            .filter(|f| f.action == FileAction::ExternalConflict)
            .map(|f| f.path.as_str())
            .collect();
        return Err(RuntimeError::ExternalConflict {
            count: conflict_paths.len(),
            paths: conflict_paths.join(", "),
        });
    }

    let mut summary = ApplySummary::default();

    for file in &plan.files {
        let full_path = workspace_root.join(&file.path);
        match file.action {
            FileAction::Unchanged => summary.unchanged.push(file.path.clone()),
            FileAction::Create => {
                if !options.dry_run
                    && let Some(content) = &file.final_content
                {
                    write_atomic(&full_path, content)?;
                }
                summary.created.push(file.path.clone());
            }
            FileAction::Update | FileAction::ExternalConflict => {
                if let Some(content) = &file.final_content {
                    if !options.dry_run {
                        write_atomic(&full_path, content)?;
                    }
                    summary.updated.push(file.path.clone());
                } else {
                    if !options.dry_run && full_path.exists() {
                        fs::remove_file(&full_path)
                            .map_err(|err| RuntimeError::io(file.path.clone(), &err))?;
                    }
                    summary.deleted.push(file.path.clone());
                }
            }
            FileAction::Delete => {
                if !options.dry_run && full_path.exists() {
                    fs::remove_file(&full_path)
                        .map_err(|err| RuntimeError::io(file.path.clone(), &err))?;
                }
                summary.deleted.push(file.path.clone());
            }
        }
    }

    if !options.dry_run {
        let lock_json = plan.next_lock.to_json_pretty()?;
        let lock_path = workspace_root.join(".lazyagents").join("lock.json");
        write_atomic(&lock_path, &lock_json)?;
        info!(
            created = summary.created.len(),
            updated = summary.updated.len(),
            deleted = summary.deleted.len(),
            unchanged = summary.unchanged.len(),
            "applied write plan and updated lockfile"
        );
    }

    Ok(summary)
}
