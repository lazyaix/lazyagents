//! End-to-end filesystem, prober, importer, atomic writer, local pack loader,
//! and offline GitHub audit comparison black-box tests for `la-runtime`.

use core::assert_matches;
use std::fs;

use la_core::engineering::{
    ExpectedRulesets, MergeStrategy, ProtectedPathEntry, TagProtectionRule,
};
use la_core::manifest::{GitOutputMode, PackRef, PackSource};
use la_core::{DetectedLanguage, ResourceId};
use la_runtime::{
    ApplyOptions, AuditCheckStatus, ImportSource, RuntimeError, apply_write_plan,
    build_initial_manifest, compare_github_state, discover_workspace_root, import_workspace,
    load_local_packs, load_workspace, probe_repository, resolve_editor_command, save_manifest,
    write_atomic, write_imported_workspace,
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

#[test]
fn prober_infers_default_branch_from_origin_head_packed_refs_and_worktrees() {
    // 1. Checked out on a feature branch, origin/HEAD resolves to `main`.
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    write_atomic(
        &root.join(".git/HEAD"),
        "ref: refs/heads/feat/tui-async-runner\n",
    )
    .unwrap();
    write_atomic(
        &root.join(".git/refs/remotes/origin/HEAD"),
        "ref: refs/remotes/origin/main\n",
    )
    .unwrap();
    write_atomic(
        &root.join(".git/config"),
        "[remote \"origin\"]\n\turl=https://github.com/lazyaix/prober-demo.git\n",
    )
    .unwrap();
    let facts = probe_repository(root).unwrap();
    assert_eq!(facts.git_default_branch.as_deref(), Some("main"));
    assert_eq!(
        facts.git_remote_repo.as_deref(),
        Some("lazyaix/prober-demo")
    );

    // 2. Without origin/HEAD, packed-refs containing refs/heads/main beats feature HEAD.
    let tmp2 = TempDir::new().unwrap();
    let root2 = tmp2.path();
    write_atomic(
        &root2.join(".git/HEAD"),
        "ref: refs/heads/feat/local-only\n",
    )
    .unwrap();
    write_atomic(
        &root2.join(".git/packed-refs"),
        "# pack-refs with: peeled fully-peeled\n0123456789abcdef0123456789abcdef01234567 refs/heads/main\n",
    )
    .unwrap();
    let facts2 = probe_repository(root2).unwrap();
    assert_eq!(facts2.git_default_branch.as_deref(), Some("main"));

    // 3. Simulated git worktree using `.git` file + `commondir`.
    let tmp3 = TempDir::new().unwrap();
    let main_git = tmp3.path().join("repo/.git");
    let wt_git = main_git.join("worktrees/task-a");
    let wt_dir = tmp3.path().join("repo-wt-task-a");
    write_atomic(
        &main_git.join("refs/remotes/origin/HEAD"),
        "ref: refs/remotes/origin/main\n",
    )
    .unwrap();
    write_atomic(
        &main_git.join("config"),
        "[remote \"origin\"]\n\turl = git@github.com:lazyaix/wt-demo.git\n",
    )
    .unwrap();
    write_atomic(&wt_git.join("HEAD"), "ref: refs/heads/agent/task-a\n").unwrap();
    write_atomic(&wt_git.join("commondir"), "../..\n").unwrap();
    write_atomic(
        &wt_dir.join(".git"),
        &format!("gitdir: {}\n", wt_git.display()),
    )
    .unwrap();

    let wt_facts = probe_repository(&wt_dir).unwrap();
    assert_eq!(wt_facts.git_default_branch.as_deref(), Some("main"));
    assert_eq!(wt_facts.git_remote_repo.as_deref(), Some("lazyaix/wt-demo"));

    // 4. Multi-language repo detection (TS + Python + Go).
    let tmp4 = TempDir::new().unwrap();
    let poly = tmp4.path();
    write_atomic(&poly.join("package.json"), "{}\n").unwrap();
    write_atomic(&poly.join("pnpm-lock.yaml"), "lockfileVersion: '9.0'\n").unwrap();
    write_atomic(&poly.join("pyproject.toml"), "[project]\nname = \"py\"\n").unwrap();
    write_atomic(&poly.join("go.mod"), "module example.com/demo\n").unwrap();
    let poly_facts = probe_repository(poly).unwrap();
    assert!(poly_facts.has_language(DetectedLanguage::TypeScript));
    assert!(poly_facts.has_language(DetectedLanguage::Python));
    assert!(poly_facts.has_language(DetectedLanguage::Go));
    assert_eq!(poly_facts.commands.test.as_deref(), Some("pnpm test"));
}

#[test]
fn discover_workspace_root_walks_ancestors_and_stops_at_git_boundary() {
    let tmp = TempDir::new().unwrap();
    let outer = tmp.path().join("outer");
    let inner_uninit = outer.join("nested-git-repo");

    let facts = probe_repository(&outer).unwrap_or_default();
    let manifest = build_initial_manifest(&facts, GitOutputMode::Commit);
    save_manifest(&outer, &manifest, false).unwrap();
    write_atomic(&outer.join(".agents/hooks.json"), "{}\n").unwrap();
    write_atomic(&outer.join("crates/la-core/src/lib.rs"), "// lib\n").unwrap();

    // Discovers outer root from `.agents` dir, deeply nested dirs, and file paths.
    assert_eq!(
        discover_workspace_root(&outer.join(".agents")),
        Some(outer.clone())
    );
    assert_eq!(
        discover_workspace_root(&outer.join("crates/la-core/src")),
        Some(outer.clone())
    );
    assert_eq!(
        discover_workspace_root(&outer.join("crates/la-core/src/lib.rs")),
        Some(outer.clone())
    );

    // Stops at an uninitialized inner Git repository boundary without leaking into outer.
    fs::create_dir_all(inner_uninit.join(".git")).unwrap();
    fs::create_dir_all(inner_uninit.join("src")).unwrap();
    assert_eq!(discover_workspace_root(&inner_uninit.join("src")), None);
}

#[test]
fn importer_handles_subagents_hooks_manifests_and_rejects_reserved_sources() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    write_atomic(&root.join("GEMINI.md"), "# Gemini Root\n").unwrap();
    write_atomic(
        &root.join(".agents/agents/reviewer.md"),
        "---\nname: reviewer\ndescription: Code reviewer.\nmodel: pro\nmainAgent: true\nsubagent: true\n---\nReview carefully.\n",
    )
    .unwrap();
    write_atomic(
        &root.join(".agents/hooks.json"),
        r#"{"custom-lint":{"PreToolUse":[{"matcher":"run_command","hooks":[{"type":"command","command":"./scripts/lint.sh","timeout":20}]}]}}"#,
    )
    .unwrap();
    write_atomic(
        &root.join(".agents/skills.json"),
        r#"{"entries":[{"path":"tools/skills","exclude":["wip-*"]}]}"#,
    )
    .unwrap();

    let imported = import_workspace(root, ImportSource::Antigravity).unwrap();
    assert_eq!(imported.manifest.agents.len(), 1);
    assert_eq!(imported.manifest.agents[0].name.as_str(), "reviewer");
    assert_eq!(imported.manifest.hooks.len(), 1);
    assert_eq!(imported.manifest.hooks[0].name.as_str(), "custom-lint");
    assert_eq!(imported.manifest.manifests.skills.entries.len(), 1);

    assert_matches!(
        import_workspace(root, ImportSource::ClaudeCode),
        Err(RuntimeError::Core(_))
    );
    assert_matches!(
        import_workspace(root, ImportSource::Codex),
        Err(RuntimeError::Core(_))
    );
}

#[test]
fn writer_dry_run_managed_regions_stale_delete_and_pack_loader_errors() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    write_atomic(&root.join(".github/CODEOWNERS"), "* @default\n").unwrap();
    let mut manifest =
        build_initial_manifest(&probe_repository(root).unwrap(), GitOutputMode::Commit);
    manifest.engineering.protected_paths.codeowners_sync = true;
    manifest
        .engineering
        .protected_paths
        .entries
        .push(ProtectedPathEntry {
            glob: ".lazyagents/**".to_owned(),
            owners: vec!["@sec".to_owned()],
            decision: None,
        });
    save_manifest(root, &manifest, false).unwrap();

    let ws = load_workspace(root, true).unwrap();
    let (_, plan) = ws.compile_and_plan(None).unwrap();

    // dry_run reports planned changes without mutating disk.
    let dry = apply_write_plan(
        root,
        &plan,
        ApplyOptions {
            force: false,
            dry_run: true,
        },
    )
    .unwrap();
    assert!(dry.created.iter().any(|p| p == "AGENTS.md"));
    assert!(!root.join("AGENTS.md").exists());

    // Apply writes managed block while preserving existing CODEOWNERS lines.
    apply_write_plan(root, &plan, ApplyOptions::default()).unwrap();
    let codeowners = fs::read_to_string(root.join(".github/CODEOWNERS")).unwrap();
    assert!(codeowners.contains("* @default"));
    assert!(codeowners.contains("/.lazyagents/ @sec"));

    // Turning off worktree skill deletes the previously managed stale skill file cleanly.
    manifest.engineering.git.worktree.enabled = false;
    save_manifest(root, &manifest, false).unwrap();
    let ws_after = load_workspace(root, true).unwrap();
    let (_, delete_plan) = ws_after.compile_and_plan(None).unwrap();
    let applied_del = apply_write_plan(root, &delete_plan, ApplyOptions::default()).unwrap();
    assert!(
        applied_del
            .deleted
            .iter()
            .any(|p| p == ".agents/skills/worktree-isolation/SKILL.md")
    );
    assert!(
        !root
            .join(".agents/skills/worktree-isolation/SKILL.md")
            .exists()
    );

    // Local pack loader error diagnostics.
    assert_matches!(
        load_local_packs(
            root,
            &[PackRef {
                id: ResourceId::parse("missing").unwrap(),
                source: PackSource::Local {
                    path: "does-not-exist".to_owned(),
                },
                include_only: Vec::new(),
                exclude: Vec::new(),
            }]
        ),
        Err(RuntimeError::Pack { .. })
    );
    assert_matches!(
        load_local_packs(
            root,
            &[PackRef {
                id: ResourceId::parse("remote").unwrap(),
                source: PackSource::Git {
                    url: "https://github.com/org/packs.git".to_owned(),
                    rev: Some("v1.0.0".to_owned()),
                },
                include_only: Vec::new(),
                exclude: Vec::new(),
            }]
        ),
        Err(RuntimeError::Pack { .. })
    );
    assert_matches!(
        load_local_packs(
            root,
            &[PackRef {
                id: ResourceId::parse("reg").unwrap(),
                source: PackSource::Registry {
                    package: "@org/core".to_owned(),
                    version: None,
                },
                include_only: Vec::new(),
                exclude: Vec::new(),
            }]
        ),
        Err(RuntimeError::Pack { .. })
    );

    // Editor command resolver always yields a non-empty program name.
    let editor = resolve_editor_command();
    assert_ne!(editor.program.trim(), "");
}
