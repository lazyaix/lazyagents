//! Multi-resolution `TestBackend` + `insta` snapshots for the 3-Zone Ultra Cyberpunk Command Deck:
//! `20x8` tiny guard, `80x24` compact, `120x40` standard, `180x50` ultrawide, `tui-big-text` splash,
//! `GraphType::Area` Budget Radar, Chinese Governance workbench, and GitHub Audit matrix.

use core::time::Duration;
use std::collections::BTreeMap;

use la_core::manifest::{InstructionSection, InstructionSpec, RuleSpec, RuleTrigger};
use la_core::{DetectedLanguage, ProjectFacts, ProjectManifest, ResourceId};
use la_tui::{
    Action, AppState, AuditCheckView, AuditStatusView, ColorProfile, GitHubAuditSnapshot, GlyphSet,
    LogEntry, LogLevel, MotionLevel, NavSection, TerminalCapabilities, TuiConfig, UiLang,
    WorkspaceSnapshot, render_studio,
};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn buffer_to_text(terminal: &Terminal<TestBackend>) -> String {
    let buf = terminal.backend().buffer();
    let area = buf.area;
    let mut out = String::new();
    for y in area.top()..area.bottom() {
        let mut line = String::new();
        for x in area.left()..area.right() {
            line.push_str(buf[(x, y)].symbol());
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

fn rich_workspace() -> WorkspaceSnapshot {
    let mut manifest = ProjectManifest::default();
    "lazyagents".clone_into(&mut manifest.project.name);
    manifest.instructions.push(InstructionSpec {
        output: "AGENTS.md".to_owned(),
        auto_probe: true,
        sections: vec![
            InstructionSection::Preset {
                preset: "builtin:engineering/git-workflow".to_owned(),
                when: None,
            },
            InstructionSection::Preset {
                preset: "builtin:lang/rust/core".to_owned(),
                when: None,
            },
        ],
    });
    if let Ok(id) = ResourceId::parse("rust-safety") {
        manifest.rules.push(RuleSpec {
            id,
            source: "rules/rust-safety.md".to_owned(),
            trigger: RuleTrigger::Glob,
            globs: vec!["**/*.rs".to_owned()],
            description: Some("Rust 1.100 safety baseline".to_owned()),
            enabled: true,
        });
    }

    let mut sources = BTreeMap::new();
    sources.insert(
        "rules/rust-safety.md".to_owned(),
        "# Rust Safety\n\nForbid `unsafe`, `unwrap`, and `panic!` in production code.\n".to_owned(),
    );

    let mut facts = ProjectFacts {
        name: "lazyagents".to_owned(),
        languages: vec![DetectedLanguage::Rust],
        toolchain_version: Some("1.100.0".to_owned()),
        language_edition: Some("2024".to_owned()),
        git_remote_repo: Some("lazyaix/lazyagents".to_owned()),
        git_default_branch: Some("main".to_owned()),
        ..ProjectFacts::default()
    };
    facts.refresh_flags();

    WorkspaceSnapshot::from_manifest(
        "/workspace/lazyagents",
        true,
        manifest,
        facts,
        sources,
        BTreeMap::new(),
        None,
    )
}

#[test]
fn snapshot_splash_tiny_guard_and_multi_resolution_command_deck() {
    let config = TuiConfig {
        glyphs: GlyphSet::Unicode,
        motion: MotionLevel::Reduced,
        ..TuiConfig::default()
    };

    // 1. Splash screen (120x28)
    let mut splash_state = AppState::new(config, TerminalCapabilities::default(), rich_workspace());
    splash_state.update(Action::Tick(Duration::from_millis(270)));
    let mut splash_term = Terminal::new(TestBackend::new(120, 28)).unwrap();
    splash_term
        .draw(|frame| render_studio(&mut splash_state, frame, Duration::from_millis(16)))
        .unwrap();
    insta::assert_snapshot!("splash_120x28", buffer_to_text(&splash_term));

    // 2. Tiny terminal guard (20x8) — zero panic
    let mut tiny_term = Terminal::new(TestBackend::new(20, 8)).unwrap();
    tiny_term
        .draw(|frame| render_studio(&mut splash_state, frame, Duration::from_millis(16)))
        .unwrap();
    insta::assert_snapshot!("tiny_guard_20x8", buffer_to_text(&tiny_term));

    // Enter Command Deck
    splash_state.update(Action::SkipSplash);

    // 3. Compact 80x24 (auto-collapsed rail & inspector)
    let mut term_80 = Terminal::new(TestBackend::new(80, 24)).unwrap();
    term_80
        .draw(|frame| render_studio(&mut splash_state, frame, Duration::from_millis(16)))
        .unwrap();
    insta::assert_snapshot!("deck_dashboard_80x24", buffer_to_text(&term_80));

    // 4. Standard 120x40 (full 3-Zone Command Deck + GraphType::Area Budget Radar)
    let mut term_120 = Terminal::new(TestBackend::new(120, 40)).unwrap();
    term_120
        .draw(|frame| render_studio(&mut splash_state, frame, Duration::from_millis(16)))
        .unwrap();
    insta::assert_snapshot!("deck_dashboard_120x40", buffer_to_text(&term_120));

    // 5. Ultrawide 180x50
    let mut ultrawide = Terminal::new(TestBackend::new(180, 50)).unwrap();
    ultrawide
        .draw(|frame| render_studio(&mut splash_state, frame, Duration::from_millis(16)))
        .unwrap();
    assert!(buffer_to_text(&ultrawide).contains("CONTEXT BUDGET RADAR"));
}

#[test]
fn snapshot_zh_governance_github_audit_and_redacted_logs_drawer() {
    let config = TuiConfig {
        lang: UiLang::Zh,
        glyphs: GlyphSet::Unicode,
        color_profile: ColorProfile::TrueColor,
        show_splash: false,
        ..TuiConfig::default()
    };
    let mut state = AppState::new(config, TerminalCapabilities::default(), rich_workspace());
    state.update(Action::SelectNav(NavSection::GitHubAudit));
    state.update(Action::GitHubAuditLoaded(GitHubAuditSnapshot {
        repo: "lazyaix/lazyagents".to_owned(),
        checks: vec![
            AuditCheckView {
                id: "merge-strategy".to_owned(),
                label: "Merge Strategy".to_owned(),
                status: AuditStatusView::Pass,
                detail: "Squash-only + auto-delete enabled.".to_owned(),
                remediation_cmd: None,
            },
            AuditCheckView {
                id: "immutable-releases".to_owned(),
                label: "Immutable Releases".to_owned(),
                status: AuditStatusView::Mismatch,
                detail: "Immutable Releases disabled.".to_owned(),
                remediation_cmd: Some(
                    "gh api -X PUT repos/lazyaix/lazyagents/immutable-releases".to_owned(),
                ),
            },
        ],
    }));
    state.update(Action::MoveDown);
    state.update(Action::ToggleLogsDrawer);
    state.update(Action::LogReceived(LogEntry {
        timestamp: "20:45:00".to_owned(),
        level: LogLevel::Info,
        target: "la_cli".to_owned(),
        message: format!("ghs_{}.{} scrubbed", "a".repeat(24), "b".repeat(24)),
    }));

    let mut term = Terminal::new(TestBackend::new(120, 36)).unwrap();
    term.draw(|frame| render_studio(&mut state, frame, Duration::from_millis(16)))
        .unwrap();
    let rendered = buffer_to_text(&term);
    assert!(rendered.contains("[REDACTED]"));
    insta::assert_snapshot!("zh_github_audit_and_logs_120x36", rendered);
}
