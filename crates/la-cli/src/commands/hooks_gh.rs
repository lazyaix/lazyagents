//! Built-in Rust hook evaluator (`hook eval`, `hook test`) and read-only GitHub audit
//! (`github audit`) CLI subcommands.

use std::fs;
use std::io::{self, Read as _, Write};
use std::path::{Path, PathBuf};

use la_core::{HookGuardKind, HookProtocol, ProjectManifest, evaluate_hook_json};
use la_runtime::{
    audit_github_repo, discover_workspace_root, load_workspace, probe_git_hook_context,
};

use crate::cli::{GithubCommand, HookCommand, OutputFormat};
use crate::commands::CommandOutcome;

pub(super) fn cmd_hook(
    explicit_dir: Option<&Path>,
    process_cwd: &Path,
    command: HookCommand,
    out: &mut impl Write,
    _err: &mut impl Write,
) -> CommandOutcome {
    match command {
        HookCommand::Eval { guard, protocol } => {
            let guard_kind = match HookGuardKind::parse(&guard) {
                Ok(k) => k,
                Err(err) => return CommandOutcome::RuntimeError(err.to_string()),
            };
            let mut stdin_buf = String::new();
            if let Err(io_err) = io::stdin().read_to_string(&mut stdin_buf) {
                return CommandOutcome::RuntimeError(format!("failed to read stdin: {io_err}"));
            }
            if stdin_buf.trim().is_empty() {
                "{}".clone_into(&mut stdin_buf);
            }
            let effective_root = resolve_hook_eval_root(explicit_dir, process_cwd, &stdin_buf);
            let manifest = load_manifest_or_default(&effective_root);
            let ctx = probe_git_hook_context(&effective_root);
            match evaluate_hook_json(guard_kind, protocol.into(), &stdin_buf, &manifest, &ctx) {
                Ok(json) => {
                    let _ = writeln!(out, "{json}");
                    CommandOutcome::Pass
                }
                Err(err) => CommandOutcome::RuntimeError(err.to_string()),
            }
        }
        HookCommand::Test {
            guard,
            input,
            command,
            path,
            content,
        } => {
            let guard_kind = match HookGuardKind::parse(&guard) {
                Ok(k) => k,
                Err(err) => return CommandOutcome::RuntimeError(err.to_string()),
            };
            let effective_root = explicit_dir.map_or_else(
                || {
                    discover_workspace_root(process_cwd)
                        .unwrap_or_else(|| process_cwd.to_path_buf())
                },
                |dir| discover_workspace_root(dir).unwrap_or_else(|| dir.to_path_buf()),
            );
            let payload = match build_test_payload(
                &effective_root,
                input.as_deref(),
                command.as_deref(),
                path.as_deref(),
                content.as_deref(),
            ) {
                Ok(p) => p,
                Err(msg) => return CommandOutcome::RuntimeError(msg),
            };
            let manifest = load_manifest_or_default(&effective_root);
            let ctx = probe_git_hook_context(&effective_root);
            match evaluate_hook_json(
                guard_kind,
                HookProtocol::Antigravity,
                &payload,
                &manifest,
                &ctx,
            ) {
                Ok(json) => {
                    let _ = writeln!(out, "{json}");
                    CommandOutcome::Pass
                }
                Err(err) => CommandOutcome::RuntimeError(err.to_string()),
            }
        }
    }
}

/// Resolves the effective workspace root for `lazyagents hook eval`.
///
/// Antigravity runs `.agents/hooks.json` handlers with process CWD set to `.agents/`, while
/// passing the agent's working directory (`cwd`) and `workspacePaths` in stdin JSON.
fn resolve_hook_eval_root(
    explicit_dir: Option<&Path>,
    process_cwd: &Path,
    stdin_json: &str,
) -> PathBuf {
    if let Some(dir) = explicit_dir {
        return discover_workspace_root(dir).unwrap_or_else(|| dir.to_path_buf());
    }

    let payload_candidates = extract_hook_workspace_candidates(stdin_json);
    for candidate in &payload_candidates {
        if let Some(found) = discover_workspace_root(candidate) {
            return found;
        }
    }

    if let Some(found) = discover_workspace_root(process_cwd) {
        return found;
    }

    if let Some(existing) = payload_candidates.into_iter().find(|p| p.is_dir()) {
        return existing;
    }

    process_cwd.to_path_buf()
}

fn extract_hook_workspace_candidates(stdin_json: &str) -> Vec<PathBuf> {
    let Ok(val) = serde_json::from_str::<serde_json::Value>(stdin_json) else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    if let Some(cwd) = val.get("cwd").and_then(serde_json::Value::as_str)
        && !cwd.trim().is_empty()
    {
        candidates.push(PathBuf::from(cwd.trim()));
    }
    if let Some(paths) = val
        .get("workspacePaths")
        .and_then(serde_json::Value::as_array)
    {
        for item in paths.iter().filter_map(serde_json::Value::as_str) {
            let trimmed = item.trim();
            if !trimmed.is_empty() {
                let pb = PathBuf::from(trimmed);
                if !candidates.contains(&pb) {
                    candidates.push(pb);
                }
            }
        }
    }
    candidates
}

pub(super) fn cmd_github(
    root: &Path,
    command: GithubCommand,
    out: &mut impl Write,
) -> CommandOutcome {
    match command {
        GithubCommand::Audit { repo, format } => {
            let manifest = load_manifest_or_default(root);
            let report = match audit_github_repo(root, &manifest, repo.as_deref()) {
                Ok(r) => r,
                Err(err) => return CommandOutcome::RuntimeError(err.to_string()),
            };

            match format {
                OutputFormat::Json => match report.to_json_pretty() {
                    Ok(json) => {
                        let _ = write!(out, "{json}");
                    }
                    Err(err) => return CommandOutcome::RuntimeError(err.to_string()),
                },
                OutputFormat::Text | OutputFormat::Github => {
                    let _ = write!(out, "{}", report.format_text());
                }
            }

            if report.has_mismatches() {
                CommandOutcome::CheckFailed
            } else {
                CommandOutcome::Pass
            }
        }
    }
}

fn load_manifest_or_default(root: &Path) -> ProjectManifest {
    load_workspace(root, true)
        .map_or_else(|_| ProjectManifest::default(), |ws| ws.resolved.manifest)
}

fn build_test_payload(
    root: &Path,
    input: Option<&str>,
    command: Option<&str>,
    path: Option<&str>,
    content: Option<&str>,
) -> Result<String, String> {
    if let Some(src) = input {
        if src == "-" {
            let mut buf = String::new();
            io::stdin()
                .read_to_string(&mut buf)
                .map_err(|err| format!("failed to read stdin: {err}"))?;
            return Ok(buf);
        }
        return fs::read_to_string(src)
            .map_err(|err| format!("failed to read fixture `{src}`: {err}"));
    }

    let mut args = serde_json::Map::new();
    if let Some(cmd) = command {
        args.insert(
            "CommandLine".to_owned(),
            serde_json::Value::String(cmd.to_owned()),
        );
    }
    if let Some(p) = path {
        args.insert("Path".to_owned(), serde_json::Value::String(p.to_owned()));
    }
    if let Some(body) = content {
        args.insert(
            "Content".to_owned(),
            serde_json::Value::String(body.to_owned()),
        );
    }

    let payload = serde_json::json!({
        "cwd": root.to_string_lossy(),
        "toolCall": {
            "args": serde_json::Value::Object(args),
        },
        "terminationReason": "NO_TOOL_CALL",
        "continuationActive": false,
    });
    serde_json::to_string(&payload).map_err(|err| err.to_string())
}
