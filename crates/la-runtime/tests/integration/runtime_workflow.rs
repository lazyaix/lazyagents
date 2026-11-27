//! End-to-end filesystem, prober, importer, atomic writer, local pack loader,
//! and offline GitHub audit comparison black-box tests for `la-runtime`.

use core::assert_matches;
use std::fs;

use la_core::engineering::{ExpectedRulesets, MergeStrategy, TagProtectionRule};
use la_core::manifest::{GitOutputMode, PackRef, PackSource};
use la_core::{DetectedLanguage, ResourceId};
use la_runtime::{
    ApplyOptions, AuditCheckStatus, ImportSource, RuntimeError, apply_write_plan,
    build_initial_manifest, compare_github_state, import_workspace, load_local_packs,
    load_workspace, probe_repository, save_manifest, write_atomic, write_imported_workspace,
};
use tempfile::TempDir;

#[test]
fn probe_init_compile_atomic_write_and_conflict_guard_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Seed a realistic Rust + GitHub Actions repository.
    write_atomic(
        &root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/app\"]\n[workspace.package]\nedition = \"2024\"\nrust-version = \"1.100\"\n",
    )
    .unwrap();
    write_atomic(
        &root.join("rust-toolchain.toml"),
        "[toolchain]\nchannel = \"1.100.0\"\n",
    )
    .unwrap();
    write_atomic(&root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    write_atomic(
        &root.join(".git/config"),
        "[remote \"origin\"]\n\turl = git@github.com:lazyaix/demo.git\n",
    )
    .unwrap();
    write_atomic(
        &root.join(".github/workflows/ci.yml"),
        "name: CI\non: [push]\npermissions: {}\njobs:\n  test:\n    name: Test\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1\n        with:\n          persist-credentials: false\n",
    )
    .unwrap();

    let facts = probe_repository(root).unwrap();
    assert!(facts.has_language(DetectedLanguage::Rust));
    assert_eq!(facts.toolchain_version.as_deref(), Some("1.100.0"));
    assert_eq!(facts.git_remote_repo.as_deref(), Some("lazyaix/demo"));
    assert!(facts.has_github_actions());

    let manifest = build_initial_manifest(&facts, GitOutputMode::Commit);
    save_manifest(root, &manifest, false).unwrap();

    let ws = load_workspace(root, true).unwrap();
    let (_, plan) = ws.compile_and_plan(None).unwrap();
    assert!(plan.has_drift());

    let applied = apply_write_plan(root, &plan, ApplyOptions::default()).unwrap();
    assert!(applied.created.iter().any(|p| p == "AGENTS.md"));
    assert!(root.join(".lazyagents/lock.json").is_file());

    // Reload and verify zero drift + zero lint errors.
    let ws2 = load_workspace(root, true).unwrap();
    let (_, plan2, report) = ws2.check(None).unwrap();
    assert!(!plan2.has_drift());
    assert!(!report.has_errors(false));

    // Hand-editing AGENTS.md triggers ExternalConflict unless --force is set.
    fs::write(root.join("AGENTS.md"), "# manual edit\n").unwrap();
    let ws3 = load_workspace(root, true).unwrap();
    let (_, conflict_plan) = ws3.compile_and_plan(None).unwrap();
    assert!(conflict_plan.has_conflicts());

    let blocked = apply_write_plan(root, &conflict_plan, ApplyOptions::default());
    assert_matches!(
        blocked,
        Err(RuntimeError::ExternalConflict { count: 1, .. })
    );

    let forced = apply_write_plan(
        root,
        &conflict_plan,
        ApplyOptions {
            force: true,
            dry_run: false,
        },
    )
    .unwrap();
    assert!(forced.updated.iter().any(|p| p == "AGENTS.md"));
}

#[test]
fn imports_existing_antigravity_workspace_and_loads_local_packs() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    write_atomic(
        &root.join("AGENTS.md"),
        "# Legacy Guidelines\nFollow TDD.\n",
    )
    .unwrap();
    write_atomic(
        &root.join(".agents/rules/ts.md"),
        "---\ntrigger: glob\nglobs: **/*.ts, **/*.tsx\ndescription: TS rules\n---\nUse strict TS.\n",
    )
    .unwrap();
    write_atomic(
        &root.join(".agents/skills/deploy-app/SKILL.md"),
        "---\nname: deploy-app\ndescription: Deploy application safely.\n---\n# Steps\n",
    )
    .unwrap();
    write_atomic(
        &root.join(".agents/mcp_config.json"),
        r#"{"mcpServers":{"sqlite":{"command":"sqlite-mcp","args":["dev.db"]}}}"#,
    )
    .unwrap();

    let imported = import_workspace(root, ImportSource::Auto).unwrap();
    assert_eq!(imported.manifest.rules.len(), 1);
    assert_eq!(imported.manifest.skills.len(), 1);
    assert_eq!(imported.manifest.mcp_servers.len(), 1);

    write_imported_workspace(root, &imported).unwrap();
    let loaded = load_workspace(root, false).unwrap();
    let (compiled, _) = loaded.compile_and_plan(None).unwrap();
    assert!(compiled.files.iter().any(|f| f.path == "AGENTS.md"));
    assert!(
        compiled
            .files
            .iter()
            .any(|f| f.path == ".agents/skills/deploy-app/SKILL.md")
    );

    // Verify local pack loader with include_only / exclude.
    let pack_dir = root.join("local-pack");
    write_atomic(&pack_dir.join("sections/keep.md"), "keep\n").unwrap();
    write_atomic(&pack_dir.join("sections/skip.md"), "skip\n").unwrap();

    let loaded_pack = load_local_packs(
        root,
        &[PackRef {
            id: ResourceId::parse("team").unwrap(),
            source: PackSource::Local {
                path: "local-pack".to_owned(),
            },
            include_only: vec!["sections/*.md".to_owned()],
            exclude: vec!["sections/skip.md".to_owned()],
        }],
    )
    .unwrap();
    assert!(loaded_pack.contains_key("sections/keep.md"));
    assert!(!loaded_pack.contains_key("sections/skip.md"));
}

#[test]
fn offline_github_audit_reports_matches_and_actionable_remediation_commands() {
    let expected = ExpectedRulesets {
        protect_default_branch: true,
        require_linear_history: true,
        require_pr_before_merge: true,
        required_checks: vec!["Format".to_owned(), "Clippy".to_owned()],
        release_tags: TagProtectionRule {
            pattern: Some("v*".to_owned()),
            immutable_releases: true,
        },
    };

    let clean_repo = r#"{"allow_squash_merge":true,"allow_merge_commit":false,"allow_rebase_merge":false,"delete_branch_on_merge":true}"#;
    let clean_rulesets = r#"[
        {"name":"Protect main","target":"branch","enforcement":"active","conditions":{"ref_name":{"include":["~DEFAULT_BRANCH"]}},"rules":[{"type":"required_status_checks","parameters":{"required_status_checks":[{"context":"Format"},{"context":"Clippy"}]}}]},
        {"name":"Protect release tags","target":"tag","enforcement":"active","conditions":{"ref_name":{"include":["refs/tags/v*"]}},"rules":[]}
    ]"#;
    let pass = compare_github_state(
        "lazyaix/lazyagents",
        &expected,
        MergeStrategy::SquashOnly,
        clean_repo,
        clean_rulesets,
        Some(true),
    )
    .unwrap();
    assert!(!pass.has_mismatches());
    assert!(
        pass.checks
            .iter()
            .all(|c| c.status == AuditCheckStatus::Pass)
    );

    let drifted_repo = r#"{"allow_squash_merge":true,"allow_merge_commit":true,"allow_rebase_merge":false,"delete_branch_on_merge":false}"#;
    let drift = compare_github_state(
        "lazyaix/lazyagents",
        &expected,
        MergeStrategy::SquashOnly,
        drifted_repo,
        "[]",
        Some(false),
    )
    .unwrap();
    assert!(drift.has_mismatches());
    assert!(
        drift
            .format_text()
            .contains("gh api -X PUT repos/lazyaix/lazyagents/immutable-releases")
    );
}
