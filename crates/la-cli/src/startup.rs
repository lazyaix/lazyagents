//! Startup banner assembly bridging `la-core`, `la-runtime`, and `la-tui` ahead of the M3 TUI loop.

use std::io::{self, Write};

use la_core::AgentId;
use la_runtime::{RuntimeConfig, RuntimeHandle};
use la_tui::{TuiConfig, status_label};

use crate::cli::CliLang;

/// Writes a concise startup summary banner to the supplied output sink.
pub(crate) fn render_startup_banner(
    out: &mut impl Write,
    tui: &TuiConfig,
    lang: CliLang,
    interactive_tty: bool,
) -> io::Result<()> {
    let runtime = RuntimeHandle::new(RuntimeConfig {
        root_agent_id: AgentId::new("studio"),
        ..RuntimeConfig::default()
    });

    writeln!(
        out,
        "lazyagents v{} [status={} | viewport={:?} | motion={:?}]",
        env!("CARGO_PKG_VERSION"),
        status_label(runtime.status()),
        tui.viewport,
        tui.motion,
    )?;

    match (lang, interactive_tty) {
        (CliLang::Zh, true) => writeln!(
            out,
            "提示：L4 交互式 TUI 配置工作室将于 M3 上线；当前可使用 `lazyagents --help` 查看全部 CLI 子命令。"
        ),
        (CliLang::Zh, false) => writeln!(
            out,
            "非 TTY 模式：请运行 `lazyagents --help` 查看 CI 与自动化命令行子命令。"
        ),
        (CliLang::En, true) => writeln!(
            out,
            "Hint: L4 interactive TUI Config Studio lands in M3; run `lazyagents --help` for headless CLI commands."
        ),
        (CliLang::En, false) => writeln!(
            out,
            "Headless mode: run `lazyagents --help` for CI and automation subcommands."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_banner_formats_cleanly_in_english_and_chinese() {
        let tui = TuiConfig::default();
        let mut en_buf = Vec::new();
        render_startup_banner(&mut en_buf, &tui, CliLang::En, false).unwrap();
        let en = String::from_utf8(en_buf).unwrap();
        assert!(en.contains("lazyagents v"));
        assert!(en.contains("status=IDLE"));
        assert!(en.contains("Headless mode"));

        let mut zh_buf = Vec::new();
        render_startup_banner(&mut zh_buf, &tui, CliLang::Zh, true).unwrap();
        let zh = String::from_utf8(zh_buf).unwrap();
        assert!(zh.contains("L4 交互式 TUI"));
    }
}
