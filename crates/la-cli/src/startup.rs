//! Startup banner assembly bridging `la-core`, `la-runtime`, and `la-tui`.

use std::io::{self, Write};

use la_core::AgentId;
use la_runtime::{RuntimeConfig, RuntimeHandle};
use la_tui::{TuiConfig, status_label};

/// Writes a concise startup summary line to the supplied output sink.
pub(crate) fn render_startup_summary(out: &mut impl Write) -> io::Result<()> {
    let runtime = RuntimeHandle::new(RuntimeConfig {
        root_agent_id: AgentId::new("orchestrator"),
        ..RuntimeConfig::default()
    });
    let tui = TuiConfig::default();

    writeln!(
        out,
        "lazyagents v{} [status={} | viewport={:?} | motion={:?}]",
        env!("CARGO_PKG_VERSION"),
        status_label(runtime.status()),
        tui.viewport,
        tui.motion,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_summary_formats_cleanly() {
        let mut buf = Vec::new();
        render_startup_summary(&mut buf).unwrap();
        let line = String::from_utf8(buf).unwrap();
        assert!(line.contains("lazyagents v"));
        assert!(line.contains("status=IDLE"));
        assert!(line.contains("Fullscreen"));
    }
}
