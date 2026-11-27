//! Pure Sans-I/O guardrail policy evaluators for all 8 built-in hooks:
//! `git-guard`, `protected-paths`, `secret-guard`, `verify-gate`,
//! `shell-guard`, `compact-guard`, `session-context`, and `auto-format`.

use globset::{Glob, GlobSetBuilder};
use regex::Regex;

use crate::engineering::{FormatPolicy, ProtectedPathsPolicy, ShellGuardPolicy, StopHookMode};
use crate::error::CoreError;
use crate::hook_eval::{FormatCommandPlan, HookEvalContext, PreToolUseOutput, StopHookOutput};
use crate::id::PolicyDecision;
use crate::manifest::{LintLevel, ProjectManifest};
use crate::secret::scan_secrets;

/// Evaluates `git-guard` policy against a shell command line.
#[must_use]
pub fn evaluate_git_guard(
    command_line: &str,
    manifest: &ProjectManifest,
    ctx: &HookEvalContext,
) -> PreToolUseOutput {
    let git = &manifest.engineering.git;
    let guardrails = &git.guardrails;
    let default_branch = git.default_branch.as_str();

    let mut worst_decision = PolicyDecision::Allow;
    let mut reasons = Vec::new();

    for segment in split_shell_segments(command_line) {
        let tokens: Vec<&str> = segment.split_whitespace().collect();
        let Some(git_idx) = tokens.iter().position(|tok| *tok == "git") else {
            continue;
        };
        let git_args = tokens.get(git_idx.saturating_add(1)..).unwrap_or_default();
        let (subcmd, rest) = extract_git_subcommand(git_args);

        match subcmd {
            Some("push") => {
                if has_force_push_flag(rest) && guardrails.block_force_push != PolicyDecision::Allow
                {
                    worst_decision = worst_decision.stricter(guardrails.block_force_push);
                    reasons.push(
                        "force push (`--force`/`-f`) is blocked by [engineering.git.guardrails]"
                            .to_owned(),
                    );
                }
                if pushes_to_default_branch(rest, default_branch, ctx.current_branch.as_deref())
                    && guardrails.block_push_default_branch != PolicyDecision::Allow
                {
                    worst_decision = worst_decision.stricter(guardrails.block_push_default_branch);
                    reasons.push(format!(
                        "direct push to protected branch `{default_branch}` is blocked; push a short-lived branch and open a PR"
                    ));
                }
            }
            Some("reset") => {
                if rest.contains(&"--hard")
                    && guardrails.block_destructive_reset != PolicyDecision::Allow
                {
                    worst_decision = worst_decision.stricter(guardrails.block_destructive_reset);
                    reasons.push(
                        "`git reset --hard` is blocked by [engineering.git.guardrails]".to_owned(),
                    );
                }
            }
            Some("clean") => {
                if has_short_flag(rest, 'f')
                    && guardrails.block_destructive_reset != PolicyDecision::Allow
                {
                    worst_decision = worst_decision.stricter(guardrails.block_destructive_reset);
                    reasons.push(
                        "destructive `git clean -f` is blocked by [engineering.git.guardrails]"
                            .to_owned(),
                    );
                }
            }
            Some("add")
                if is_broad_git_add(rest)
                    && !ctx.untracked_sensitive_files.is_empty()
                    && guardrails.guard_broad_staging != PolicyDecision::Allow =>
            {
                worst_decision = worst_decision.stricter(guardrails.guard_broad_staging);
                reasons.push(format!(
                    "broad `git add` detected while sensitive files exist in working tree: {}",
                    ctx.untracked_sensitive_files.join(", ")
                ));
            }
            _ => {}
        }
    }

    if worst_decision == PolicyDecision::Allow {
        PreToolUseOutput::allow()
    } else {
        PreToolUseOutput::with_decision(worst_decision, reasons.join("; "))
    }
}

/// Evaluates `protected-paths` policy against a target file path.
///
/// # Errors
///
/// Returns [`CoreError::HookEval`] if any configured glob pattern in `[engineering.protected_paths]`
/// is syntactically invalid.
pub fn evaluate_protected_paths(
    target_path: &str,
    workspace_root: Option<&str>,
    policy: &ProtectedPathsPolicy,
) -> Result<PreToolUseOutput, CoreError> {
    if target_path.trim().is_empty() || policy.entries.is_empty() {
        return Ok(PreToolUseOutput::allow());
    }
    let rel_path = normalize_relative_path(target_path, workspace_root);
    let mut builder = GlobSetBuilder::new();
    for entry in &policy.entries {
        let glob = Glob::new(&entry.glob).map_err(|err| CoreError::HookEval {
            message: format!("invalid protected path glob `{}`: {err}", entry.glob),
        })?;
        builder.add(glob);
    }
    let set = builder.build().map_err(|err| CoreError::HookEval {
        message: format!("failed to build protected paths matcher: {err}"),
    })?;

    let matches = set.matches(&rel_path);
    if matches.is_empty() {
        return Ok(PreToolUseOutput::allow());
    }

    let mut worst = PolicyDecision::Allow;
    let mut owners = Vec::new();
    for idx in matches {
        if let Some(entry) = policy.entries.get(idx) {
            let decision = entry.decision.unwrap_or(policy.default_decision);
            worst = worst.stricter(decision);
            for owner in &entry.owners {
                if !owners.contains(owner) {
                    owners.push(owner.clone());
                }
            }
        }
    }

    let owner_note = if owners.is_empty() {
        String::new()
    } else {
        format!(" (owners: {})", owners.join(", "))
    };
    Ok(PreToolUseOutput::with_decision(
        worst,
        format!("`{rel_path}` is protected by [engineering.protected_paths]{owner_note}"),
    ))
}

/// Evaluates `secret-guard` policy against outgoing file content.
#[must_use]
pub fn evaluate_secret_guard(content: &str, severity: LintLevel) -> PreToolUseOutput {
    if severity == LintLevel::Off || content.is_empty() {
        return PreToolUseOutput::allow();
    }
    let findings = scan_secrets(content);
    if findings.is_empty() {
        return PreToolUseOutput::allow();
    }
    let decision = match severity {
        LintLevel::Error => PolicyDecision::Deny,
        LintLevel::Warn => PolicyDecision::Ask,
        LintLevel::Off => PolicyDecision::Allow,
    };
    let summary: Vec<String> = findings
        .iter()
        .map(|f| format!("{:?} at line {} ({})", f.kind, f.line, f.preview))
        .collect();
    PreToolUseOutput::with_decision(
        decision,
        format!(
            "plaintext secret detected ({}); use `${{VAR}}` placeholders instead",
            summary.join(", ")
        ),
    )
}

/// Evaluates `shell-guard` policy against a shell command line.
///
/// # Errors
///
/// Returns [`CoreError::HookEval`] if any regex in `[engineering.shell_guard.extra_deny_patterns]`
/// fails to compile.
pub fn evaluate_shell_guard(
    command_line: &str,
    policy: &ShellGuardPolicy,
) -> Result<PreToolUseOutput, CoreError> {
    if !policy.is_enabled() || command_line.trim().is_empty() {
        return Ok(PreToolUseOutput::allow());
    }

    let mut worst = PolicyDecision::Allow;
    let mut reasons = Vec::new();
    let lower = command_line.to_ascii_lowercase();

    if policy.block_pipe_to_shell != PolicyDecision::Allow && is_remote_pipe_to_shell(&lower) {
        worst = worst.stricter(policy.block_pipe_to_shell);
        reasons.push(
            "piping remote downloads (`curl`/`wget`) directly into a shell is blocked by [engineering.shell_guard]"
                .to_owned(),
        );
    }

    for segment in split_shell_segments(command_line) {
        let tokens: Vec<&str> = segment.split_whitespace().collect();
        if policy.block_destructive_rm != PolicyDecision::Allow && is_destructive_fs_cmd(&tokens) {
            worst = worst.stricter(policy.block_destructive_rm);
            reasons.push(
                "destructive filesystem command (`rm -rf` root/home or `chmod 777`) is blocked by [engineering.shell_guard]"
                    .to_owned(),
            );
        }
        if policy.block_credential_reads != PolicyDecision::Allow
            && reads_sensitive_credential(segment, &tokens)
        {
            worst = worst.stricter(policy.block_credential_reads);
            reasons.push(
                "reading local credential material (`~/.ssh`, `~/.aws`, `~/.gnupg`, `.env`) is blocked by [engineering.shell_guard]"
                    .to_owned(),
            );
        }
    }

    for pattern in &policy.extra_deny_patterns {
        let re = Regex::new(pattern).map_err(|err| CoreError::HookEval {
            message: format!("invalid shell_guard regex `{pattern}`: {err}"),
        })?;
        if re.is_match(command_line) {
            worst = worst.stricter(PolicyDecision::Deny);
            reasons.push(format!(
                "command matched [engineering.shell_guard] deny pattern `{pattern}`"
            ));
        }
    }

    if worst == PolicyDecision::Allow {
        Ok(PreToolUseOutput::allow())
    } else {
        Ok(PreToolUseOutput::with_decision(worst, reasons.join("; ")))
    }
}

/// Evaluates `verify-gate` on `Stop` lifecycle events.
#[must_use]
pub(super) fn evaluate_verify_gate(
    termination_reason: Option<&str>,
    continuation_active: bool,
    manifest: &ProjectManifest,
    ctx: &HookEvalContext,
) -> StopHookOutput {
    let verify = &manifest.engineering.verify;
    let normal_stop = termination_reason.is_none_or(|reason| reason == "NO_TOOL_CALL");
    let required_cmds: Vec<&str> = verify
        .commands
        .iter()
        .filter(|c| c.required)
        .map(|c| c.cmd.as_str())
        .collect();

    if verify.stop_hook_mode == StopHookMode::Off
        || continuation_active
        || !normal_stop
        || !ctx.has_modified_tracked_files
        || required_cmds.is_empty()
    {
        return StopHookOutput {
            decision: "allow".to_owned(),
            reason: None,
        };
    }

    let prefix = match verify.stop_hook_mode {
        StopHookMode::Remind => "Reminder: confirm required local verification passed",
        StopHookMode::Enforce => "Required local verification must pass before completing",
        StopHookMode::Off => "",
    };
    StopHookOutput {
        decision: "continue".to_owned(),
        reason: Some(format!("{prefix}: {}", required_cmds.join(" && "))),
    }
}

/// Evaluates `compact-guard` on `PreCompact` lifecycle events, reminding the agent to persist
/// durable task state before context compaction drops intermediate details.
#[must_use]
pub fn evaluate_compact_guard(manifest: &ProjectManifest, ctx: &HookEvalContext) -> String {
    let branch = ctx.current_branch.as_deref().unwrap_or("unknown");
    let dirty_note = if ctx.has_modified_tracked_files {
        " Working tree has uncommitted tracked changes."
    } else {
        ""
    };
    format!(
        "PreCompact checkpoint ({}, branch `{branch}`): persist active plan, key decisions, modified files, and remaining tasks to a durable workspace artifact (e.g., `progress.md` or `plan.md`) before compaction.{dirty_note}",
        manifest.project.name
    )
}

/// Evaluates `session-context` on `SessionStart` lifecycle events, producing a compact cold-start
/// status summary without mutating static system-prompt prefixes.
#[must_use]
pub fn evaluate_session_context(manifest: &ProjectManifest, ctx: &HookEvalContext) -> String {
    let default_branch = &manifest.engineering.git.default_branch;
    let mut parts = Vec::new();
    if let Some(branch) = &ctx.current_branch {
        if branch == default_branch {
            parts.push(format!(
                "branch=`{branch}` (WARNING: protected default branch; create a short-lived topic branch before committing)"
            ));
        } else {
            parts.push(format!("branch=`{branch}`"));
        }
    }
    if ctx.has_modified_tracked_files {
        parts.push("working_tree=dirty".to_owned());
    } else {
        parts.push("working_tree=clean".to_owned());
    }
    if !ctx.untracked_sensitive_files.is_empty() {
        parts.push(format!(
            "sensitive_untracked=[{}]",
            ctx.untracked_sensitive_files.join(", ")
        ));
    }
    format!(
        "Session context ({}): {}",
        manifest.project.name,
        parts.join("; ")
    )
}

/// Matches an edited path against `[engineering.format]` and builds a deterministic,
/// shell-free [`FormatCommandPlan`] for execution by `la-runtime::hook_exec`.
///
/// # Errors
///
/// Returns [`CoreError::HookEval`] if a configured formatter glob is invalid.
pub fn plan_auto_format(
    target_path: &str,
    workspace_root: Option<&str>,
    policy: &FormatPolicy,
) -> Result<Option<FormatCommandPlan>, CoreError> {
    if !policy.enabled || target_path.trim().is_empty() || policy.rules.is_empty() {
        return Ok(None);
    }
    let rel_path = normalize_relative_path(target_path, workspace_root);
    for rule in &policy.rules {
        let matcher = Glob::new(&rule.glob)
            .map_err(|err| CoreError::HookEval {
                message: format!("invalid format rule glob `{}`: {err}", rule.glob),
            })?
            .compile_matcher();
        if !matcher.is_match(&rel_path) {
            continue;
        }
        let Some((program, raw_args)) = rule.cmd.split_first() else {
            continue;
        };
        let mut args = Vec::with_capacity(raw_args.len().saturating_add(1));
        let mut substituted = false;
        for arg in raw_args {
            if arg.contains("{path}") {
                args.push(arg.replace("{path}", &rel_path));
                substituted = true;
            } else {
                args.push(arg.clone());
            }
        }
        if !substituted {
            args.push(rel_path.clone());
        }
        return Ok(Some(FormatCommandPlan {
            rule_id: rule.id.as_str().to_owned(),
            program: program.clone(),
            args,
            target_path: rel_path,
            timeout_ms: rule.timeout_ms,
        }));
    }
    Ok(None)
}

fn is_remote_pipe_to_shell(lower: &str) -> bool {
    let has_fetcher = lower.contains("curl ") || lower.contains("wget ");
    if !has_fetcher {
        return false;
    }
    let mut segments = lower.split('|');
    let _first = segments.next();
    segments.any(|seg| {
        let mut words = seg.split_whitespace();
        let first_word = match words.next() {
            Some("sudo" | "env") => words.next(),
            other => other,
        };
        matches!(
            first_word,
            Some("sh" | "bash" | "zsh" | "dash" | "ksh" | "/bin/sh" | "/bin/bash")
        )
    })
}

fn is_destructive_fs_cmd(tokens: &[&str]) -> bool {
    let start = usize::from(matches!(tokens.first(), Some(&"sudo")));
    let cmd = tokens.get(start).copied();
    let rest = tokens.get(start.saturating_add(1)..).unwrap_or_default();

    match cmd {
        Some("rm") => {
            let recursive = has_short_flag(rest, 'r')
                || has_short_flag(rest, 'R')
                || rest.contains(&"--recursive");
            let forced = has_short_flag(rest, 'f') || rest.contains(&"--force");
            let dangerous_target = rest.iter().any(|tok| {
                !tok.starts_with('-')
                    && matches!(
                        *tok,
                        "/" | "/*" | "~" | "~/" | "$HOME" | "${HOME}" | "." | ".."
                    )
            });
            recursive && forced && dangerous_target
        }
        Some("chmod") => {
            let world_writable = rest
                .iter()
                .any(|tok| matches!(*tok, "777" | "0777" | "a+rwx"));
            let wide_target = rest
                .iter()
                .any(|tok| matches!(*tok, "/" | "~" | "$HOME" | ".") || has_short_flag(rest, 'R'));
            world_writable && wide_target
        }
        _ => false,
    }
}

fn reads_sensitive_credential(segment: &str, tokens: &[&str]) -> bool {
    let lower = segment.to_ascii_lowercase();
    let sensitive_paths = [
        "~/.ssh",
        "$home/.ssh",
        "${home}/.ssh",
        "/.ssh/id_",
        "~/.aws/credentials",
        "$home/.aws/credentials",
        "~/.gnupg",
        "~/.netrc",
    ];
    if sensitive_paths.iter().any(|needle| lower.contains(needle)) {
        return true;
    }

    let reader_cmd = matches!(
        tokens.first().copied(),
        Some("cat" | "less" | "head" | "tail" | "sed" | "awk" | "grep" | "base64" | "cp" | "scp")
    );
    reader_cmd
        && tokens.iter().skip(1).any(|tok| {
            let base = tok.rsplit('/').next().unwrap_or(tok);
            base == ".env" || base.starts_with(".env.")
        })
}

fn split_shell_segments(command_line: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut start = 0usize;
    let bytes = command_line.as_bytes();
    let mut idx = 0usize;

    while idx < bytes.len() {
        let is_sep = match bytes.get(idx) {
            Some(b';' | b'\n') => Some(1),
            Some(b'&') if bytes.get(idx.saturating_add(1)) == Some(&b'&') => Some(2),
            Some(b'|') => {
                if bytes.get(idx.saturating_add(1)) == Some(&b'|') {
                    Some(2)
                } else {
                    Some(1)
                }
            }
            _ => None,
        };
        if let Some(width) = is_sep {
            if let Some(seg) = command_line.get(start..idx)
                && !seg.trim().is_empty()
            {
                segments.push(seg.trim());
            }
            idx = idx.saturating_add(width);
            start = idx;
        } else {
            idx = idx.saturating_add(1);
        }
    }
    if let Some(tail) = command_line.get(start..)
        && !tail.trim().is_empty()
    {
        segments.push(tail.trim());
    }
    segments
}

fn extract_git_subcommand<'a>(args: &'a [&'a str]) -> (Option<&'a str>, &'a [&'a str]) {
    let mut idx = 0usize;
    while let Some(&tok) = args.get(idx) {
        if tok == "-C" || tok == "-c" || tok == "--git-dir" || tok == "--work-tree" {
            idx = idx.saturating_add(2);
            continue;
        }
        if tok.starts_with('-') {
            idx = idx.saturating_add(1);
            continue;
        }
        return (
            Some(tok),
            args.get(idx.saturating_add(1)..).unwrap_or_default(),
        );
    }
    (None, &[])
}

fn has_force_push_flag(args: &[&str]) -> bool {
    args.iter().any(|tok| {
        *tok == "--force"
            || *tok == "-f"
            || (*tok).starts_with('-') && !tok.starts_with("--") && tok.contains('f')
    })
}

fn has_short_flag(args: &[&str], flag: char) -> bool {
    args.iter().any(|tok| {
        tok.starts_with('-') && !tok.starts_with("--") && tok.chars().skip(1).any(|c| c == flag)
    })
}

fn pushes_to_default_branch(
    args: &[&str],
    default_branch: &str,
    current_branch: Option<&str>,
) -> bool {
    let positional: Vec<&str> = args
        .iter()
        .copied()
        .filter(|tok| !tok.starts_with('-'))
        .collect();
    if positional.len() <= 1 {
        return current_branch == Some(default_branch);
    }
    positional.iter().skip(1).any(|refspec| {
        let dest = refspec.split(':').next_back().unwrap_or(refspec);
        let stripped = dest.strip_prefix("refs/heads/").unwrap_or(dest);
        stripped == default_branch
    })
}

fn is_broad_git_add(args: &[&str]) -> bool {
    args.iter()
        .any(|tok| matches!(*tok, "." | "-A" | "--all" | "*"))
}

fn normalize_relative_path(raw_path: &str, workspace_root: Option<&str>) -> String {
    let posix = raw_path.replace('\\', "/");
    if let Some(root) = workspace_root {
        let root_posix = root.replace('\\', "/");
        let root_trimmed = root_posix.trim_end_matches('/');
        if let Some(stripped) = posix.strip_prefix(root_trimmed) {
            return stripped.trim_start_matches('/').to_owned();
        }
    }
    posix.trim_start_matches("./").to_owned()
}
