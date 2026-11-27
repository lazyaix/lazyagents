//! Pure-Rust built-in hook policy evaluator (`lazyagents hook eval <id>`) speaking
//! Antigravity's camelCase JSON stdin/stdout contract with zero external shell/jq dependencies.

use core::fmt;

use globset::{Glob, GlobSetBuilder};
use serde::{Deserialize, Serialize};

use crate::engineering::{ProtectedPathsPolicy, StopHookMode};
use crate::error::CoreError;
use crate::id::PolicyDecision;
use crate::manifest::{LintLevel, ProjectManifest};
use crate::secret::scan_secrets;

/// Built-in engineering guardrail evaluated by `lazyagents hook eval <id>`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookGuardKind {
    /// Blocks destructive Git commands (`push` to default branch, `--force`, `reset --hard`, `clean -f`, broad `git add`).
    GitGuard,
    /// Intercepts edits targeting globs declared in `[engineering.protected_paths]`.
    ProtectedPaths,
    /// Blocks writing plaintext credentials (stateless `ghs_*`, PATs, API keys, literal `Bearer`).
    SecretGuard,
    /// Reminds or asks the agent to run `[engineering.verify]` commands before stopping.
    VerifyGate,
}

/// Wire protocol spoken on stdin/stdout by the hook evaluator.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookProtocol {
    /// Google Antigravity camelCase JSON stdin/stdout contract (V1 active).
    #[default]
    Antigravity,
    /// Reserved Anthropic Claude Code hook protocol.
    ClaudeCode,
    /// Reserved `OpenAI` Codex hook protocol.
    Codex,
}

/// Optional workspace runtime facts supplied by `la-runtime` to enrich hook evaluation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HookEvalContext {
    /// Workspace root path used to normalize absolute target file paths to repo-relative globs.
    pub workspace_root: Option<String>,
    /// Current Git branch checked out in the workspace (e.g., `"main"` or `"feat/x"`).
    pub current_branch: Option<String>,
    /// Untracked or modified sensitive files (e.g., `.env`) present in the working tree.
    pub untracked_sensitive_files: Vec<String>,
    /// Whether tracked files in the repository have uncommitted modifications.
    pub has_modified_tracked_files: bool,
}

/// Antigravity `PreToolUse` JSON stdout decision payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PreToolUseOutput {
    /// Wire decision string (`"allow"`, `"ask"`, `"force_ask"`, `"deny"`).
    pub decision: String,
    /// Optional human- and agent-readable explanation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Antigravity `Stop` hook JSON stdout decision payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StopHookOutput {
    /// `"continue"` blocks termination and re-enters the loop; `"allow"` permits stopping.
    pub decision: String,
    /// Optional system-message reason injected when continuing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AntigravityPreToolUseInput {
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    workspace_paths: Vec<String>,
    #[serde(default)]
    tool_call: Option<AntigravityToolCall>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AntigravityToolCall {
    #[serde(default)]
    args: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AntigravityStopInput {
    #[serde(default)]
    termination_reason: Option<String>,
    #[serde(default)]
    continuation_active: bool,
}

impl HookGuardKind {
    /// Parses a CLI guard ID (`"git-guard"`, `"protected-paths"`, `"secret-guard"`, `"verify-gate"`).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::HookEval`] if `value` does not match a known built-in guard.
    pub fn parse(value: &str) -> Result<Self, CoreError> {
        match value.trim() {
            "git-guard" => Ok(Self::GitGuard),
            "protected-paths" => Ok(Self::ProtectedPaths),
            "secret-guard" => Ok(Self::SecretGuard),
            "verify-gate" => Ok(Self::VerifyGate),
            other => Err(CoreError::HookEval {
                message: format!(
                    "unknown built-in hook guard `{other}`; expected one of: git-guard, protected-paths, secret-guard, verify-gate"
                ),
            }),
        }
    }

    /// Returns the canonical kebab-case guard identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GitGuard => "git-guard",
            Self::ProtectedPaths => "protected-paths",
            Self::SecretGuard => "secret-guard",
            Self::VerifyGate => "verify-gate",
        }
    }
}

impl fmt::Display for HookGuardKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PreToolUseOutput {
    /// Constructs an `"allow"` decision with no reason.
    #[must_use]
    pub fn allow() -> Self {
        Self {
            decision: PolicyDecision::Allow.as_wire_str().to_owned(),
            reason: None,
        }
    }

    /// Constructs a policy decision paired with an explanatory reason.
    #[must_use]
    pub fn with_decision(decision: PolicyDecision, reason: impl Into<String>) -> Self {
        Self {
            decision: decision.as_wire_str().to_owned(),
            reason: if decision == PolicyDecision::Allow {
                None
            } else {
                Some(reason.into())
            },
        }
    }
}

/// Evaluates a built-in guardrail hook against a raw JSON `stdin` payload and returns
/// the serialized JSON `stdout` response.
///
/// # Errors
///
/// Returns [`CoreError::HookEval`] if the protocol is not yet active, `stdin_json` is
/// malformed JSON, or a configured protected-path glob is invalid.
pub fn evaluate_hook_json(
    guard: HookGuardKind,
    protocol: HookProtocol,
    stdin_json: &str,
    manifest: &ProjectManifest,
    ctx: &HookEvalContext,
) -> Result<String, CoreError> {
    if protocol != HookProtocol::Antigravity {
        return Err(CoreError::HookEval {
            message: format!("hook protocol `{protocol:?}` is reserved for M4; use `antigravity`"),
        });
    }

    let value = match guard {
        HookGuardKind::GitGuard => {
            let input: AntigravityPreToolUseInput = parse_stdin(stdin_json)?;
            let cmd = extract_command_line(&input);
            let out = evaluate_git_guard(cmd.as_deref().unwrap_or_default(), manifest, ctx);
            serde_json::to_value(out)
        }
        HookGuardKind::ProtectedPaths => {
            let input: AntigravityPreToolUseInput = parse_stdin(stdin_json)?;
            let path = extract_target_path(&input);
            let root = ctx
                .workspace_root
                .as_deref()
                .or(input.cwd.as_deref())
                .or_else(|| input.workspace_paths.first().map(String::as_str));
            let out = evaluate_protected_paths(
                path.as_deref().unwrap_or_default(),
                root,
                &manifest.engineering.protected_paths,
            )?;
            serde_json::to_value(out)
        }
        HookGuardKind::SecretGuard => {
            let input: AntigravityPreToolUseInput = parse_stdin(stdin_json)?;
            let content = extract_written_content(&input);
            let out = evaluate_secret_guard(
                content.as_deref().unwrap_or_default(),
                manifest.lint.secrets,
            );
            serde_json::to_value(out)
        }
        HookGuardKind::VerifyGate => {
            let input: AntigravityStopInput = parse_stdin(stdin_json)?;
            let out = evaluate_verify_gate(&input, manifest, ctx);
            serde_json::to_value(out)
        }
    };

    let serialized = value.map_err(|err| CoreError::HookEval {
        message: err.to_string(),
    })?;
    serde_json::to_string(&serialized).map_err(|err| CoreError::HookEval {
        message: err.to_string(),
    })
}

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

fn evaluate_verify_gate(
    input: &AntigravityStopInput,
    manifest: &ProjectManifest,
    ctx: &HookEvalContext,
) -> StopHookOutput {
    let verify = &manifest.engineering.verify;
    let normal_stop = input
        .termination_reason
        .as_deref()
        .is_none_or(|reason| reason == "NO_TOOL_CALL");
    let required_cmds: Vec<&str> = verify
        .commands
        .iter()
        .filter(|c| c.required)
        .map(|c| c.cmd.as_str())
        .collect();

    if verify.stop_hook_mode == StopHookMode::Off
        || input.continuation_active
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

fn parse_stdin<T: for<'de> Deserialize<'de>>(stdin_json: &str) -> Result<T, CoreError> {
    serde_json::from_str(stdin_json).map_err(|err| CoreError::HookEval {
        message: format!("invalid hook stdin JSON: {err}"),
    })
}

fn extract_command_line(input: &AntigravityPreToolUseInput) -> Option<String> {
    let args = input.tool_call.as_ref()?.args.as_object()?;
    for key in ["CommandLine", "command_line", "command", "cmd"] {
        if let Some(val) = args.get(key).and_then(serde_json::Value::as_str) {
            return Some(val.to_owned());
        }
    }
    None
}

fn extract_target_path(input: &AntigravityPreToolUseInput) -> Option<String> {
    let args = input.tool_call.as_ref()?.args.as_object()?;
    for key in ["Path", "TargetFile", "path", "file_path"] {
        if let Some(val) = args.get(key).and_then(serde_json::Value::as_str) {
            return Some(val.to_owned());
        }
    }
    None
}

fn extract_written_content(input: &AntigravityPreToolUseInput) -> Option<String> {
    let args = input.tool_call.as_ref()?.args.as_object()?;
    for key in ["Content", "Replacement", "CodeContent", "content"] {
        if let Some(val) = args.get(key).and_then(serde_json::Value::as_str) {
            return Some(val.to_owned());
        }
    }
    None
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

#[cfg(test)]
mod tests {
    use crate::engineering::{ProtectedPathEntry, VerifyCommand};
    use crate::id::ResourceId;

    use super::*;

    #[test]
    fn git_guard_blocks_force_push_main_push_reset_hard_and_sensitive_staging() {
        let manifest = ProjectManifest::default();
        let ctx = HookEvalContext {
            current_branch: Some("main".to_owned()),
            untracked_sensitive_files: vec![".env".to_owned()],
            ..HookEvalContext::default()
        };

        assert_eq!(
            evaluate_git_guard("git push origin main", &manifest, &ctx).decision,
            "deny"
        );
        assert_eq!(
            evaluate_git_guard("git push -f origin feat/x", &manifest, &ctx).decision,
            "deny"
        );
        assert_eq!(
            evaluate_git_guard("git status && git reset --hard HEAD~1", &manifest, &ctx).decision,
            "deny"
        );
        assert_eq!(
            evaluate_git_guard("git clean -fd", &manifest, &ctx).decision,
            "deny"
        );
        assert_eq!(
            evaluate_git_guard("git add .", &manifest, &ctx).decision,
            "ask"
        );

        let feat_ctx = HookEvalContext {
            current_branch: Some("feat/ok".to_owned()),
            ..HookEvalContext::default()
        };
        assert_eq!(
            evaluate_git_guard("git push -u origin feat/ok", &manifest, &feat_ctx).decision,
            "allow"
        );
        assert_eq!(
            evaluate_git_guard(
                "git push --force-with-lease origin feat/ok",
                &manifest,
                &feat_ctx
            )
            .decision,
            "allow"
        );
    }

    #[test]
    fn protected_paths_and_secret_guard_and_verify_gate_work_over_json_wire() {
        let mut manifest = ProjectManifest::default();
        manifest
            .engineering
            .protected_paths
            .entries
            .push(ProtectedPathEntry {
                glob: ".github/**".to_owned(),
                owners: vec!["@sec".to_owned()],
                decision: Some(PolicyDecision::Deny),
            });
        manifest.engineering.verify.commands.push(VerifyCommand {
            id: ResourceId::parse("test").unwrap(),
            label: "Test".to_owned(),
            cmd: "cargo test".to_owned(),
            required: true,
        });

        let ctx = HookEvalContext {
            workspace_root: Some("/repo".to_owned()),
            has_modified_tracked_files: true,
            ..HookEvalContext::default()
        };

        let pp_in = r#"{"cwd":"/repo","toolCall":{"name":"edit_file","args":{"Path":"/repo/.github/workflows/ci.yml"}}}"#;
        let pp_out = evaluate_hook_json(
            HookGuardKind::ProtectedPaths,
            HookProtocol::Antigravity,
            pp_in,
            &manifest,
            &ctx,
        )
        .unwrap();
        assert!(pp_out.contains("\"decision\":\"deny\""));

        let ws_paths_in = r#"{"workspacePaths":["/repo"],"toolCall":{"name":"edit_file","args":{"Path":"/repo/.github/workflows/ci.yml"}}}"#;
        let ws_paths_out = evaluate_hook_json(
            HookGuardKind::ProtectedPaths,
            HookProtocol::Antigravity,
            ws_paths_in,
            &manifest,
            &HookEvalContext::default(),
        )
        .unwrap();
        assert!(ws_paths_out.contains("\"decision\":\"deny\""));

        let leaked = format!("ghs_{}.{}", "a".repeat(25), "b".repeat(25));
        let sec_in = format!(
            r#"{{"toolCall":{{"name":"write_file","args":{{"Content":"token={leaked}"}}}}}}"#
        );
        let sec_out = evaluate_hook_json(
            HookGuardKind::SecretGuard,
            HookProtocol::Antigravity,
            &sec_in,
            &manifest,
            &ctx,
        )
        .unwrap();
        assert!(sec_out.contains("\"decision\":\"deny\""));
        assert!(!sec_out.contains(&leaked));

        let stop_first = r#"{"terminationReason":"NO_TOOL_CALL","continuationActive":false}"#;
        let stop_out = evaluate_hook_json(
            HookGuardKind::VerifyGate,
            HookProtocol::Antigravity,
            stop_first,
            &manifest,
            &ctx,
        )
        .unwrap();
        assert!(stop_out.contains("\"decision\":\"continue\""));

        let stop_loop_guard = r#"{"terminationReason":"NO_TOOL_CALL","continuationActive":true}"#;
        let stop_done = evaluate_hook_json(
            HookGuardKind::VerifyGate,
            HookProtocol::Antigravity,
            stop_loop_guard,
            &manifest,
            &ctx,
        )
        .unwrap();
        assert!(stop_done.contains("\"decision\":\"allow\""));
    }
}
