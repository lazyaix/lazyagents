//! Top-level CLI command dispatcher and structured exit-code contract (`0` / `1` / `2`).

mod hooks_gh;
mod manage;
mod workspace;

use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use la_runtime::discover_workspace_root;
use la_tui::TerminalCapabilities;

use crate::cli::{Cli, Command};
use crate::prefs::{
    collect_terminal_env_hints, load_preferences_from, preferences_path, resolve_tui_config,
};
use crate::startup;

/// Outcome of executing a CLI invocation, mapping deterministically to POSIX exit codes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CommandOutcome {
    /// Command completed cleanly with zero errors or drift (exit code `0`).
    Pass,
    /// Check, diff (`--exit-code`), or GitHub audit found policy/drift failures (exit code `1`).
    CheckFailed,
    /// Invalid usage, malformed configuration, missing auth, or I/O error (exit code `2`).
    RuntimeError(String),
}

impl CommandOutcome {
    pub(crate) fn into_exit_code(self, err: &mut impl Write) -> ExitCode {
        match self {
            Self::Pass => ExitCode::SUCCESS,
            Self::CheckFailed => ExitCode::from(1),
            Self::RuntimeError(msg) => {
                let _ = writeln!(err, "error: {msg}");
                ExitCode::from(2)
            }
        }
    }
}

/// Dispatches parsed [`Cli`] arguments against explicit `out` and `err` writers.
pub(crate) fn dispatch(cli: Cli, out: &mut impl Write, err: &mut impl Write) -> CommandOutcome {
    let cwd = match env::current_dir() {
        Ok(path) => path,
        Err(io_err) => {
            return CommandOutcome::RuntimeError(format!(
                "failed to determine current directory: {io_err}"
            ));
        }
    };
    let explicit_dir = cli.dir.clone();

    let Some(command) = cli.command else {
        let env_hints = collect_terminal_env_hints();
        let caps = TerminalCapabilities::from_env_hints(&env_hints);
        let saved = preferences_path()
            .as_deref()
            .and_then(load_preferences_from);
        let tui = resolve_tui_config(&cli, caps, &env_hints, saved);
        return match startup::render_startup_banner(out, &tui, tui.lang) {
            Ok(()) => CommandOutcome::Pass,
            Err(io_err) => CommandOutcome::RuntimeError(io_err.to_string()),
        };
    };

    let init_root = resolve_init_root(explicit_dir.as_deref(), &cwd);
    let root = resolve_workspace_root(explicit_dir.as_deref(), &cwd);

    match command {
        Command::Init(args) => workspace::cmd_init(&init_root, &args, out),
        Command::Import(args) => workspace::cmd_import(&init_root, &args, out),
        Command::Generate(args) => workspace::cmd_generate(&root, &args, out),
        Command::Diff(args) => workspace::cmd_diff(&root, &args, out),
        Command::Check(args) => workspace::cmd_check(&root, &args, out),
        Command::Inspect(args) => workspace::cmd_inspect(&root, &args, out),
        Command::Add(args) => manage::cmd_add(&root, &args, out),
        Command::Remove(args) => manage::cmd_remove(&root, &args, out),
        Command::Enable(args) => manage::cmd_toggle(&root, &args, true, out),
        Command::Disable(args) => manage::cmd_toggle(&root, &args, false, out),
        Command::Hook { command } => {
            hooks_gh::cmd_hook(explicit_dir.as_deref(), &cwd, command, out, err)
        }
        Command::Github { command } => hooks_gh::cmd_github(&root, command, out),
    }
}

fn resolve_init_root(explicit: Option<&Path>, cwd: &Path) -> PathBuf {
    explicit.map_or_else(|| cwd.to_path_buf(), Path::to_path_buf)
}

fn resolve_workspace_root(explicit: Option<&Path>, cwd: &Path) -> PathBuf {
    if let Some(dir) = explicit {
        return dir.to_path_buf();
    }
    discover_workspace_root(cwd).unwrap_or_else(|| cwd.to_path_buf())
}
