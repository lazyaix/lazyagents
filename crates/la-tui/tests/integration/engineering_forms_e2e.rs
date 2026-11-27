//! End-to-end `AppState` modal form CRUD + deletion + live compilation + `TestBackend` snapshot
//! tests covering all 7 `[engineering]` governance domains (`Git`, `ProtectedPaths`, `Verify`,
//! `GitHub`, `SupplyChain`, `Architecture`, `Release`).

use core::assert_matches;
use core::time::Duration;
use std::collections::BTreeMap;

use la_core::engineering::{
    BranchingModel, CiInstallMode, GitHubScaffold, MergeStrategy, PrTemplateSyncMode, ReleaseTool,
    StopHookMode,
};
use la_core::{PolicyDecision, ProjectFacts, ProjectManifest};
use la_tui::{
    Action, AppState, FormAction, FormTarget, GlyphSet, GovernanceDomain, ModalKind, MotionLevel,
    NavSection, StudioCommand, TerminalCapabilities, TuiConfig, WorkspaceSnapshot, render_studio,
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

fn engineering_state() -> AppState {
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
    let mut state = AppState::new(config, TerminalCapabilities::default(), ws);
    state.update(Action::SelectNav(NavSection::Engineering));
    state
}

fn replace_focused_text(state: &mut AppState, text: &str) {
    state.update(Action::FormInput(FormAction::MoveEnd));
    for _ in 0..80 {
        state.update(Action::FormInput(FormAction::Backspace));
    }
    state.update(Action::FormInput(FormAction::InsertStr(text.to_owned())));
}

#[test]
fn git_and_protected_paths_domains_edit_compile_codeowners_and_delete() {
    let mut state = engineering_state();

    // 1. Git domain (2 tabs: Workflow + Worktree & Guardrails).
    state.governance_domain = GovernanceDomain::Git;
    state.update(Action::OpenEditResource);
    assert_eq!(state.modal, Some(ModalKind::FormModal));
    assert_eq!(
        state.active_form_target,
        Some(FormTarget::EngineeringDomain(GovernanceDomain::Git))
    );
    replace_focused_text(&mut state, "trunk");
    state.update(Action::FormInput(FormAction::NextField)); // branching_model -> github-flow
    state.update(Action::FormInput(FormAction::MoveRight));
    state.update(Action::FormInput(FormAction::NextField)); // merge_strategy -> merge
    state.update(Action::FormInput(FormAction::MoveRight));
    state.update(Action::FormInput(FormAction::NextTab)); // Worktree & Guardrails tab
    state.update(Action::FormInput(FormAction::NextField)); // worktree_pattern
    replace_focused_text(&mut state, "../wt-{task}");
    for _ in 0..4 {
        state.update(Action::FormInput(FormAction::NextField)); // guard_broad_staging (ask -> force_ask -> deny)
    }
    state.update(Action::FormInput(FormAction::MoveRight));
    state.update(Action::FormInput(FormAction::MoveRight));

    let cmds = state.update(Action::FormInput(FormAction::Submit));
    assert!(state.modal.is_none());
    assert_matches!(cmds.first(), Some(StudioCommand::SaveManifest(_)));
    let git = &state.workspace.manifest.engineering.git;
    assert_eq!(git.default_branch, "trunk");
    assert_eq!(git.branching_model, BranchingModel::GithubFlow);
    assert_eq!(git.merge_strategy, MergeStrategy::Merge);
    assert_eq!(git.worktree.pattern, "../wt-{task}");
    assert_eq!(git.guardrails.guard_broad_staging, PolicyDecision::Deny);

    // 2. ProtectedPaths domain: `a` focuses entries directly, validates decision, syncs CODEOWNERS.
    state.governance_domain = GovernanceDomain::ProtectedPaths;
    state.update(Action::OpenAddResource);
    assert_eq!(
        state
            .active_form
            .as_ref()
            .and_then(|f| f.fields.get(f.focused_field))
            .map(|f| f.key.as_str()),
        Some("entries")
    );
    // Toggle codeowners_sync ON first (prev field).
    state.update(Action::FormInput(FormAction::PrevField));
    state.update(Action::FormInput(FormAction::Activate));
    state.update(Action::FormInput(FormAction::NextField));

    // Add entry with invalid decision -> domain error keeps modal open.
    state.update(Action::FormInput(FormAction::AddItem));
    state.update(Action::FormInput(FormAction::InsertStr(
        ".github/**".to_owned(),
    )));
    state.update(Action::FormInput(FormAction::Activate));
    state.update(Action::FormInput(FormAction::InsertStr("yolo".to_owned())));
    state.update(Action::FormInput(FormAction::Activate));
    state.update(Action::FormInput(FormAction::InsertStr(
        "@lazyaix/sec".to_owned(),
    )));
    state.update(Action::FormInput(FormAction::Activate));
    let bad = state.update(Action::FormInput(FormAction::Submit));
    assert_eq!(bad, Vec::new());
    assert_eq!(state.modal, Some(ModalKind::FormModal));

    // Replace with valid entry and submit.
    state.update(Action::FormInput(FormAction::RemoveItem));
    state.update(Action::FormInput(FormAction::AddItem));
    state.update(Action::FormInput(FormAction::InsertStr(
        ".github/**".to_owned(),
    )));
    state.update(Action::FormInput(FormAction::Activate));
    state.update(Action::FormInput(FormAction::InsertStr("deny".to_owned())));
    state.update(Action::FormInput(FormAction::Activate));
    state.update(Action::FormInput(FormAction::InsertStr(
        "@lazyaix/sec, @maintainers".to_owned(),
    )));
    state.update(Action::FormInput(FormAction::Activate));
    let cmds = state.update(Action::FormInput(FormAction::Submit));
    assert_matches!(cmds.first(), Some(StudioCommand::SaveManifest(_)));
    let pp = &state.workspace.manifest.engineering.protected_paths;
    assert!(pp.codeowners_sync);
    assert_eq!(pp.entries.len(), 1);
    assert_eq!(pp.entries[0].decision, Some(PolicyDecision::Deny));
    assert_eq!(pp.entries[0].owners, vec!["@lazyaix/sec", "@maintainers"]);
    assert!(
        state
            .workspace
            .compiled
            .files
            .iter()
            .any(|f| f.path == ".github/CODEOWNERS" && f.content.contains("@lazyaix/sec"))
    );

    // Delete protected path row (`governance_row = 2`) via `d` + `ConfirmDelete`.
    state.governance_row = 2;
    state.update(Action::OpenDeleteConfirm);
    assert_eq!(state.modal, Some(ModalKind::ConfirmDelete));
    let del_cmds = state.update(Action::ConfirmDelete);
    assert_matches!(del_cmds.first(), Some(StudioCommand::SaveManifest(_)));
    assert_eq!(
        state
            .workspace
            .manifest
            .engineering
            .protected_paths
            .entries
            .len(),
        0
    );
    assert_eq!(state.governance_row, 1);
}

#[test]
fn verify_domain_command_and_stop_mode_crud() {
    let mut state = engineering_state();

    state.governance_domain = GovernanceDomain::Verify;
    state.update(Action::OpenAddResource);
    assert_eq!(
        state.active_form_target,
        Some(FormTarget::VerifyCommand { edit_idx: None })
    );
    replace_focused_text(&mut state, "clippy");
    state.update(Action::FormInput(FormAction::NextField));
    replace_focused_text(&mut state, "Clippy Strict");
    state.update(Action::FormInput(FormAction::NextField));
    replace_focused_text(&mut state, "cargo clippy --workspace -- -D warnings");
    let cmds = state.update(Action::FormInput(FormAction::Submit));
    assert_matches!(cmds.first(), Some(StudioCommand::SaveManifest(_)));
    assert_eq!(
        state.workspace.manifest.engineering.verify.commands.len(),
        1
    );

    // Edit selected command (row 1) -> flip required OFF.
    state.governance_row = 1;
    state.update(Action::OpenEditResource);
    assert_eq!(
        state.active_form_target,
        Some(FormTarget::VerifyCommand { edit_idx: Some(0) })
    );
    state.update(Action::FormInput(FormAction::PrevField)); // wraps to `required`
    state.update(Action::FormInput(FormAction::Activate));
    state.update(Action::FormInput(FormAction::Submit));
    assert!(!state.workspace.manifest.engineering.verify.commands[0].required);

    // Edit row 0 (`stop_hook_mode` -> Enforce).
    state.governance_row = 0;
    state.update(Action::OpenEditResource);
    assert_eq!(
        state.active_form_target,
        Some(FormTarget::EngineeringDomain(GovernanceDomain::Verify))
    );
    state.update(Action::FormInput(FormAction::MoveRight)); // remind -> enforce
    state.update(Action::FormInput(FormAction::Submit));
    assert_eq!(
        state.workspace.manifest.engineering.verify.stop_hook_mode,
        StopHookMode::Enforce
    );

    // Delete command row 1 cleanly.
    state.governance_row = 1;
    state.update(Action::OpenDeleteConfirm);
    state.update(Action::ConfirmDelete);
    assert_eq!(
        state.workspace.manifest.engineering.verify.commands.len(),
        0
    );
}

#[test]
fn github_domain_2tab_form_validates_scaffolds_and_compiles_workflow() {
    let mut state = engineering_state();

    state.governance_domain = GovernanceDomain::GitHub;
    state.governance_row = 0;
    state.update(Action::OpenEditResource);
    state.update(Action::FormInput(FormAction::Activate)); // enabled = true
    state.update(Action::FormInput(FormAction::NextField)); // repo
    replace_focused_text(&mut state, "lazyaix/lazyagents");
    state.update(Action::FormInput(FormAction::NextField)); // ci_workflow = true
    state.update(Action::FormInput(FormAction::Activate));
    state.update(Action::FormInput(FormAction::NextField)); // ci_install -> cargo-workspace
    state.update(Action::FormInput(FormAction::MoveRight));
    state.update(Action::FormInput(FormAction::NextField)); // pr_template = true
    state.update(Action::FormInput(FormAction::Activate));
    state.update(Action::FormInput(FormAction::NextField)); // pr_template_mode -> managed-block
    state.update(Action::FormInput(FormAction::MoveRight));
    state.update(Action::FormInput(FormAction::NextField)); // lint_workflows = true
    state.update(Action::FormInput(FormAction::Activate));
    state.update(Action::FormInput(FormAction::NextField)); // enable_mcp_preset
    state.update(Action::FormInput(FormAction::NextField)); // enable_skills
    state.update(Action::FormInput(FormAction::AddItem));
    state.update(Action::FormInput(FormAction::InsertStr(
        "pr-author".to_owned(),
    )));
    state.update(Action::FormInput(FormAction::Activate));
    state.update(Action::FormInput(FormAction::NextField)); // scaffolds -> test invalid first
    state.update(Action::FormInput(FormAction::AddItem));
    state.update(Action::FormInput(FormAction::InsertStr(
        "bad-scaffold".to_owned(),
    )));
    state.update(Action::FormInput(FormAction::Activate));
    assert_eq!(
        state.update(Action::FormInput(FormAction::Submit)),
        Vec::new()
    );
    assert_eq!(state.modal, Some(ModalKind::FormModal));

    // Replace invalid scaffold with valid `pr-title`, switch to Tab 1 (`Expected Rulesets`).
    state.update(Action::FormInput(FormAction::RemoveItem));
    state.update(Action::FormInput(FormAction::AddItem));
    state.update(Action::FormInput(FormAction::InsertStr(
        "pr-title".to_owned(),
    )));
    state.update(Action::FormInput(FormAction::Activate));

    state.update(Action::FormInput(FormAction::NextTab)); // Expected Rulesets
    for _ in 0..3 {
        state.update(Action::FormInput(FormAction::NextField));
    }
    state.update(Action::FormInput(FormAction::AddItem));
    state.update(Action::FormInput(FormAction::InsertStr(
        "Clippy".to_owned(),
    )));
    state.update(Action::FormInput(FormAction::Activate));
    state.update(Action::FormInput(FormAction::NextField)); // release_tag_pattern
    replace_focused_text(&mut state, "v*");
    state.update(Action::FormInput(FormAction::NextField)); // release_immutable = true
    state.update(Action::FormInput(FormAction::Activate));

    let mut term = Terminal::new(TestBackend::new(120, 36)).unwrap();
    term.draw(|frame| render_studio(&mut state, frame, Duration::from_millis(16)))
        .unwrap();
    let rendered = buffer_to_text(&term);
    assert!(rendered.contains("Expected Rulesets"));
    assert!(rendered.contains("Clippy"));
    insta::assert_snapshot!("engineering_github_form_120x36", rendered);

    let cmds = state.update(Action::FormInput(FormAction::Submit));
    assert!(state.modal.is_none());
    assert_matches!(cmds.first(), Some(StudioCommand::SaveManifest(_)));
    let gh = &state.workspace.manifest.engineering.github;
    assert_eq!(gh.sync.ci_install, CiInstallMode::CargoWorkspace);
    assert_eq!(gh.sync.pr_template_mode, PrTemplateSyncMode::ManagedBlock);
    assert_eq!(gh.scaffolds, vec![GitHubScaffold::PrTitle]);
    assert_eq!(gh.expected_rulesets.required_checks, vec!["Clippy"]);
    assert!(
        state
            .workspace
            .compiled
            .files
            .iter()
            .any(|f| f.path == ".github/workflows/lazyagents.yml"
                && f.content.contains("cargo run --locked"))
    );
}

#[test]
fn supply_chain_architecture_and_release_domains_crud_and_snapshot() {
    let mut state = engineering_state();

    // 1. SupplyChain: invalid cooldown blocked, valid 14 days persisted.
    state.governance_domain = GovernanceDomain::SupplyChain;
    state.update(Action::OpenEditResource);
    state.update(Action::FormInput(FormAction::NextField)); // stateless_github_tokens
    state.update(Action::FormInput(FormAction::NextField)); // publish_cooldown_days
    replace_focused_text(&mut state, "not-a-number");
    assert_eq!(
        state.update(Action::FormInput(FormAction::Submit)),
        Vec::new()
    );
    assert_eq!(state.modal, Some(ModalKind::FormModal));
    replace_focused_text(&mut state, "14");
    state.update(Action::FormInput(FormAction::Submit));
    assert_eq!(
        state
            .workspace
            .manifest
            .engineering
            .supply_chain
            .publish_cooldown_days,
        14
    );

    // 2. Architecture: add doc + layer boundary, snapshot workbench, then delete both via `d`.
    state.governance_domain = GovernanceDomain::Architecture;
    state.update(Action::OpenAddResource); // pre-focused on `layers`
    state.update(Action::FormInput(FormAction::PrevField)); // `docs`
    state.update(Action::FormInput(FormAction::AddItem));
    state.update(Action::FormInput(FormAction::InsertStr(
        "docs/architecture.md".to_owned(),
    )));
    state.update(Action::FormInput(FormAction::Activate));
    state.update(Action::FormInput(FormAction::NextField)); // `layers`
    state.update(Action::FormInput(FormAction::AddItem));
    state.update(Action::FormInput(FormAction::InsertStr(
        "la-core".to_owned(),
    )));
    state.update(Action::FormInput(FormAction::Activate));
    state.update(Action::FormInput(FormAction::InsertStr(
        "crates/la-core/**".to_owned(),
    )));
    state.update(Action::FormInput(FormAction::Activate));
    state.update(Action::FormInput(FormAction::InsertStr(
        "Pure Sans-I/O with zero Tokio or filesystem dependencies.".to_owned(),
    )));
    state.update(Action::FormInput(FormAction::Activate));
    let cmds = state.update(Action::FormInput(FormAction::Submit));
    assert_matches!(cmds.first(), Some(StudioCommand::SaveManifest(_)));
    assert_eq!(
        state.workspace.manifest.engineering.architecture.docs.len(),
        1
    );
    assert_eq!(
        state
            .workspace
            .manifest
            .engineering
            .architecture
            .layers
            .len(),
        1
    );

    let mut term = Terminal::new(TestBackend::new(120, 34)).unwrap();
    term.draw(|frame| render_studio(&mut state, frame, Duration::from_millis(16)))
        .unwrap();
    let rendered = buffer_to_text(&term);
    assert!(rendered.contains("docs/architecture.md"));
    assert!(rendered.contains("la-core (crates/la-core/**)"));
    insta::assert_snapshot!("engineering_architecture_view_120x34", rendered);

    // Delete doc (row 0) then layer (now row 0).
    state.governance_row = 0;
    state.update(Action::OpenDeleteConfirm);
    state.update(Action::ConfirmDelete);
    assert_eq!(
        state.workspace.manifest.engineering.architecture.docs.len(),
        0
    );
    state.update(Action::OpenDeleteConfirm);
    state.update(Action::ConfirmDelete);
    assert_eq!(
        state
            .workspace
            .manifest
            .engineering
            .architecture
            .layers
            .len(),
        0
    );

    // 3. Release domain: edit tool, tag_pattern, immutable_releases.
    state.governance_domain = GovernanceDomain::Release;
    state.update(Action::OpenEditResource);
    state.update(Action::FormInput(FormAction::MoveRight)); // manual -> release-plz
    state.update(Action::FormInput(FormAction::NextField)); // tag_pattern
    replace_focused_text(&mut state, "v{version}");
    state.update(Action::FormInput(FormAction::NextField)); // immutable_releases -> false
    state.update(Action::FormInput(FormAction::Activate));
    let cmds = state.update(Action::FormInput(FormAction::Submit));
    assert_matches!(cmds.first(), Some(StudioCommand::SaveManifest(_)));
    let rel = &state.workspace.manifest.engineering.release;
    assert_eq!(rel.tool, ReleaseTool::ReleasePlz);
    assert_eq!(rel.tag_pattern, "v{version}");
    assert!(!rel.immutable_releases);
}
