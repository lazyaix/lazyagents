//! `lazyagents` CLI binary entrypoint wiring `la-core`, `la-runtime`, and `la-tui`.

mod cli;
mod commands;
mod prefs;
mod startup;
mod tui_runner;

use std::io::{self, IsTerminal as _, Write as _};
use std::process::ExitCode;

use clap::Parser as _;

use crate::cli::Cli;

fn main() -> ExitCode {
    let _ = color_eyre::install();

    let parsed = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            let is_help_or_version = matches!(
                err.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            );
            let _ = err.print();
            return if is_help_or_version {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            };
        }
    };

    if parsed.command.is_none() && !parsed.no_tui && io::stdout().is_terminal() {
        return match tui_runner::run_studio_from_cli(&parsed) {
            Ok(()) => ExitCode::SUCCESS,
            Err(msg) => {
                let mut stderr = io::stderr().lock();
                let _ = writeln!(stderr, "error: {msg}");
                ExitCode::from(2)
            }
        };
    }

    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    commands::dispatch(parsed, &mut stdout, &mut stderr).into_exit_code(&mut stderr)
}
