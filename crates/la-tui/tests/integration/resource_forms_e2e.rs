//! End-to-end `AppState` CRUD + live compilation + `TestBackend` snapshot tests
//! covering all 8 Antigravity resource modal forms (`Instructions`, `Rules`, `Skills`,
//! `Subagents`, `Hooks`, `McpServers`, `Manifests`, `Plugins`).

use core::assert_matches;
use core::time::Duration;
use std::collections::BTreeMap;

use la_core::manifest::{InstructionSection, McpTransport, ModelTier, RuleTrigger};
use la_core::{HookEvent, LintCode, ProjectFacts, ProjectManifest};
use la_tui::{
    Action, AppState, FormAction, FormTarget, GlyphSet, ModalKind, MotionLevel, NavSection,
    StudioCommand, TerminalCapabilities, TuiConfig, WorkspaceSnapshot, render_studio,
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

fn empty_studio_state() -> AppState {
    let mut manifest = ProjectManifest::default();
    "lazyagents".clone_into(&mut manifest.project.name);
    let ws = WorkspaceSnapshot::from_manifest(
        "/workspace/lazyagents",
        true,
        manifest,
        ProjectFacts::default(),
        BTreeMap::new(),
        BTreeMap::new(),
        None,
    );
    let config = TuiConfig {
        glyphs: GlyphSet::Unicode,
        motion: MotionLevel::None,
        show_splash: false,
        ..TuiConfig::default()
    };
    AppState::new(config, TerminalCapabilities::default(), ws)
}

fn replace_focused_text(state: &mut AppState, text: &str) {
    state.update(Action::FormInput(FormAction::MoveEnd));
    for _ in 0..80 {
        state.update(Action::FormInput(FormAction::Backspace));
    }
    state.update(Action::FormInput(FormAction::InsertStr(text.to_owned())));
}

#[test]
fn instructions_plan_and_section_crud_scaffolds_and_recompiles() {
    let mut state = empty_studio_state();
    state.update(Action::SelectNav(NavSection::Instructions));

    // Add nested Monorepo instruction plan via `A`.
    state.update(Action::OpenAddInstructionPlan);
    assert_eq!(state.modal, Some(ModalKind::FormModal));
    assert_eq!(
        state.active_form_target,
        Some(FormTarget::InstructionPlan { edit_idx: None })
    );
    replace_focused_text(&mut state, "crates/la-tui/AGENTS.md");
    let cmds = state.update(Action::FormInput(FormAction::Submit));
    assert!(state.modal.is_none());
    assert_matches!(cmds.first(), Some(StudioCommand::SaveManifest(_)));
    assert_eq!(state.workspace.manifest.instructions.len(), 1);
    assert_eq!(
        state.workspace.manifest.instructions[0].output,
        "crates/la-tui/AGENTS.md"
    );

    // Duplicate output path keeps modal open with domain error.
    state.update(Action::OpenAddInstructionPlan);
    replace_focused_text(&mut state, "crates/la-tui/AGENTS.md");
    let dup_cmds = state.update(Action::FormInput(FormAction::Submit));
    assert_eq!(dup_cmds, Vec::new());
    assert_eq!(state.modal, Some(ModalKind::FormModal));
    assert!(
        state
            .active_form
            .as_ref()
            .and_then(|f| f.submit_error.as_deref())
            .is_some_and(|e| e.contains("already exists"))
    );
    state.update(Action::FormInput(FormAction::Cancel));

    // Add a File section to root AGENTS.md via `a` -> scaffolds `.lazyagents/sections/tui.md`.
    state.list_index = 0;
    state.update(Action::OpenAddResource);
    state.update(Action::FormInput(FormAction::MoveRight)); // preset -> file
    state.update(Action::FormInput(FormAction::NextField));
    replace_focused_text(&mut state, "sections/tui.md");
    let cmds = state.update(Action::FormInput(FormAction::Submit));
    assert!(state.modal.is_none());
    assert_matches!(
        cmds.first(),
        Some(StudioCommand::SaveSourceFile { rel_path, .. }) if rel_path == "sections/tui.md"
    );
    assert_matches!(cmds.get(1), Some(StudioCommand::SaveManifest(_)));
    assert!(state.workspace.source_files.contains_key("sections/tui.md"));

    // Edit the newly appended section (`E`) into an Inline section.
    let sec_len = state.workspace.manifest.instructions[0].sections.len();
    state.section_index = sec_len.saturating_sub(1);
    state.update(Action::OpenEditResource);
    state.update(Action::FormInput(FormAction::MoveRight)); // file -> inline
    state.update(Action::FormInput(FormAction::NextField));
    replace_focused_text(&mut state, "## TUI Invariants\nZero hardcoded colors.");
    let cmds = state.update(Action::FormInput(FormAction::Submit));
    assert_matches!(cmds.first(), Some(StudioCommand::SaveManifest(_)));
    assert_matches!(
        state.workspace.manifest.instructions[0].sections.last(),
        Some(InstructionSection::Inline { inline, .. }) if inline.contains("TUI Invariants")
    );
}

#[test]
fn rules_skills_and_subagents_modal_crud_sync_source_files_and_compile() {
    let mut state = empty_studio_state();

    // 1. Rules: add + duplicate guard + edit.
    state.update(Action::SelectNav(NavSection::Rules));
    state.update(Action::OpenAddResource);
    replace_focused_text(&mut state, "rust-tui");
    state.update(Action::FormInput(FormAction::NextField)); // trigger (default glob)
    state.update(Action::FormInput(FormAction::NextField)); // globs (default **/*.rs)
    state.update(Action::FormInput(FormAction::NextField)); // description
    replace_focused_text(&mut state, "Ratatui theme tokens only");
    let cmds = state.update(Action::FormInput(FormAction::Submit));
    assert!(state.modal.is_none());
    assert_matches!(
        cmds.first(),
        Some(StudioCommand::SaveSourceFile { rel_path, .. }) if rel_path == "rules/rust-tui.md"
    );
    assert!(
        state
            .workspace
            .compiled
            .files
            .iter()
            .any(|f| f.path == ".agents/rules/rust-tui.md")
    );

    // Duplicate rule ID keeps modal open without dropping edits.
    state.update(Action::OpenAddResource);
    replace_focused_text(&mut state, "rust-tui");
    let dup_cmds = state.update(Action::FormInput(FormAction::Submit));
    assert_eq!(dup_cmds, Vec::new());
    assert_eq!(state.modal, Some(ModalKind::FormModal));
    state.update(Action::FormInput(FormAction::Cancel));

    // Edit rule trigger -> manual.
    state.list_index = 0;
    state.update(Action::OpenEditResource);
    state.update(Action::FormInput(FormAction::NextField)); // trigger
    state.update(Action::FormInput(FormAction::MoveRight)); // model_decision
    state.update(Action::FormInput(FormAction::MoveRight)); // manual
    let cmds = state.update(Action::FormInput(FormAction::Submit));
    assert_matches!(cmds.first(), Some(StudioCommand::SaveManifest(_)));
    assert_eq!(
        state.workspace.manifest.rules[0].trigger,
        RuleTrigger::Manual
    );

    // 2. Skills: add scaffolds valid YAML SKILL.md; edit updates frontmatter preserving body.
    state.update(Action::SelectNav(NavSection::Skills));
    state.update(Action::OpenAddResource);
    replace_focused_text(&mut state, "release-prep");
    state.update(Action::FormInput(FormAction::NextField));
    replace_focused_text(&mut state, "Run v0.1.1 release checks");
    let cmds = state.update(Action::FormInput(FormAction::Submit));
    assert_matches!(
        cmds.first(),
        Some(StudioCommand::SaveSourceFile { rel_path, content })
            if rel_path == "skills/release-prep/SKILL.md"
                && content.contains("name: release-prep")
                && content.contains("description: Run v0.1.1 release checks")
    );
    assert!(
        !state
            .workspace
            .lint_report
            .diagnostics
            .iter()
            .any(|d| d.code == LintCode::La004)
    );

    state.list_index = 0;
    state.update(Action::OpenEditResource);
    state.update(Action::FormInput(FormAction::NextField)); // description
    replace_focused_text(&mut state, "Updated release checklist");
    let cmds = state.update(Action::FormInput(FormAction::Submit));
    assert_matches!(
        cmds.first(),
        Some(StudioCommand::SaveSourceFile { content, .. })
            if content.contains("description: Updated release checklist")
                && content.contains("1. Step one.")
    );

    // 3. Subagents: 2-tab form (General + Antigravity).
    state.update(Action::SelectNav(NavSection::Subagents));
    state.update(Action::OpenAddResource);
    replace_focused_text(&mut state, "security-auditor");
    state.update(Action::FormInput(FormAction::NextField));
    replace_focused_text(&mut state, "Read-only security reviewer");
    state.update(Action::FormInput(FormAction::NextField)); // model -> pro
    state.update(Action::FormInput(FormAction::MoveLeft)); // wrap 0 -> 3 (pro)
    state.update(Action::FormInput(FormAction::NextTab)); // Antigravity tab
    state.update(Action::FormInput(FormAction::Activate)); // mainAgent = true
    state.update(Action::FormInput(FormAction::NextField)); // subagent
    state.update(Action::FormInput(FormAction::NextField)); // permissionMode
    replace_focused_text(&mut state, "plan");
    let cmds = state.update(Action::FormInput(FormAction::Submit));
    assert_matches!(
        cmds.first(),
        Some(StudioCommand::SaveSourceFile { rel_path, .. }) if rel_path == "agents/security-auditor.md"
    );
    let agent = &state.workspace.manifest.agents[0];
    assert_eq!(agent.model, ModelTier::Pro);
    assert!(agent.targets.antigravity.main_agent);
    assert_eq!(
        agent.targets.antigravity.permission_mode.as_deref(),
        Some("plan")
    );
    let compiled_agent = state
        .workspace
        .compiled
        .files
        .iter()
        .find(|f| f.path == ".agents/agents/security-auditor.md")
        .unwrap();
    assert!(compiled_agent.content.contains("mainAgent: true"));
    assert!(compiled_agent.content.contains("permissionMode: plan"));
}

#[test]
fn hooks_and_mcp_forms_enforce_la001_and_snapshot_2tab_modal() {
    let mut state = empty_studio_state();

    // 1. Hooks: add regex PreToolUse hook + edit timeout.
    state.update(Action::SelectNav(NavSection::Hooks));
    state.update(Action::OpenAddResource);
    replace_focused_text(&mut state, "custom-lint-guard");
    state.update(Action::FormInput(FormAction::NextField)); // event (PreToolUse)
    state.update(Action::FormInput(FormAction::NextField)); // matcher_mode (none -> capability -> regex)
    state.update(Action::FormInput(FormAction::MoveLeft)); // regex
    state.update(Action::FormInput(FormAction::NextField)); // matcher_value
    replace_focused_text(&mut state, "run_command|edit_file");
    state.update(Action::FormInput(FormAction::NextField)); // command (keep default)
    state.update(Action::FormInput(FormAction::NextField)); // timeout_secs
    replace_focused_text(&mut state, "20");
    let cmds = state.update(Action::FormInput(FormAction::Submit));
    assert_matches!(cmds.first(), Some(StudioCommand::SaveManifest(_)));
    assert_eq!(state.workspace.manifest.hooks.len(), 1);
    assert_eq!(
        state.workspace.manifest.hooks[0].event,
        HookEvent::PreToolUse
    );
    assert_eq!(state.workspace.manifest.hooks[0].timeout_secs, 20);

    // 2. MCP Server: 2-tab form with LA001 secret guard + snapshot at 120x36.
    state.update(Action::SelectNav(NavSection::McpServers));
    state.update(Action::OpenAddResource);
    replace_focused_text(&mut state, "docs-mcp");
    state.update(Action::FormInput(FormAction::NextTab)); // Vars & Headers tab

    // Attempt plaintext GitHub token in header value -> blocked by LA001!
    state.update(Action::FormInput(FormAction::AddItem));
    state.update(Action::FormInput(FormAction::InsertStr(
        "Authorization".to_owned(),
    )));
    state.update(Action::FormInput(FormAction::Activate));
    let fake_secret = format!("Bearer ghs_{}.{}", "a".repeat(25), "b".repeat(25));
    state.update(Action::FormInput(FormAction::InsertStr(fake_secret)));
    state.update(Action::FormInput(FormAction::Activate));
    assert!(
        state
            .active_form
            .as_ref()
            .and_then(|f| f.fields.iter().find(|fld| fld.key == "env_or_headers"))
            .and_then(|fld| fld.error.as_deref())
            .is_some_and(|e| e.contains("LA001"))
    );
    // Cancel bad draft and add safe `${DOCS_TOKEN}` header + setup variable.
    state.update(Action::FormInput(FormAction::Cancel));
    state.update(Action::FormInput(FormAction::AddItem));
    state.update(Action::FormInput(FormAction::InsertStr(
        "Authorization".to_owned(),
    )));
    state.update(Action::FormInput(FormAction::Activate));
    state.update(Action::FormInput(FormAction::InsertStr(
        "Bearer ${DOCS_TOKEN}".to_owned(),
    )));
    state.update(Action::FormInput(FormAction::Activate));

    state.update(Action::FormInput(FormAction::NextField)); // variables
    state.update(Action::FormInput(FormAction::AddItem));
    state.update(Action::FormInput(FormAction::InsertStr(
        "DOCS_TOKEN".to_owned(),
    )));
    state.update(Action::FormInput(FormAction::Activate));
    state.update(Action::FormInput(FormAction::InsertStr(
        "Docs API Token".to_owned(),
    )));
    state.update(Action::FormInput(FormAction::Activate));
    state.update(Action::FormInput(FormAction::InsertStr(
        "Read-only docs bearer token".to_owned(),
    )));
    state.update(Action::FormInput(FormAction::Activate));

    let mut term = Terminal::new(TestBackend::new(120, 36)).unwrap();
    term.draw(|frame| render_studio(&mut state, frame, Duration::from_millis(16)))
        .unwrap();
    let rendered = buffer_to_text(&term);
    assert!(rendered.contains("Authorization = Bearer ${DOCS_TOKEN}"));
    assert!(rendered.contains("DOCS_TOKEN | Docs API Token"));
    insta::assert_snapshot!("mcp_server_form_2tab_120x36", rendered);

    let cmds = state.update(Action::FormInput(FormAction::Submit));
    assert!(state.modal.is_none());
    assert_matches!(cmds.first(), Some(StudioCommand::SaveManifest(_)));
    assert_eq!(state.workspace.manifest.mcp_servers.len(), 1);
    assert_matches!(
        &state.workspace.manifest.mcp_servers[0].transport,
        McpTransport::StreamableHttp { headers, .. }
            if headers.get("Authorization").map(String::as_str) == Some("Bearer ${DOCS_TOKEN}")
    );

    // 3. Stdio MCP Server variant.
    state.update(Action::OpenAddResource);
    replace_focused_text(&mut state, "local-fs");
    state.update(Action::FormInput(FormAction::NextField)); // transport -> stdio
    state.update(Action::FormInput(FormAction::MoveRight));
    state.update(Action::FormInput(FormAction::NextField)); // url
    state.update(Action::FormInput(FormAction::NextField)); // command
    replace_focused_text(&mut state, "npx");
    state.update(Action::FormInput(FormAction::NextField)); // args
    state.update(Action::FormInput(FormAction::AddItem));
    state.update(Action::FormInput(FormAction::InsertStr("-y".to_owned())));
    state.update(Action::FormInput(FormAction::Activate));
    let cmds = state.update(Action::FormInput(FormAction::Submit));
    assert_matches!(cmds.first(), Some(StudioCommand::SaveManifest(_)));
    assert_matches!(
        &state.workspace.manifest.mcp_servers[1].transport,
        McpTransport::Stdio { command, args, .. } if command == "npx" && args == &["-y"]
    );
}

#[test]
fn index_manifests_and_plugin_forms_scroll_at_80x24_and_emit_logo_load() {
    let mut state = empty_studio_state();

    // 1. Index Manifest (`skills.json`) entries + inherits.
    state.update(Action::SelectNav(NavSection::Manifests));
    state.list_index = 0;
    state.update(Action::OpenEditResource);
    state.update(Action::FormInput(FormAction::AddItem));
    state.update(Action::FormInput(FormAction::InsertStr(
        "../shared-skills".to_owned(),
    )));
    state.update(Action::FormInput(FormAction::Activate));
    state.update(Action::FormInput(FormAction::InsertStr(
        "pr-author, release".to_owned(),
    )));
    state.update(Action::FormInput(FormAction::Activate));
    state.update(Action::FormInput(FormAction::InsertStr("draft".to_owned())));
    state.update(Action::FormInput(FormAction::Activate));

    state.update(Action::FormInput(FormAction::NextTab)); // Inherits
    state.update(Action::FormInput(FormAction::AddItem));
    state.update(Action::FormInput(FormAction::InsertStr(
        "../base/skills.json".to_owned(),
    )));
    state.update(Action::FormInput(FormAction::Activate));
    state.update(Action::FormInput(FormAction::Activate)); // empty include_only
    state.update(Action::FormInput(FormAction::Activate)); // empty exclude

    let cmds = state.update(Action::FormInput(FormAction::Submit));
    assert_matches!(cmds.first(), Some(StudioCommand::SaveManifest(_)));
    assert_eq!(state.workspace.manifest.manifests.skills.entries.len(), 1);
    assert_eq!(
        state.workspace.manifest.manifests.skills.entries[0].include_only,
        vec!["pr-author", "release"]
    );
    assert_eq!(state.workspace.manifest.manifests.skills.inherits.len(), 1);

    // Seed a real rule first so the plugin bundle references a declared rule.
    state.update(Action::SelectNav(NavSection::Rules));
    state.update(Action::OpenAddResource);
    replace_focused_text(&mut state, "rust-tui");
    state.update(Action::FormInput(FormAction::Submit));

    // 2. Plugins: 10-field Metadata tab + Bundle Contents tab + compact 80x24 scroll snapshot.
    state.update(Action::SelectNav(NavSection::Plugins));
    state.update(Action::OpenAddResource);
    replace_focused_text(&mut state, "team-kit");
    state.update(Action::FormInput(FormAction::NextField)); // display_name
    replace_focused_text(&mut state, "Team Kit");
    state.update(Action::FormInput(FormAction::NextField)); // version
    state.update(Action::FormInput(FormAction::NextField)); // description
    replace_focused_text(&mut state, "Cohesive governance plugin");
    state.update(Action::FormInput(FormAction::NextField)); // author
    replace_focused_text(&mut state, "LazyAIX");
    state.update(Action::FormInput(FormAction::NextField)); // homepage
    replace_focused_text(&mut state, "https://github.com/lazyaix/lazyagents");
    state.update(Action::FormInput(FormAction::NextField)); // repository
    replace_focused_text(&mut state, "https://github.com/lazyaix/lazyagents");
    state.update(Action::FormInput(FormAction::NextField)); // logo
    replace_focused_text(&mut state, "assets/team.png");
    state.update(Action::FormInput(FormAction::NextField)); // suggested_prompts
    state.update(Action::FormInput(FormAction::NextField)); // enabled (field 9 -> triggers scroll!)

    let mut term_80 = Terminal::new(TestBackend::new(80, 24)).unwrap();
    term_80
        .draw(|frame| render_studio(&mut state, frame, Duration::from_millis(16)))
        .unwrap();
    let rendered_80 = buffer_to_text(&term_80);
    // Scrolled viewport at 80x24 keeps the focused 10th field (`Enabled`) visible!
    assert!(rendered_80.contains("Enabled"));
    assert!(rendered_80.contains("assets/team.png"));
    insta::assert_snapshot!("plugin_form_scrolled_80x24", rendered_80);

    // Switch to Bundle Contents tab: unknown rule ID fails domain check cleanly without closing.
    state.update(Action::FormInput(FormAction::NextTab));
    state.update(Action::FormInput(FormAction::AddItem));
    state.update(Action::FormInput(FormAction::InsertStr(
        "ghost-rule".to_owned(),
    )));
    state.update(Action::FormInput(FormAction::Activate));
    let bad_cmds = state.update(Action::FormInput(FormAction::Submit));
    assert_eq!(bad_cmds, Vec::new());
    assert_eq!(state.modal, Some(ModalKind::FormModal));
    assert!(
        state
            .active_form
            .as_ref()
            .and_then(|f| f.submit_error.as_deref())
            .is_some_and(|e| e.contains("ghost-rule"))
    );

    // Replace with declared `rust-tui` rule and submit cleanly.
    state.update(Action::FormInput(FormAction::RemoveItem));
    state.update(Action::FormInput(FormAction::AddItem));
    state.update(Action::FormInput(FormAction::InsertStr(
        "rust-tui".to_owned(),
    )));
    state.update(Action::FormInput(FormAction::Activate));

    let cmds = state.update(Action::FormInput(FormAction::Submit));
    assert!(state.modal.is_none());
    assert_matches!(
        cmds.first(),
        Some(StudioCommand::LoadPluginLogo { plugin_name, rel_path })
            if plugin_name == "team-kit" && rel_path == "assets/team.png"
    );
    assert_matches!(cmds.get(1), Some(StudioCommand::SaveManifest(_)));
    let plugin = &state.workspace.manifest.plugins[0];
    assert_eq!(plugin.author.as_deref(), Some("LazyAIX"));
    assert_eq!(plugin.include_rules.len(), 1);
    assert!(
        state
            .workspace
            .compiled
            .files
            .iter()
            .any(|f| f.path == ".agents/plugins/team-kit/plugin.json")
    );
    assert!(
        state
            .workspace
            .compiled
            .files
            .iter()
            .any(|f| f.path == ".agents/plugins/team-kit/rules/AGENTS.md")
    );
    assert!(
        !state
            .workspace
            .lint_report
            .diagnostics
            .iter()
            .any(|d| d.code == LintCode::La010)
    );
}
