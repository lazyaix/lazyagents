//! Zero-stdout TUI observability layer routing `tracing` events exclusively to a rotating
//! log file (`tracing-appender`) and the in-TUI secret-redacted ring buffer drawer.

use std::env;
use std::fmt;
use std::path::PathBuf;

use la_core::redact_ghs_tokens;
use la_tui::{Action, LogEntry, LogLevel};
use tokio::sync::mpsc;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::prelude::*;

/// Guard holding the non-blocking rotating file logger worker alive for the TUI session lifetime.
pub(crate) struct TuiLoggingGuard {
    _file_guard: Option<WorkerGuard>,
}

/// Initializes a zero-stdout `tracing` subscriber for TUI sessions.
///
/// Never writes to stdout or stderr while the TUI screen buffer is active.
#[must_use]
pub(crate) fn init_tui_tracing(tx: mpsc::Sender<Action>) -> TuiLoggingGuard {
    let ring_layer = InTuiRingLayer { tx };

    let (file_layer, file_guard) = log_directory().map_or((None, None), |dir| {
        let _ = std::fs::create_dir_all(&dir);
        let appender = tracing_appender::rolling::daily(dir, "lazyagents-tui.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(appender);
        let layer = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(non_blocking);
        (Some(layer), Some(guard))
    });

    let _ = tracing_subscriber::registry()
        .with(ring_layer)
        .with(file_layer)
        .try_init();

    TuiLoggingGuard {
        _file_guard: file_guard,
    }
}

struct InTuiRingLayer {
    tx: mpsc::Sender<Action>,
}

struct MessageVisitor {
    message: String,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if !self.message.is_empty() {
            self.message.push(' ');
        }
        if field.name() == "message" {
            let _ = fmt::write(&mut self.message, format_args!("{value:?}"));
        } else {
            let _ = fmt::write(
                &mut self.message,
                format_args!("{}={value:?}", field.name()),
            );
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if !self.message.is_empty() {
            self.message.push(' ');
        }
        if field.name() == "message" {
            self.message.push_str(value);
        } else {
            let _ = fmt::write(&mut self.message, format_args!("{}={value}", field.name()));
        }
    }
}

impl<S: Subscriber> Layer<S> for InTuiRingLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let level = match *meta.level() {
            Level::ERROR => LogLevel::Error,
            Level::WARN => LogLevel::Warn,
            Level::INFO => LogLevel::Info,
            Level::DEBUG | Level::TRACE => LogLevel::Debug,
        };
        let mut visitor = MessageVisitor {
            message: String::new(),
        };
        event.record(&mut visitor);

        let entry = LogEntry {
            timestamp: "LIVE".to_owned(),
            level,
            target: meta.target().to_owned(),
            message: redact_ghs_tokens(&visitor.message),
        };
        let _ = self.tx.try_send(Action::LogReceived(entry));
    }
}

fn log_directory() -> Option<PathBuf> {
    cfg_select! {
        windows => {
            env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .map(|dir| dir.join("lazyagents").join("logs"))
        },
        _ => {
            if let Some(xdg) = env::var_os("XDG_STATE_HOME").filter(|v| !v.is_empty()) {
                return Some(PathBuf::from(xdg).join("lazyagents").join("logs"));
            }
            env::var_os("HOME")
                .filter(|v| !v.is_empty())
                .map(|home| PathBuf::from(home).join(".local").join("state").join("lazyagents").join("logs"))
        },
    }
}
