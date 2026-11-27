//! On-demand Tokio async TUI runner orchestrating the 3-Zone Ultra Cyberpunk Command Deck.
//!
//! Only instantiated when the user invokes `lazyagents` interactively on a TTY (without `--no-tui`),
//! keeping headless CLI subcommands and `lazyagents hook eval` at zero async runtime overhead.

mod bridge;
mod input;
mod logging;
mod terminal;
mod watcher;

use core::time::Duration;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::Instant;

use futures_util::StreamExt as _;
use la_tui::protocol::ProgressBarState;
use la_tui::{
    Action, AppState, StudioCommand, TerminalCapabilities, ToastLevel, TuiConfig, render_studio,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::info;

use crate::cli::Cli;
use crate::prefs::{
    collect_terminal_env_hints, load_preferences_from, preferences_path, resolve_tui_config,
};
use crate::tui_runner::terminal::{
    TerminalSession, emergency_restore_terminal, probe_terminal_capabilities,
};

static PANIC_HOOK_ONCE: Once = Once::new();

/// Resolves workspace root from `cli`, starts the on-demand Tokio runtime, and runs the TUI Studio.
pub(crate) fn run_studio_from_cli(cli: &Cli) -> Result<(), String> {
    let root = match &cli.dir {
        Some(dir) => dir.clone(),
        None => env::current_dir()
            .map_err(|err| format!("failed to determine current directory: {err}"))?,
    };
    run_studio(&root, cli)
}

/// Runs the interactive L4 Cyberpunk HUD Config Studio against `root`.
pub(crate) fn run_studio(root: &Path, cli: &Cli) -> Result<(), String> {
    let env_hints = collect_terminal_env_hints();
    let caps = probe_terminal_capabilities(&env_hints);
    let saved_prefs = preferences_path()
        .as_deref()
        .and_then(load_preferences_from);
    let config = resolve_tui_config(cli, caps, &env_hints, saved_prefs);

    install_terminal_panic_hook();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start TUI async runtime: {err}"))?;

    let mut session = TerminalSession::enter(&config, caps)
        .map_err(|err| format!("failed to initialize terminal session: {err}"))?;

    runtime.block_on(run_event_loop(root, config, caps, &mut session))
}

fn install_terminal_panic_hook() {
    PANIC_HOOK_ONCE.call_once(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            emergency_restore_terminal();
            prev(info);
        }));
    });
}

#[expect(
    clippy::future_not_send,
    reason = "run_event_loop executes on the main TUI thread via Runtime::block_on and owns !Send TachyonFX AppState"
)]
async fn run_event_loop(
    root: &Path,
    config: TuiConfig,
    caps: TerminalCapabilities,
    session: &mut TerminalSession,
) -> Result<(), String> {
    let (tx, mut rx) = mpsc::channel::<Action>(256);
    let _log_guard = logging::init_tui_tracing(tx.clone());
    let tracker = TaskTracker::new();
    let cancel = CancellationToken::new();

    let workspace = bridge::load_tui_workspace_snapshot(root);
    bridge::spawn_plugin_logo_loaders(root, &workspace.manifest, &tx, &tracker, &cancel);
    let _watcher = watcher::WorkspaceWatcher::start(root, tx.clone());

    let mut state = AppState::new(config, caps, workspace);
    if state.splash_active
        && let Ok(area) = session.size()
    {
        state.effects.trigger_splash(area, &state.theme);
    }

    info!(
        theme = state.config.theme.as_str(),
        shader = state.config.shader_preset.as_str(),
        motion = state.config.motion.as_str(),
        "lazyagents L4 Cyberpunk Config Studio started"
    );

    session
        .draw_synchronized(|frame| render_studio(&mut state, frame, Duration::ZERO))
        .map_err(|err| format!("terminal render error: {err}"))?;

    let mut events = crossterm::event::EventStream::new();
    let frame_ms = 1_000_u64
        .checked_div(u64::from(state.config.active_fps))
        .unwrap_or(16)
        .max(1);
    let frame_period = Duration::from_millis(frame_ms);
    let mut last_frame = Instant::now();

    while !state.should_quit {
        let animating = state.needs_animation_tick();
        let next =
            await_next_event(&mut events, &mut rx, frame_period, animating, last_frame).await;

        let mut pending = Vec::<Action>::with_capacity(16);
        match next {
            LoopEvent::Closed => break,
            LoopEvent::Terminal(ev) => {
                if let Some(action) = input::translate_event(ev, &state) {
                    pending.push(action);
                }
            }
            LoopEvent::Action(action) => pending.push(action),
            LoopEvent::Tick(dt) => pending.push(Action::Tick(dt)),
            LoopEvent::Shutdown => pending.push(Action::Quit),
        }

        drain_ready_actions(&mut rx, &mut pending);

        let mut dispatch_ctx = DispatchContext {
            root,
            session,
            tx: &tx,
            tracker: &tracker,
            cancel: &cancel,
        };
        for action in pending {
            handle_inbound_hooks(&action, &mut dispatch_ctx);
            let cmds = state.update(action);
            for cmd in cmds {
                handle_studio_command(cmd, &mut state, &mut dispatch_ctx);
            }
        }

        if state.should_quit {
            break;
        }

        let dt = last_frame.elapsed().min(Duration::from_millis(100));
        last_frame = Instant::now();
        session
            .draw_synchronized(|frame| render_studio(&mut state, frame, dt))
            .map_err(|err| format!("terminal render error: {err}"))?;
    }

    session.set_progress(ProgressBarState::Hidden);
    cancel.cancel();
    tracker.close();
    let _ = tokio::time::timeout(Duration::from_millis(500), tracker.wait()).await;
    Ok(())
}

enum LoopEvent {
    Terminal(crossterm::event::Event),
    Action(Action),
    Tick(Duration),
    Shutdown,
    Closed,
}

struct DispatchContext<'a> {
    root: &'a Path,
    session: &'a mut TerminalSession,
    tx: &'a mpsc::Sender<Action>,
    tracker: &'a TaskTracker,
    cancel: &'a CancellationToken,
}

async fn await_next_event(
    events: &mut crossterm::event::EventStream,
    rx: &mut mpsc::Receiver<Action>,
    frame_period: Duration,
    animating: bool,
    last_frame: Instant,
) -> LoopEvent {
    tokio::select! {
        maybe_ev = events.next() => match maybe_ev {
            Some(Ok(ev)) => LoopEvent::Terminal(ev),
            Some(Err(_)) | None => LoopEvent::Closed,
        },
        maybe_act = rx.recv() => maybe_act.map_or(LoopEvent::Closed, LoopEvent::Action),
        () = tokio::time::sleep(frame_period), if animating => {
            LoopEvent::Tick(last_frame.elapsed().min(Duration::from_millis(100)))
        }
        () = wait_shutdown_signal() => LoopEvent::Shutdown,
    }
}

fn drain_ready_actions(rx: &mut mpsc::Receiver<Action>, pending: &mut Vec<Action>) {
    for _ in 0..64 {
        match rx.try_recv() {
            Ok(action) => pending.push(action),
            Err(_) => break,
        }
    }
}

fn handle_inbound_hooks(action: &Action, ctx: &mut DispatchContext<'_>) {
    match action {
        Action::WorkspaceLoaded(ws) => {
            bridge::spawn_plugin_logo_loaders(
                ctx.root,
                &ws.manifest,
                ctx.tx,
                ctx.tracker,
                ctx.cancel,
            );
        }
        Action::SyncCompleted {
            created,
            updated,
            deleted,
        } => {
            ctx.session.set_progress(ProgressBarState::Hidden);
            ctx.session.notify_desktop(
                "lazyagents",
                &format!("Sync complete (+{created} ~{updated} -{deleted})"),
            );
            bridge::spawn_studio_command(
                StudioCommand::ReloadWorkspace,
                ctx.root,
                ctx.tx,
                ctx.tracker,
                ctx.cancel,
            );
        }
        Action::SyncFailed(_) | Action::GitHubAuditLoaded(_) | Action::GitHubAuditFailed(_) => {
            ctx.session.set_progress(ProgressBarState::Hidden);
        }
        _ => {}
    }
}

fn handle_studio_command(cmd: StudioCommand, state: &mut AppState, ctx: &mut DispatchContext<'_>) {
    match cmd {
        StudioCommand::Quit => {
            state.should_quit = true;
        }
        StudioCommand::CopyToClipboard(text) => {
            ctx.session.copy_osc52(&text);
        }
        StudioCommand::OpenExternalEditor { rel_path } => {
            let full_path = resolve_editor_path(ctx.root, &rel_path);
            match ctx.session.run_external_editor(&full_path) {
                Ok(()) => {
                    let _ = state.update(Action::Notify {
                        level: ToastLevel::Info,
                        message: format!("Reloading after editor: {rel_path}"),
                    });
                    bridge::spawn_studio_command(
                        StudioCommand::ReloadWorkspace,
                        ctx.root,
                        ctx.tx,
                        ctx.tracker,
                        ctx.cancel,
                    );
                }
                Err(msg) => {
                    let _ = state.update(Action::Notify {
                        level: ToastLevel::Error,
                        message: msg,
                    });
                }
            }
        }
        StudioCommand::ApplySync { .. } | StudioCommand::RunGitHubAudit => {
            ctx.session.set_progress(ProgressBarState::Indeterminate);
            bridge::spawn_studio_command(cmd, ctx.root, ctx.tx, ctx.tracker, ctx.cancel);
        }
        other => {
            bridge::spawn_studio_command(other, ctx.root, ctx.tx, ctx.tracker, ctx.cancel);
        }
    }
}

fn resolve_editor_path(root: &Path, rel_path: &str) -> PathBuf {
    let direct = root.join(rel_path);
    if direct.exists() {
        direct
    } else {
        root.join(".lazyagents").join(rel_path)
    }
}

async fn wait_shutdown_signal() {
    cfg_select! {
        unix => {
            let ctrl_c = async {
                let _ = tokio::signal::ctrl_c().await;
            };
            let terminate = async {
                if let Ok(mut sigterm) =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                {
                    let _ = sigterm.recv().await;
                } else {
                    core::future::pending::<()>().await;
                }
            };
            tokio::select! {
                () = ctrl_c => {},
                () = terminate => {},
            }
        },
        _ => {
            let _ = tokio::signal::ctrl_c().await;
        },
    }
}
