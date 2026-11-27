//! Headless / `--no-tui` startup banner summarizing resolved TUI & runtime telemetry.

use std::io::{self, Write};

use la_core::AgentId;
use la_runtime::{RuntimeConfig, RuntimeHandle};
use la_tui::{TuiConfig, UiLang, status_label};

/// Writes a concise startup summary banner to the supplied output sink in headless / `--no-tui` mode.
pub(crate) fn render_startup_banner(
    out: &mut impl Write,
    tui: &TuiConfig,
    lang: UiLang,
) -> io::Result<()> {
    let runtime = RuntimeHandle::new(RuntimeConfig {
        root_agent_id: AgentId::new("studio"),
        ..RuntimeConfig::default()
    });

    writeln!(
        out,
        "lazyagents v{} [status={} | theme={} | shader={} | viewport={:?} | motion={}]",
        env!("CARGO_PKG_VERSION"),
        status_label(runtime.status()),
        tui.theme,
        tui.shader_preset,
        tui.viewport,
        tui.motion.as_str(),
    )?;

    match lang {
        UiLang::Zh => writeln!(
            out,
            "非 TTY / --no-tui 模式：在交互终端运行 `lazyagents` 启动 L4 赛博朋克配置工作室，或运行 `lazyagents --help` 查看 CLI 子命令。"
        ),
        UiLang::En => writeln!(
            out,
            "Headless mode: run `lazyagents` in an interactive TTY for the L4 Cyberpunk Config Studio, or `lazyagents --help` for CLI subcommands."
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
        render_startup_banner(&mut en_buf, &tui, UiLang::En).unwrap();
        let en = String::from_utf8(en_buf).unwrap();
        assert!(en.contains("lazyagents v"));
        assert!(en.contains("status=IDLE"));
        assert!(en.contains("shader=cyber-overdrive"));
        assert!(en.contains("Headless mode"));

        let mut zh_buf = Vec::new();
        render_startup_banner(&mut zh_buf, &tui, UiLang::Zh).unwrap();
        let zh = String::from_utf8(zh_buf).unwrap();
        assert!(zh.contains("L4 赛博朋克配置工作室"));
    }
}
