//! External `$VISUAL` / `$EDITOR` resolution and interactive handoff helper for M3 TUI
//! and CLI editing workflows.

use std::env;
use std::path::Path;
use std::process::Command;

use crate::error::RuntimeError;

/// Resolved external editor executable and leading arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorCommand {
    /// Executable binary name or path (e.g., `"nvim"`, `"hx"`, `"code"`).
    pub program: String,
    /// Leading command-line arguments (e.g., `["--wait"]` for VS Code).
    pub args: Vec<String>,
}

/// Resolves the preferred external editor command from `$VISUAL`, `$EDITOR`, or a platform fallback.
#[must_use]
pub fn resolve_editor_command() -> EditorCommand {
    let raw = env::var("VISUAL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| env::var("EDITOR").ok().filter(|v| !v.trim().is_empty()));

    if let Some(cmd) = raw
        && let Some(parsed) = parse_editor_string(&cmd)
    {
        return parsed;
    }

    cfg_select! {
        windows => EditorCommand {
            program: "notepad".to_owned(),
            args: Vec::new(),
        },
        _ => EditorCommand {
            program: "vi".to_owned(),
            args: Vec::new(),
        },
    }
}

/// Parses an editor command string (e.g., `"code --wait"` or `"nvim"`) into [`EditorCommand`].
#[must_use]
pub fn parse_editor_string(raw: &str) -> Option<EditorCommand> {
    let mut parts = raw.split_whitespace();
    let program = parts.next()?.to_owned();
    let args = parts.map(str::to_owned).collect();
    Some(EditorCommand { program, args })
}

/// Spawns the resolved external editor on `target_file` and waits for it to exit cleanly.
///
/// Callers running inside an active TUI MUST restore terminal raw mode / alternate screen
/// (via `DropGuard`) before invoking this function and re-enter after it returns.
///
/// # Errors
///
/// Returns [`RuntimeError::Editor`] if spawning the editor fails or it exits with non-zero status.
pub fn open_in_editor(target_file: &Path) -> Result<(), RuntimeError> {
    let editor = resolve_editor_command();
    let status = Command::new(&editor.program)
        .args(&editor.args)
        .arg(target_file)
        .status()
        .map_err(|err| RuntimeError::Editor {
            message: format!("failed to launch `{}`: {err}", editor.program),
        })?;

    if !status.success() {
        return Err(RuntimeError::Editor {
            message: format!("`{}` exited with status {status}", editor.program),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multi_word_editor_command_strings() {
        let parsed = parse_editor_string("code --wait --new-window").unwrap();
        assert_eq!(parsed.program, "code");
        assert_eq!(parsed.args, vec!["--wait", "--new-window"]);
        assert_eq!(parse_editor_string("   "), None);
    }
}
