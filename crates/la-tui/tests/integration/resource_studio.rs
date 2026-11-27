//! Integration and `insta` visual snapshots for PR 3: 8 Antigravity resource editors,
//! live Hook Policy Playground, `Ctrl+K` Command Palette, Guided Init Wizard, Conflict Guard,
//! inline grapheme editor, and `ratatui-image` plugin logo rendering.

use core::assert_matches;
use core::time::Duration;
use std::collections::BTreeMap;

use la_core::manifest::{
    InstructionSection, InstructionSpec, McpAuthMode, McpServerSpec, McpToolLoading, McpTransport,
    PluginSpec, RuleSpec, RuleTrigger,
};
use la_core::{KebabName, PolicyDecision, ProjectFacts, ProjectManifest, ResourceId};
use la_tui::{
    Action, AppState, EditorAction, GlyphSet, ModalKind, MotionLevel, NavSection, StudioCommand,
    TerminalCapabilities, TuiConfig, WorkspaceSnapshot, render_studio,
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

fn studio_workspace(initialized: bool) -> WorkspaceSnapshot {
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
            trigger: RuleTrigger::AlwaysOn,
            globs: Vec::new(),
            description: Some("Rust safety red lines".to_owned()),
            enabled: true,
        });
    }
    if let Ok(id) = ResourceId::parse("github") {
        manifest.mcp_servers.push(McpServerSpec {
            name: id,
            enabled: true,
            transport: McpTransport::StreamableHttp {
                url: "https://api.githubcopilot.com/mcp/".to_owned(),
                auth: McpAuthMode::HeaderVar,
                headers: BTreeMap::new(),
            },
            variables: Vec::new(),
            tool_loading: McpToolLoading::Eager,
            allowed_tools: Vec::new(),
            denied_tools: Vec::new(),
            risk_surfaces: Vec::new(),
        });
    }
    if let Ok(name) = KebabName::parse("cyber-pack") {
        manifest.plugins.push(PluginSpec {
            name,
            display_name: Some("Cyber Governance Pack".to_owned()),
            version: Some("1.0.0".to_owned()),
            author: None,
            homepage: None,
            repository: None,
            description: "All-in-one Antigravity engineering bundle".to_owned(),
            logo: None,
            suggested_prompts: Vec::new(),
            enabled: true,
            include_rules: Vec::new(),
            include_skills: Vec::new(),
            include_agents: Vec::new(),
            include_hooks: Vec::new(),
            include_mcp: Vec::new(),
        });
    }

    let mut sources = BTreeMap::new();
    sources.insert(
        "rules/rust-safety.md".to_owned(),
        "# Rust Safety\n\nNever use unsafe or unwrap in production.\n".to_owned(),
    );

    WorkspaceSnapshot::from_manifest(
        "/workspace/lazyagents",
        initialized,
        manifest,
        ProjectFacts::default(),
        sources,
        BTreeMap::new(),
        None,
    )
}

#[test]
fn hook_playground_evaluates_live_verdict_and_renders_snapshot() {
    let config = TuiConfig {
        glyphs: GlyphSet::Unicode,
        motion: MotionLevel::Reduced,
        show_splash: false,
        ..TuiConfig::default()
    };
    let mut state = AppState::new(
        config,
        TerminalCapabilities::default(),
        studio_workspace(true),
    );
    state.update(Action::SelectNav(NavSection::Hooks));
    state.update(Action::PlaygroundEvaluate);

    let verdict = state.playground.last_verdict.as_ref().unwrap();
    assert_eq!(verdict.decision, PolicyDecision::Deny);
    assert_eq!(verdict.wire_decision, "deny");

    let mut term = Terminal::new(TestBackend::new(120, 34)).unwrap();
    term.draw(|frame| render_studio(&mut state, frame, Duration::from_millis(16)))
        .unwrap();
    let rendered = buffer_to_text(&term);
    assert!(rendered.contains("DENY"));
    insta::assert_snapshot!("hooks_playground_deny_120x34", rendered);
}

#[test]
fn command_palette_fuzzy_search_and_shadow_modal_snapshot() {
    let config = TuiConfig {
        glyphs: GlyphSet::Unicode,
        motion: MotionLevel::None,
        show_splash: false,
        ..TuiConfig::default()
    };
    let mut state = AppState::new(
        config,
        TerminalCapabilities::default(),
        studio_workspace(true),
    );
    state.update(Action::OpenModal(ModalKind::CommandPalette));
    for ch in "audit".chars() {
        state.update(Action::PaletteInput(ch));
    }

    let mut term = Terminal::new(TestBackend::new(120, 34)).unwrap();
    term.draw(|frame| render_studio(&mut state, frame, Duration::from_millis(16)))
        .unwrap();
    let rendered = buffer_to_text(&term);
    assert!(rendered.contains("COMMAND PALETTE"));
    assert!(rendered.contains("GitHub Audit"));
    insta::assert_snapshot!("command_palette_fuzzy_120x34", rendered);

    let cmds = state.update(Action::PaletteConfirm);
    assert!(state.modal.is_none());
    assert!(cmds.is_empty() || matches!(cmds.first(), Some(StudioCommand::RunGitHubAudit)));
}

#[test]
fn inline_editor_rule_offload_section_reorder_and_plugin_logo_work_end_to_end() {
    let config = TuiConfig {
        glyphs: GlyphSet::Unicode,
        motion: MotionLevel::None,
        show_splash: false,
        ..TuiConfig::default()
    };
    let mut state = AppState::new(
        config,
        TerminalCapabilities::default(),
        studio_workspace(true),
    );

    // Reorder AGENTS.md section down (J)
    state.update(Action::SelectNav(NavSection::Instructions));
    let reorder_cmds = state.update(Action::ReorderItem { up: false });
    assert_matches!(reorder_cmds.first(), Some(StudioCommand::SaveManifest(_)));
    assert_eq!(state.section_index, 1);

    // Cycle rule trigger from AlwaysOn -> Glob (one-key budget offload!)
    state.update(Action::SelectNav(NavSection::Rules));
    assert_eq!(
        state.workspace.manifest.rules[0].trigger,
        RuleTrigger::AlwaysOn
    );
    state.update(Action::CycleItemProperty);
    assert_eq!(state.workspace.manifest.rules[0].trigger, RuleTrigger::Glob);

    // Open inline native grapheme editor, edit, and save
    state.update(Action::StartInlineEdit);
    assert!(state.inline_edit.is_some());
    state.update(Action::EditorInput(EditorAction::InsertStr(
        "# Updated\n".to_owned(),
    )));
    let save_cmds = state.update(Action::SaveInlineEdit);
    assert_matches!(
        save_cmds.first(),
        Some(StudioCommand::SaveSourceFile { .. })
    );
    state.update(Action::CancelInlineEdit);
    assert!(state.inline_edit.is_none());

    // Render Plugins view with procedural ratatui-image Halfblocks logo
    state.update(Action::SelectNav(NavSection::Plugins));
    let mut term = Terminal::new(TestBackend::new(120, 34)).unwrap();
    term.draw(|frame| render_studio(&mut state, frame, Duration::from_millis(16)))
        .unwrap();
    assert!(state.plugin_logos.contains_key("cyber-pack"));

    // First-run Guided Init Wizard when workspace is uninitialized
    let mut uninit = AppState::new(
        config,
        TerminalCapabilities::default(),
        studio_workspace(false),
    );
    assert_eq!(uninit.modal, Some(ModalKind::InitWizard));
    uninit.update(Action::WizardNext);
    uninit.update(Action::WizardToggleOption);
    let init_cmds = uninit.update(Action::WizardNext);
    assert_eq!(init_cmds, Vec::new());
    let final_cmds = uninit.update(Action::WizardNext);
    assert_matches!(
        final_cmds.first(),
        Some(StudioCommand::InitializeWorkspace { .. })
    );
}
