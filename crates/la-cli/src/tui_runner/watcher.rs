//! Debounced live filesystem watcher (`notify-debouncer-mini`) monitoring `.lazyagents/`,
//! `.agents/`, `.github/`, and `AGENTS.md` to auto-refresh the Cyberpunk Command Deck HUD.

use core::time::Duration;
use std::path::Path;

use la_tui::Action;
use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};
use tokio::sync::mpsc;
use tracing::{debug, warn};

/// Active debounced filesystem watcher handle (stops automatically when dropped).
pub(crate) struct WorkspaceWatcher {
    _debouncer: Debouncer<notify_debouncer_mini::notify::RecommendedWatcher>,
}

impl WorkspaceWatcher {
    /// Starts a 250 ms debounced watcher on managed config directories under `root`.
    /// Returns `None` gracefully if OS watcher limits or permissions prevent watching.
    #[must_use]
    pub(crate) fn start(root: &Path, tx: mpsc::Sender<Action>) -> Option<Self> {
        let mut debouncer = new_debouncer(
            Duration::from_millis(250),
            move |res: DebounceEventResult| match res {
                Ok(events) if !events.is_empty() => {
                    let relevant = events.iter().any(|ev| is_relevant_path(&ev.path));
                    if relevant {
                        debug!(count = events.len(), "workspace file change detected");
                        let _ = tx.try_send(Action::ReloadWorkspace);
                    }
                }
                Ok(_) => {}
                Err(err) => {
                    warn!(error = %err, "filesystem watcher error");
                }
            },
        )
        .ok()?;

        let watcher = debouncer.watcher();
        for rel in [".lazyagents", ".agents", ".github"] {
            let dir = root.join(rel);
            if dir.is_dir() {
                let _ = watcher.watch(&dir, RecursiveMode::Recursive);
            }
        }
        let _ = watcher.watch(root, RecursiveMode::NonRecursive);

        Some(Self {
            _debouncer: debouncer,
        })
    }
}

fn is_relevant_path(path: &Path) -> bool {
    let lossy = path.to_string_lossy();
    if lossy.contains("/target/") || lossy.contains("/.git/") || lossy.contains("\\target\\") {
        return false;
    }
    lossy.contains(".lazyagents")
        || lossy.contains(".agents")
        || lossy.contains(".github")
        || lossy.ends_with("AGENTS.md")
        || lossy.ends_with("GEMINI.md")
}
