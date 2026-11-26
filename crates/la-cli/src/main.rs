//! `lazyagents` CLI binary entrypoint wiring `la-core`, `la-runtime`, and `la-tui`.

mod startup;

use std::io;

fn main() -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    startup::render_startup_summary(&mut stdout)
}
