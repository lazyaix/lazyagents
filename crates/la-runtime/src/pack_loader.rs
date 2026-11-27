//! Offline local directory pack loader (`PackSource::Local`) with `include_only` / `exclude`
//! glob filtering and clear M4 notices for reserved remote packs.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use globset::{Glob, GlobSet, GlobSetBuilder};
use la_core::manifest::{PackRef, PackSource};
use tracing::debug;

use crate::error::RuntimeError;

/// Loads file contents from all configured local directory packs in `packs` into a
/// `.lazyagents/`-style relative POSIX path map.
///
/// Built-in packs are resolved directly inside `la-core::catalog`, while reserved `Git` and
/// `Registry` pack sources return a clear error pointing to M4.
///
/// # Errors
///
/// Returns [`RuntimeError::Pack`] or [`RuntimeError::Io`] if a local pack directory is missing,
/// a filter glob is invalid, or an unsupported reserved remote pack is referenced.
pub fn load_local_packs(
    workspace_root: &Path,
    packs: &[PackRef],
) -> Result<BTreeMap<String, String>, RuntimeError> {
    let mut loaded = BTreeMap::new();

    for pack in packs {
        match &pack.source {
            PackSource::Builtin { .. } => {}
            PackSource::Local { path } => {
                let pack_dir = resolve_pack_path(workspace_root, path);
                if !pack_dir.is_dir() {
                    return Err(RuntimeError::Pack {
                        pack_id: pack.id.to_string(),
                        message: format!(
                            "local pack directory `{}` does not exist",
                            pack_dir.display()
                        ),
                    });
                }
                let includes = build_glob_set(&pack.id.to_string(), &pack.include_only)?;
                let excludes = build_glob_set(&pack.id.to_string(), &pack.exclude)?;
                collect_pack_files(
                    &pack_dir,
                    &pack_dir,
                    includes.as_ref(),
                    excludes.as_ref(),
                    &mut loaded,
                )?;
                debug!(pack = %pack.id, dir = %pack_dir.display(), "loaded local pack");
            }
            PackSource::Git { url, .. } => {
                return Err(RuntimeError::Pack {
                    pack_id: pack.id.to_string(),
                    message: format!(
                        "remote Git pack `{url}` is reserved for M4; clone locally and use `kind = \"local\"` in V1"
                    ),
                });
            }
            PackSource::Registry { package, .. } => {
                return Err(RuntimeError::Pack {
                    pack_id: pack.id.to_string(),
                    message: format!(
                        "registry pack `{package}` is reserved for M4; use a built-in or local pack in V1"
                    ),
                });
            }
        }
    }

    Ok(loaded)
}

fn resolve_pack_path(workspace_root: &Path, raw: &str) -> std::path::PathBuf {
    let path = Path::new(raw);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    }
}

fn build_glob_set(pack_id: &str, patterns: &[String]) -> Result<Option<GlobSet>, RuntimeError> {
    if patterns.is_empty() {
        return Ok(None);
    }
    let mut builder = GlobSetBuilder::new();
    for pat in patterns {
        let glob = Glob::new(pat).map_err(|err| RuntimeError::Pack {
            pack_id: pack_id.to_owned(),
            message: format!("invalid glob `{pat}`: {err}"),
        })?;
        builder.add(glob);
    }
    let set = builder.build().map_err(|err| RuntimeError::Pack {
        pack_id: pack_id.to_owned(),
        message: format!("failed to compile glob set: {err}"),
    })?;
    Ok(Some(set))
}

fn collect_pack_files(
    pack_root: &Path,
    dir: &Path,
    includes: Option<&GlobSet>,
    excludes: Option<&GlobSet>,
    out: &mut BTreeMap<String, String>,
) -> Result<(), RuntimeError> {
    let entries =
        fs::read_dir(dir).map_err(|err| RuntimeError::io(dir.display().to_string(), &err))?;
    for entry_res in entries {
        let entry = entry_res.map_err(|err| RuntimeError::io(dir.display().to_string(), &err))?;
        let path = entry.path();
        if path.is_dir() {
            collect_pack_files(pack_root, &path, includes, excludes, out)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let Ok(rel) = path.strip_prefix(pack_root) else {
            continue;
        };
        let rel_posix = rel.to_string_lossy().replace('\\', "/");
        if let Some(inc) = includes
            && !inc.is_match(&rel_posix)
        {
            continue;
        }
        if let Some(exc) = excludes
            && exc.is_match(&rel_posix)
        {
            continue;
        }
        let content = fs::read_to_string(&path)
            .map_err(|err| RuntimeError::io(path.display().to_string(), &err))?;
        out.insert(rel_posix, content);
    }
    Ok(())
}
