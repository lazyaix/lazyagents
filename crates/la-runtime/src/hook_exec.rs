//! Shell-free, timeout-enforced subprocess execution for `PostToolUse` `auto-format`
//! [`FormatCommandPlan`]s emitted by `la-core`.

use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use la_core::{FormatCommandPlan, redact_ghs_tokens};

use crate::error::RuntimeError;

const MAX_DIAGNOSTIC_BYTES: usize = 2_048;
const POLL_INTERVAL: Duration = Duration::from_millis(5);

/// Outcome of executing a [`FormatCommandPlan`] inside a workspace root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormatExecOutcome {
    /// Matched `[engineering.format.rules]` identifier.
    pub rule_id: String,
    /// `true` when the formatter exited with status `0` or was safely skipped (missing target file).
    pub succeeded: bool,
    /// Process exit status code when available.
    pub exit_code: Option<i32>,
    /// `true` if the formatter exceeded `plan.timeout_ms` and was terminated.
    pub timed_out: bool,
    /// Concise redacted stderr/stdout diagnostic on failure or timeout (`None` on success).
    pub diagnostic: Option<String>,
}

/// Executes a pure [`FormatCommandPlan`] directly via [`Command`] (never invoking a shell) with
/// strict workspace path containment and bounded timeout enforcement.
///
/// # Errors
///
/// Returns [`RuntimeError::HookExec`] when `plan.program` is empty, `plan.target_path` attempts
/// path traversal outside `workspace_root`, or polling the child process fails, and
/// [`RuntimeError::Io`] when `workspace_root` cannot be canonicalized.
pub fn execute_format_plan(
    workspace_root: &Path,
    plan: &FormatCommandPlan,
) -> Result<FormatExecOutcome, RuntimeError> {
    if plan.program.trim().is_empty() {
        return Err(RuntimeError::hook_exec(
            &plan.rule_id,
            "formatter program must not be empty",
        ));
    }

    validate_relative_target(&plan.rule_id, &plan.target_path)?;

    let canonical_root = fs::canonicalize(workspace_root)
        .map_err(|err| RuntimeError::io(workspace_root.display().to_string(), &err))?;
    let candidate = canonical_root.join(&plan.target_path);

    let canonical_target = match fs::canonicalize(&candidate) {
        Ok(path) => path,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(FormatExecOutcome {
                rule_id: plan.rule_id.clone(),
                succeeded: true,
                exit_code: None,
                timed_out: false,
                diagnostic: None,
            });
        }
        Err(err) => {
            return Err(RuntimeError::io(candidate.display().to_string(), &err));
        }
    };

    if !canonical_target.starts_with(&canonical_root) {
        return Err(RuntimeError::hook_exec(
            &plan.rule_id,
            format!(
                "target path `{}` resolves outside workspace root",
                plan.target_path
            ),
        ));
    }

    let mut child = match Command::new(&plan.program)
        .args(&plan.args)
        .current_dir(&canonical_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            return Ok(FormatExecOutcome {
                rule_id: plan.rule_id.clone(),
                succeeded: false,
                exit_code: None,
                timed_out: false,
                diagnostic: Some(format!(
                    "auto-format rule `{}` failed to launch `{}`: {err}",
                    plan.rule_id, plan.program
                )),
            });
        }
    };

    let stdout_reader = child
        .stdout
        .take()
        .map(|pipe| thread::spawn(move || read_bounded_output(pipe, MAX_DIAGNOSTIC_BYTES)));
    let stderr_reader = child
        .stderr
        .take()
        .map(|pipe| thread::spawn(move || read_bounded_output(pipe, MAX_DIAGNOSTIC_BYTES)));

    let (status, timed_out) = wait_with_timeout(&mut child, plan)?;

    let stdout_text = stdout_reader
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default();
    let stderr_text = stderr_reader
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default();

    Ok(build_exec_outcome(
        plan,
        status,
        timed_out,
        &stderr_text,
        &stdout_text,
    ))
}

fn wait_with_timeout(
    child: &mut Child,
    plan: &FormatCommandPlan,
) -> Result<(Option<ExitStatus>, bool), RuntimeError> {
    let timeout = Duration::from_millis(u64::from(plan.timeout_ms.max(1)));
    let started = Instant::now();
    loop {
        if let Some(exit_status) = child.try_wait().map_err(|err| {
            RuntimeError::hook_exec(&plan.rule_id, format!("failed polling formatter: {err}"))
        })? {
            return Ok((Some(exit_status), false));
        }
        let elapsed = started.elapsed();
        if elapsed >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Ok((None, true));
        }
        let remaining = timeout.saturating_sub(elapsed);
        thread::sleep(POLL_INTERVAL.min(remaining));
    }
}

fn build_exec_outcome(
    plan: &FormatCommandPlan,
    status: Option<ExitStatus>,
    timed_out: bool,
    stderr_text: &str,
    stdout_text: &str,
) -> FormatExecOutcome {
    if timed_out {
        let excerpt = concise_output_excerpt(stderr_text, stdout_text);
        let detail = if excerpt.is_empty() {
            format!(
                "auto-format rule `{}` (`{}`) timed out after {}ms",
                plan.rule_id, plan.program, plan.timeout_ms
            )
        } else {
            format!(
                "auto-format rule `{}` (`{}`) timed out after {}ms: {excerpt}",
                plan.rule_id, plan.program, plan.timeout_ms
            )
        };
        return FormatExecOutcome {
            rule_id: plan.rule_id.clone(),
            succeeded: false,
            exit_code: None,
            timed_out: true,
            diagnostic: Some(detail),
        };
    }

    if let Some(exit_status) = status
        && exit_status.success()
    {
        return FormatExecOutcome {
            rule_id: plan.rule_id.clone(),
            succeeded: true,
            exit_code: exit_status.code(),
            timed_out: false,
            diagnostic: None,
        };
    }

    let exit_code = status.and_then(|s| s.code());
    let excerpt = concise_output_excerpt(stderr_text, stdout_text);
    let code_label = exit_code.map_or_else(|| "signal".to_owned(), |c| c.to_string());
    let diagnostic = if excerpt.is_empty() {
        format!(
            "auto-format rule `{}` (`{}`) failed with exit status {code_label}",
            plan.rule_id, plan.program
        )
    } else {
        format!(
            "auto-format rule `{}` (`{}`) failed (exit {code_label}): {excerpt}",
            plan.rule_id, plan.program
        )
    };

    FormatExecOutcome {
        rule_id: plan.rule_id.clone(),
        succeeded: false,
        exit_code,
        timed_out: false,
        diagnostic: Some(diagnostic),
    }
}

fn validate_relative_target(rule_id: &str, target_path: &str) -> Result<(), RuntimeError> {
    if target_path.trim().is_empty() {
        return Err(RuntimeError::hook_exec(
            rule_id,
            "target path must not be empty",
        ));
    }
    let path = Path::new(target_path);
    if path.is_absolute() {
        return Err(RuntimeError::hook_exec(
            rule_id,
            format!("target path `{target_path}` must be workspace-relative"),
        ));
    }
    for component in path.components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(RuntimeError::hook_exec(
                    rule_id,
                    format!("target path `{target_path}` escapes workspace root"),
                ));
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    Ok(())
}

fn read_bounded_output(mut reader: impl Read, max_bytes: usize) -> String {
    let mut kept = Vec::new();
    let mut chunk = [0_u8; 1_024];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let remaining = max_bytes.saturating_sub(kept.len());
                if remaining > 0
                    && let Some(slice) = chunk.get(..n.min(remaining))
                {
                    kept.extend_from_slice(slice);
                }
            }
        }
    }
    String::from_utf8_lossy(&kept).into_owned()
}

fn concise_output_excerpt(stderr: &str, stdout: &str) -> String {
    let primary = if stderr.trim().is_empty() {
        stdout.trim()
    } else {
        stderr.trim()
    };
    if primary.is_empty() {
        return String::new();
    }
    let redacted = redact_ghs_tokens(primary);
    let compact: Vec<&str> = redacted.lines().take(4).map(str::trim).collect();
    compact.join(" | ")
}
