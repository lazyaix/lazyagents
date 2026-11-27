//! Built-in Rust hook evaluator (`hook eval`, `hook test`) and read-only GitHub audit
//! (`github audit`) CLI subcommands.

use std::fs;
use std::io::{self, Read as _, Write};
use std::path::Path;

use la_core::{HookGuardKind, HookProtocol, ProjectManifest, evaluate_hook_json};
use la_runtime::{audit_github_repo, load_workspace, probe_git_hook_context};

use crate::cli::{GithubCommand, HookCommand, OutputFormat};
use crate::commands::CommandOutcome;

pub(super) fn cmd_hook(
    root: &Path,
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
            let manifest = load_manifest_or_default(root);
            let ctx = probe_git_hook_context(root);
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
            let payload = match build_test_payload(
                root,
                input.as_deref(),
                command.as_deref(),
                path.as_deref(),
                content.as_deref(),
            ) {
                Ok(p) => p,
                Err(msg) => return CommandOutcome::RuntimeError(msg),
            };
            let manifest = load_manifest_or_default(root);
            let ctx = probe_git_hook_context(root);
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
