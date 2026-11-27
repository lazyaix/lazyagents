//! Top-level CLI command dispatcher and structured exit-code contract (`0` / `1` / `2`).

mod hooks_gh;
mod manage;
mod workspace;

use std::env;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use la_tui::TuiConfig;

use crate::cli::{Cli, Command};
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
    let root = match resolve_workspace_root(cli.dir) {
        Ok(path) => path,
        Err(msg) => return CommandOutcome::RuntimeError(msg),
    };

    let Some(command) = cli.command else {
        let tui = TuiConfig {
            motion: cli.motion.into(),
            show_splash: !cli.no_splash,
            ..TuiConfig::default()
        };
        let is_tty = std::io::stdout().is_terminal() && !cli.no_tui;
        return match startup::render_startup_banner(out, &tui, cli.lang, is_tty) {
            Ok(()) => CommandOutcome::Pass,
            Err(io_err) => CommandOutcome::RuntimeError(io_err.to_string()),
        };
    };

    match command {
        Command::Init(args) => workspace::cmd_init(&root, &args, out),
        Command::Import(args) => workspace::cmd_import(&root, &args, out),
        Command::Generate(args) => workspace::cmd_generate(&root, &args, out),
        Command::Diff(args) => workspace::cmd_diff(&root, &args, out),
        Command::Check(args) => workspace::cmd_check(&root, &args, out),
        Command::Inspect(args) => workspace::cmd_inspect(&root, &args, out),
        Command::Add(args) => manage::cmd_add(&root, &args, out),
        Command::Remove(args) => manage::cmd_remove(&root, &args, out),
        Command::Enable(args) => manage::cmd_toggle(&root, &args, true, out),
        Command::Disable(args) => manage::cmd_toggle(&root, &args, false, out),
        Command::Hook { command } => hooks_gh::cmd_hook(&root, command, out, err),
        Command::Github { command } => hooks_gh::cmd_github(&root, command, out),
    }
}

fn resolve_workspace_root(explicit: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(dir) = explicit {
        return Ok(dir);
    }
    env::current_dir().map_err(|err| format!("failed to determine current directory: {err}"))
}
