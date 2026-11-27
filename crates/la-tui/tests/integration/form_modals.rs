//! Multi-resolution, CJK/Emoji, and `NO_COLOR`/ASCII `TestBackend` + `insta` snapshot
//! tests for the reusable Modal Form Engine, Delete Confirmation Modal, and Offline Catalog Picker.

use core::assert_matches;
use core::time::Duration;
use std::collections::BTreeMap;

use la_core::manifest::{RuleSpec, RuleTrigger};
use la_core::{ProjectFacts, ProjectManifest, ResourceId, TargetKind};
use la_tui::{
    Action, AppState, ColorProfile, FieldValidator, FormAction, FormField, FormTab, GlyphSet,
    KeyValueEntry, KeyValueListState, ModalFormState, ModalKind, MotionLevel, NavSection,
    StudioCommand, TerminalCapabilities, TuiConfig, UiLang, WorkspaceSnapshot, render_studio,
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

fn base_workspace() -> WorkspaceSnapshot {
    let mut manifest = ProjectManifest::default();
    "lazyagents".clone_into(&mut manifest.project.name);
    if let Ok(id) = ResourceId::parse("rust-safety") {
        manifest.rules.push(RuleSpec {
            id,
            source: "rules/rust-safety.md".to_owned(),
            trigger: RuleTrigger::Glob,
            globs: vec!["crates/**/*.rs".to_owned()],
            description: Some("Rust 1.100 safety baseline".to_owned()),
            enabled: true,
        });
    }
    let mut sources = BTreeMap::new();
    sources.insert(
        "rules/rust-safety.md".to_owned(),
        "# Rust Safety\nNever use unsafe.\n".to_owned(),
    );
    WorkspaceSnapshot::from_manifest(
        "/workspace/lazyagents",
        true,
        manifest,
        ProjectFacts::default(),
        sources,
        BTreeMap::new(),
        None,
    )
}

fn sample_multi_tab_form() -> ModalFormState {
    ModalFormState::new(
        "Edit Conditional Rule / 编辑规则",
        vec![
            FormField::text(
                "id",
                "Rule ID",
                "rust-safety",
                "rust-safety",
                FieldValidator::ResourceId,
            )
            .with_hint("Lowercase ID compiled to .agents/rules/<id>.md"),
            FormField::text(
                "description",
                "Description",
                "赛博安全基线 🚀",
                "Rule summary",
                FieldValidator::Required,
            ),
            FormField::toggle("enabled", "Enabled", true),
            FormField::enum_select(
                "trigger",
                "Trigger",
                vec![
                    "always_on".into(),
                    "glob".into(),
                    "model_decision".into(),
                    "manual".into(),
                ],
                1,
            ),
            FormField::string_list(
                "globs",
                "Globs",
                vec!["crates/**/*.rs".into(), "docs/**/*.md".into()],
                "src/**/*.ts",
                FieldValidator::Required,
            ),
            FormField::kv_list(
                "headers",
                "Headers",
                KeyValueListState::new(
                    vec![KeyValueEntry {
                        key: "Authorization".to_owned(),
                        value: "Bearer ${GITHUB_MCP_TOKEN}".to_owned(),
                        extra: None,
                    }],
                    "Header",
                    "Value",
                    None,
                    true,
                ),
            )
            .on_tab(1),
        ],
    )
    .with_subtitle("Project scope")
    .with_tabs(vec![
        FormTab::general("General"),
        FormTab::for_target("Antigravity", TargetKind::Antigravity),
    ])
}

#[test]
fn modal_form_cjk_multi_resolution_and_no_color_ascii_snapshots() {
    // 1. Compact 80x24 with CJK + Emoji text + StringList.
    let cfg_80 = TuiConfig {
        glyphs: GlyphSet::Unicode,
        motion: MotionLevel::None,
        show_splash: false,
        ..TuiConfig::default()
    };
    let mut state_80 = AppState::new(cfg_80, TerminalCapabilities::default(), base_workspace());
    state_80.update(Action::OpenForm(Box::new(sample_multi_tab_form())));
    state_80.update(Action::FormInput(FormAction::NextField));

    let mut term_80 = Terminal::new(TestBackend::new(80, 24)).unwrap();
    term_80
        .draw(|frame| render_studio(&mut state_80, frame, Duration::from_millis(16)))
        .unwrap();
    let rendered_80 = buffer_to_text(&term_80);
    assert!(rendered_80.contains("赛 博 安 全 基 线"));
    insta::assert_snapshot!("form_modal_cjk_80x24", rendered_80);

    // 2. Standard 120x40 in Chinese switching to Tab 2 (Antigravity TargetBadge + KV list).
    let cfg_zh = TuiConfig {
        lang: UiLang::Zh,
        glyphs: GlyphSet::Unicode,
        motion: MotionLevel::None,
        show_splash: false,
        ..TuiConfig::default()
    };
    let mut state_zh = AppState::new(cfg_zh, TerminalCapabilities::default(), base_workspace());
    state_zh.update(Action::OpenForm(Box::new(sample_multi_tab_form())));
    state_zh.update(Action::FormInput(FormAction::NextTab));

    let mut term_120 = Terminal::new(TestBackend::new(120, 40)).unwrap();
    term_120
        .draw(|frame| render_studio(&mut state_zh, frame, Duration::from_millis(16)))
        .unwrap();
    let rendered_120 = buffer_to_text(&term_120);
    assert!(rendered_120.contains("Authorization = Bearer ${GITHUB_MCP_TOKEN}"));
    insta::assert_snapshot!("form_modal_zh_120x40", rendered_120);

    // 3. NO_COLOR + ASCII degradation at 80x24 with live submit validation error.
    let cfg_ascii = TuiConfig {
        color_profile: ColorProfile::NoColor,
        glyphs: GlyphSet::Ascii,
        motion: MotionLevel::None,
        show_splash: false,
        ..TuiConfig::default()
    };
    let mut state_ascii =
        AppState::new(cfg_ascii, TerminalCapabilities::default(), base_workspace());
    let mut form = sample_multi_tab_form();
    // Inject invalid ID to trigger live validation banner.
    form.apply(FormAction::InsertStr(" BAD!".to_owned()));
    state_ascii.update(Action::OpenForm(Box::new(form)));
    state_ascii.update(Action::FormInput(FormAction::Submit));
    assert_eq!(state_ascii.modal, Some(ModalKind::FormModal));

    let mut term_ascii = Terminal::new(TestBackend::new(80, 24)).unwrap();
    term_ascii
        .draw(|frame| render_studio(&mut state_ascii, frame, Duration::from_millis(16)))
        .unwrap();
    let rendered_ascii = buffer_to_text(&term_ascii);
    insta::assert_snapshot!("form_modal_no_color_ascii_80x24", rendered_ascii);
}

#[test]
fn delete_confirm_modal_toggles_purge_and_deletes_resource_cleanly() {
    let config = TuiConfig {
        glyphs: GlyphSet::Unicode,
        motion: MotionLevel::None,
        show_splash: false,
        ..TuiConfig::default()
    };
    let mut state = AppState::new(config, TerminalCapabilities::default(), base_workspace());
    state.update(Action::SelectNav(NavSection::Rules));
    assert_eq!(state.workspace.manifest.rules.len(), 1);

    state.update(Action::OpenDeleteConfirm);
    assert_eq!(state.modal, Some(ModalKind::ConfirmDelete));
    state.update(Action::ToggleDeletePurge);

    let mut term = Terminal::new(TestBackend::new(120, 34)).unwrap();
    term.draw(|frame| render_studio(&mut state, frame, Duration::from_millis(16)))
        .unwrap();
    let rendered = buffer_to_text(&term);
    assert!(rendered.contains("CONFIRM DELETE"));
    assert!(rendered.contains(".lazyagents/rules/rust-safety.md"));
    insta::assert_snapshot!("delete_confirm_modal_120x34", rendered);

    let cmds = state.update(Action::ConfirmDelete);
    assert!(state.modal.is_none());
    assert_eq!(state.workspace.manifest.rules.len(), 0);
    assert!(
        !state
            .workspace
            .source_files
            .contains_key("rules/rust-safety.md")
    );
    assert_matches!(
        cmds.first(),
        Some(StudioCommand::DeleteSourceFile { rel_path }) if rel_path == "rules/rust-safety.md"
    );
    assert_matches!(cmds.get(1), Some(StudioCommand::SaveManifest(_)));
}

#[test]
fn catalog_picker_fuzzy_filters_and_installs_github_mcp_preset() {
    let config = TuiConfig {
        glyphs: GlyphSet::Unicode,
        motion: MotionLevel::None,
        show_splash: false,
        ..TuiConfig::default()
    };
    let mut state = AppState::new(config, TerminalCapabilities::default(), base_workspace());
    state.update(Action::OpenCatalogPicker);
    assert_eq!(state.modal, Some(ModalKind::CatalogPicker));

    for ch in "mcp:git".chars() {
        state.update(Action::CatalogInput(ch));
    }

    let mut term = Terminal::new(TestBackend::new(120, 34)).unwrap();
    term.draw(|frame| render_studio(&mut state, frame, Duration::from_millis(16)))
        .unwrap();
    let rendered = buffer_to_text(&term);
    assert!(rendered.contains("OFFLINE CATALOG"));
    assert!(rendered.contains("mcp:github"));
    insta::assert_snapshot!("catalog_picker_modal_120x34", rendered);

    let cmds = state.update(Action::CatalogConfirm);
    assert!(state.modal.is_none());
    assert_matches!(cmds.first(), Some(StudioCommand::SaveManifest(_)));
    assert!(
        state
            .workspace
            .manifest
            .mcp_servers
            .iter()
            .any(|m| m.name.as_str() == "github")
    );
}
