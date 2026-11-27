//! Multi-protocol hook stdin parser and stdout/exit-code wire encoder supporting
//! Antigravity, Claude Code, Cursor, and GitHub Copilot hook contracts.

use serde_json::{Map, Value};

use crate::error::CoreError;
use crate::hook_eval::{
    FormatCommandPlan, HookActionOutcome, HookProtocol, PreToolUseOutput, StopHookOutput,
};

/// Protocol-agnostic normalized stdin payload extracted from any supported agent harness.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct NormalizedHookInput {
    pub cwd: Option<String>,
    pub workspace_paths: Vec<String>,
    pub command_line: Option<String>,
    pub target_path: Option<String>,
    pub written_content: Option<String>,
    pub termination_reason: Option<String>,
    pub continuation_active: bool,
}

/// Parses an arbitrary supported protocol's hook JSON `stdin` into [`NormalizedHookInput`].
///
/// Handles Antigravity `camelCase` (`toolCall.args`), Claude Code `snake_case` (`tool_input`),
/// Cursor (`command`, `file_path`, `workspace_roots`), and GitHub Copilot (`toolArgs` as object
/// or JSON-encoded string).
pub(super) fn parse_normalized_input(stdin_json: &str) -> Result<NormalizedHookInput, CoreError> {
    let val: Value = serde_json::from_str(stdin_json).map_err(|err| CoreError::HookEval {
        message: format!("invalid hook stdin JSON: {err}"),
    })?;
    let Some(obj) = val.as_object() else {
        return Ok(NormalizedHookInput::default());
    };

    let cwd = first_str(obj, &["cwd", "workingDirectory", "working_directory"]);
    let mut workspace_paths = extract_str_array(obj, "workspacePaths");
    for root in extract_str_array(obj, "workspace_roots") {
        if !workspace_paths.contains(&root) {
            workspace_paths.push(root);
        }
    }

    let tool_args = extract_tool_args_object(obj);
    let command_line = tool_args
        .as_ref()
        .and_then(|m| first_str(m, &["CommandLine", "command_line", "command", "cmd"]))
        .or_else(|| first_str(obj, &["CommandLine", "command_line", "command", "cmd"]));

    let target_path = tool_args
        .as_ref()
        .and_then(|m| {
            first_str(
                m,
                &[
                    "Path",
                    "TargetFile",
                    "path",
                    "file_path",
                    "filePath",
                    "target_file",
                ],
            )
        })
        .or_else(|| {
            first_str(
                obj,
                &["Path", "TargetFile", "path", "file_path", "filePath"],
            )
        });

    let written_content = tool_args
        .as_ref()
        .and_then(|m| {
            first_str(
                m,
                &[
                    "Content",
                    "Replacement",
                    "CodeContent",
                    "content",
                    "new_string",
                    "new_str",
                ],
            )
        })
        .or_else(|| first_str(obj, &["Content", "Replacement", "content", "new_string"]));

    let termination_reason = first_str(obj, &["terminationReason", "termination_reason", "status"]);
    let continuation_active = obj
        .get("continuationActive")
        .or_else(|| obj.get("stop_hook_active"))
        .or_else(|| obj.get("continuation_active"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Ok(NormalizedHookInput {
        cwd,
        workspace_paths,
        command_line,
        target_path,
        written_content,
        termination_reason,
        continuation_active,
    })
}

/// Encodes a `PreToolUse` decision into the requested target's stdout JSON, stderr, and exit code.
pub(super) fn encode_pre_tool_use(
    protocol: HookProtocol,
    output: &PreToolUseOutput,
) -> Result<HookActionOutcome, CoreError> {
    match protocol {
        HookProtocol::Antigravity => Ok(HookActionOutcome {
            stdout: to_json_compact(output)?,
            stderr: None,
            exit_code: 0,
            format_plan: None,
        }),
        HookProtocol::ClaudeCode => {
            let decision = match output.decision.as_str() {
                "deny" => "deny",
                "ask" | "force_ask" => "ask",
                _ => "allow",
            };
            let mut specific = Map::new();
            specific.insert(
                "hookEventName".to_owned(),
                Value::String("PreToolUse".to_owned()),
            );
            specific.insert(
                "permissionDecision".to_owned(),
                Value::String(decision.to_owned()),
            );
            if let Some(reason) = &output.reason {
                specific.insert(
                    "permissionDecisionReason".to_owned(),
                    Value::String(reason.clone()),
                );
            }
            let payload = serde_json::json!({ "hookSpecificOutput": Value::Object(specific) });
            let (exit_code, stderr) = if decision == "deny" {
                (2, output.reason.clone())
            } else {
                (0, None)
            };
            Ok(HookActionOutcome {
                stdout: to_json_compact(&payload)?,
                stderr,
                exit_code,
                format_plan: None,
            })
        }
        HookProtocol::Cursor => {
            let permission = match output.decision.as_str() {
                "deny" => "deny",
                "ask" | "force_ask" => "ask",
                _ => "allow",
            };
            let mut obj = Map::new();
            obj.insert(
                "permission".to_owned(),
                Value::String(permission.to_owned()),
            );
            if let Some(reason) = &output.reason {
                obj.insert("userMessage".to_owned(), Value::String(reason.clone()));
                obj.insert("agentMessage".to_owned(), Value::String(reason.clone()));
            }
            Ok(HookActionOutcome {
                stdout: to_json_compact(&Value::Object(obj))?,
                stderr: None,
                exit_code: 0,
                format_plan: None,
            })
        }
        HookProtocol::GitHubCopilot => {
            let decision = match output.decision.as_str() {
                "deny" => "deny",
                "ask" | "force_ask" => "ask",
                _ => "allow",
            };
            let mut obj = Map::new();
            obj.insert(
                "permissionDecision".to_owned(),
                Value::String(decision.to_owned()),
            );
            if let Some(reason) = &output.reason {
                obj.insert(
                    "permissionDecisionReason".to_owned(),
                    Value::String(reason.clone()),
                );
            }
            Ok(HookActionOutcome {
                stdout: to_json_compact(&Value::Object(obj))?,
                stderr: None,
                exit_code: 0,
                format_plan: None,
            })
        }
        HookProtocol::Codex => Err(codex_unsupported_error()),
    }
}

/// Encodes a `Stop` hook decision into the requested target's wire format.
pub(super) fn encode_stop(
    protocol: HookProtocol,
    output: &StopHookOutput,
) -> Result<HookActionOutcome, CoreError> {
    let blocking = output.decision == "continue";
    match protocol {
        HookProtocol::Antigravity | HookProtocol::GitHubCopilot => Ok(HookActionOutcome {
            stdout: to_json_compact(output)?,
            stderr: None,
            exit_code: 0,
            format_plan: None,
        }),
        HookProtocol::ClaudeCode => {
            if blocking {
                let payload = serde_json::json!({
                    "decision": "block",
                    "reason": output.reason.as_deref().unwrap_or("Verification required before stopping"),
                });
                Ok(HookActionOutcome {
                    stdout: to_json_compact(&payload)?,
                    stderr: output.reason.clone(),
                    exit_code: 2,
                    format_plan: None,
                })
            } else {
                let payload = serde_json::json!({ "decision": "approve" });
                Ok(HookActionOutcome {
                    stdout: to_json_compact(&payload)?,
                    stderr: None,
                    exit_code: 0,
                    format_plan: None,
                })
            }
        }
        HookProtocol::Cursor => {
            let payload = if blocking {
                serde_json::json!({
                    "followup_message": output.reason.as_deref().unwrap_or("Run required verification checks"),
                })
            } else {
                serde_json::json!({})
            };
            Ok(HookActionOutcome {
                stdout: to_json_compact(&payload)?,
                stderr: None,
                exit_code: 0,
                format_plan: None,
            })
        }
        HookProtocol::Codex => Err(codex_unsupported_error()),
    }
}

/// Encodes an advisory/context-injection hook (`PreCompact`, `SessionStart`, `PostToolUse`)
/// into the requested target's wire format.
pub(super) fn encode_context_event(
    protocol: HookProtocol,
    event_name: &'static str,
    context_message: Option<String>,
    format_plan: Option<FormatCommandPlan>,
) -> Result<HookActionOutcome, CoreError> {
    let stdout = match protocol {
        HookProtocol::Antigravity => {
            let mut obj = Map::new();
            obj.insert("decision".to_owned(), Value::String("allow".to_owned()));
            if let Some(msg) = context_message {
                obj.insert("reason".to_owned(), Value::String(msg));
            }
            to_json_compact(&Value::Object(obj))?
        }
        HookProtocol::ClaudeCode => {
            let mut specific = Map::new();
            specific.insert(
                "hookEventName".to_owned(),
                Value::String(event_name.to_owned()),
            );
            if let Some(msg) = context_message {
                specific.insert("additionalContext".to_owned(), Value::String(msg));
            }
            let payload = serde_json::json!({ "hookSpecificOutput": Value::Object(specific) });
            to_json_compact(&payload)?
        }
        HookProtocol::Cursor => {
            let mut obj = Map::new();
            if let Some(msg) = context_message {
                obj.insert("additional_context".to_owned(), Value::String(msg));
            }
            to_json_compact(&Value::Object(obj))?
        }
        HookProtocol::GitHubCopilot => {
            let mut obj = Map::new();
            if let Some(msg) = context_message {
                obj.insert("additionalContext".to_owned(), Value::String(msg));
            }
            to_json_compact(&Value::Object(obj))?
        }
        HookProtocol::Codex => return Err(codex_unsupported_error()),
    };

    Ok(HookActionOutcome {
        stdout,
        stderr: None,
        exit_code: 0,
        format_plan,
    })
}

pub(super) fn codex_unsupported_error() -> CoreError {
    CoreError::HookEval {
        message: "OpenAI Codex does not expose runtime stdin/stdout hooks; use declarative approval rules or target `antigravity`, `claude-code`, `cursor`, or `github-copilot`".to_owned(),
    }
}

fn extract_tool_args_object(obj: &Map<String, Value>) -> Option<Map<String, Value>> {
    if let Some(args) = obj
        .get("toolCall")
        .and_then(Value::as_object)
        .and_then(|tc| tc.get("args"))
        .and_then(Value::as_object)
    {
        return Some(args.clone());
    }
    if let Some(input) = obj.get("tool_input").and_then(Value::as_object) {
        return Some(input.clone());
    }
    if let Some(raw_args) = obj.get("toolArgs") {
        if let Some(map) = raw_args.as_object() {
            return Some(map.clone());
        }
        if let Some(s) = raw_args.as_str()
            && let Ok(Value::Object(map)) = serde_json::from_str::<Value>(s)
        {
            return Some(map);
        }
    }
    None
}

fn first_str(obj: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(val) = obj.get(*key).and_then(Value::as_str) {
            return Some(val.to_owned());
        }
    }
    None
}

fn extract_str_array(obj: &Map<String, Value>, key: &str) -> Vec<String> {
    obj.get(key)
        .and_then(Value::as_array)
        .map_or_default(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
}

fn to_json_compact<T: serde::Serialize>(val: &T) -> Result<String, CoreError> {
    serde_json::to_string(val).map_err(|err| CoreError::HookEval {
        message: err.to_string(),
    })
}
