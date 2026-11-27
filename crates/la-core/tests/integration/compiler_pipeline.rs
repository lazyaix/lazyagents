//! End-to-end Sans-I/O compilation, catalog rendering, lockfile drift,
//! and managed-region diff planning black-box tests.

use std::collections::BTreeMap;

use la_core::catalog::{builtin_github_scaffold, builtin_section_template, list_section_presets};
use la_core::compiler::CompatibilitySeverity;
use la_core::engineering::GitHubScaffold;
use la_core::{
    CompileInputs, DetectedLanguage, FileAction, ProjectFacts, ProjectManifest, TargetKind,
    TemplateContext, TemplateEngine, compile_all, plan_writes, scan_secrets,
};

#[test]
fn all_builtin_presets_and_scaffolds_render_cleanly_without_template_errors() {
    let mut facts = ProjectFacts {
        name: "lazyagents".to_owned(),
        languages: vec![DetectedLanguage::Rust],
        toolchain_version: Some("1.100.0".to_owned()),
        language_edition: Some("2024".to_owned()),
        ..ProjectFacts::default()
    };
    facts.refresh_flags();
    let manifest = ProjectManifest::default();
    let ctx = TemplateContext {
        project: &facts,
        engineering: &manifest.engineering,
    };

    for preset in list_section_presets() {
        let tpl = builtin_section_template(preset.id).unwrap();
        let rendered = TemplateEngine::render(preset.id, tpl, &ctx).unwrap();
        assert_ne!(rendered.trim(), "");
        assert_eq!(scan_secrets(&rendered), Vec::new());
    }

    for scaffold in [
        GitHubScaffold::PrTitle,
        GitHubScaffold::SecurityZizmor,
        GitHubScaffold::Dependabot,
        GitHubScaffold::IssueTemplates,
        GitHubScaffold::Contributing,
        GitHubScaffold::SecurityPolicy,
    ] {
        let (path, tpl) = builtin_github_scaffold(scaffold);
        let rendered = TemplateEngine::render(path, tpl, &ctx).unwrap();
        assert_ne!(rendered.trim(), "");
        assert_eq!(scan_secrets(&rendered), Vec::new());
    }
}

const FULL_COMPILE_MANIFEST_TOML: &str = r#"
schema_version = 1

[project]
name = "lazyagents"
git_mode = "commit"

[targets.antigravity]
enabled = true

[targets.claude_code]
enabled = true

[engineering.protected_paths]
default_decision = "force_ask"
codeowners_sync = true
entries = [
  { glob = ".lazyagents/**", owners = ["@handoffx"] },
  { glob = ".agents/**", owners = ["@handoffx"] },
]

[engineering.verify]
stop_hook_mode = "remind"
commands = [
  { id = "fmt", label = "Format", cmd = "cargo fmt --all -- --check", required = true },
  { id = "test", label = "Test", cmd = "cargo test --workspace --all-features", required = true },
]

[engineering.github]
enabled = true
repo = "lazyaix/lazyagents"
enable_mcp_preset = true
enable_skills = ["pr-author", "ci-remediation"]
scaffolds = ["pr-title"]

[engineering.github.sync]
ci_workflow = true
pr_template = true
lint_workflows = true

[engineering.architecture]
[[engineering.architecture.layers]]
name = "la-core"
glob = "crates/la-core/**"
rule = "Pure Sans-I/O domain crate."

[[instructions]]
output = "AGENTS.md"
sections = [
  { preset = "builtin:engineering/git-workflow" },
  { preset = "builtin:engineering/github-hygiene" },
  { preset = "builtin:engineering/verify-and-supply-chain" },
  { preset = "builtin:lang/rust/core" },
  { file = "sections/extra.md", when = "project.has_rust" },
]

[[rules]]
id = "rust-safety"
source = "rules/rust-safety.md"
trigger = "glob"
globs = ["crates/**/*.{rs,toml}"]
description = "Rust safety guidelines."

[[skills]]
name = "custom-skill"
source = "skills/custom-skill"

[manifests.skills]
entries = [{ path = "tools/shared-skills" }]

[[agents]]
name = "auditor"
source = "agents/auditor.md"
description = "Read-only security auditor."
model = "pro"
capabilities = ["file_read", "search"]

[[plugins]]
name = "repo-kit"
display_name = "Repo Kit"
version = "1.0.0"
description = "Cohesive repository bundle."
include_rules = ["rust-safety"]
include_skills = ["custom-skill"]
include_agents = ["auditor"]
include_hooks = ["lazyagents-git-guard"]
include_mcp = ["github"]
"#;

#[test]
fn full_antigravity_and_github_compile_then_plan_writes_roundtrip() {
    let manifest = ProjectManifest::from_toml_str(FULL_COMPILE_MANIFEST_TOML).unwrap();
    let mut facts = ProjectFacts {
        name: "lazyagents".to_owned(),
        languages: vec![DetectedLanguage::Rust],
        toolchain_version: Some("1.100.0".to_owned()),
        language_edition: Some("2024".to_owned()),
        ..ProjectFacts::default()
    };
    facts.refresh_flags();

    let mut sources = BTreeMap::new();
    sources.insert(
        "sections/extra.md".to_owned(),
        "## Extra\nRepo: `{{ project.name }}`.\n".to_owned(),
    );
    sources.insert(
        "rules/rust-safety.md".to_owned(),
        "# Rust Safety\nNever use `unsafe`.\n".to_owned(),
    );
    sources.insert(
        "skills/custom-skill/SKILL.md".to_owned(),
        "---\nname: custom-skill\ndescription: Custom workflow.\n---\n\n# Custom\n".to_owned(),
    );
    sources.insert(
        "agents/auditor.md".to_owned(),
        "Audit security invariants strictly read-only.\n".to_owned(),
    );

    let inputs = CompileInputs {
        facts,
        source_files: sources,
        target_filter: None,
    };

    let output = compile_all(&manifest, &inputs).unwrap();

    // Reserved Claude Code target emits an explicit Unsupported diagnostic.
    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(output.diagnostics[0].target, TargetKind::ClaudeCode);
    assert_eq!(
        output.diagnostics[0].severity,
        CompatibilitySeverity::Unsupported
    );

    let by_path: BTreeMap<&str, &str> = output
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.content.as_str()))
        .collect();

    // Verify all 8 Antigravity asset types + .github/* governance outputs.
    assert!(by_path["AGENTS.md"].contains("cargo test --workspace --all-features"));
    assert!(by_path["AGENTS.md"].contains("Repo: `lazyagents`."));
    assert!(
        by_path[".agents/rules/rust-safety.md"]
            .contains("globs: **/crates/**/*.rs, **/crates/**/*.toml")
    );
    assert!(by_path[".agents/rules/arch-la-core.md"].contains("Pure Sans-I/O domain crate."));
    assert!(by_path.contains_key(".agents/skills/custom-skill/SKILL.md"));
    assert!(by_path.contains_key(".agents/skills/worktree-isolation/SKILL.md"));
    assert!(by_path.contains_key(".agents/skills/pr-author/SKILL.md"));
    assert!(by_path.contains_key(".agents/skills/ci-remediation/SKILL.md"));
    assert!(by_path[".agents/agents/auditor.md"].contains("- read_file"));
    assert!(by_path[".agents/hooks.json"].contains("lazyagents hook eval git-guard"));
    assert!(by_path[".agents/hooks.json"].contains("lazyagents hook eval protected-paths"));
    assert!(by_path[".agents/hooks.json"].contains("lazyagents hook eval secret-guard"));
    assert!(by_path[".agents/hooks.json"].contains("lazyagents hook eval verify-gate"));
    assert!(by_path[".agents/mcp_config.json"].contains("GITHUB_MCP_TOKEN"));
    assert!(by_path.contains_key(".agents/skills.json"));
    assert!(
        by_path[".agents/plugins/repo-kit/plugin.json"].contains("\"displayName\": \"Repo Kit\"")
    );
    assert!(by_path.contains_key(".agents/plugins/repo-kit/hooks.json"));
    assert!(by_path.contains_key(".agents/plugins/repo-kit/rules/AGENTS.md"));
    assert!(by_path[".github/workflows/lazyagents.yml"].contains("permissions: {}"));
    assert!(by_path[".github/CODEOWNERS"].contains("/.lazyagents/ @handoffx"));
    assert!(
        by_path[".github/pull_request_template.md"].contains("Human-reviewed and verified locally")
    );
    assert!(by_path.contains_key(".github/workflows/pr-title.yml"));
    assert!(by_path[".gitignore"].contains(".lazyagents/local.toml"));

    // Verify zero secrets in any compiled artifact.
    for file in &output.files {
        assert_eq!(
            scan_secrets(&file.content),
            Vec::new(),
            "unexpected secret finding in {}",
            file.path
        );
    }

    // Verify plan_writes -> write -> zero-drift cycle with pre-existing custom CODEOWNERS.
    let mut disk = BTreeMap::new();
    disk.insert(
        ".github/CODEOWNERS".to_owned(),
        "* @existing-owner\n".to_owned(),
    );

    let plan = plan_writes(&output.files, None, &disk).unwrap();
    assert!(plan.has_drift());
    assert!(!plan.has_conflicts());

    for file_plan in &plan.files {
        if let Some(content) = &file_plan.final_content {
            disk.insert(file_plan.path.clone(), content.clone());
        }
    }
    assert!(disk[".github/CODEOWNERS"].contains("* @existing-owner"));
    assert!(disk[".github/CODEOWNERS"].contains("/.lazyagents/ @handoffx"));

    let clean_plan = plan_writes(&output.files, Some(&plan.next_lock), &disk).unwrap();
    assert!(!clean_plan.has_drift());
    assert!(!clean_plan.has_conflicts());

    // Simulate external hand edit on AGENTS.md -> detected as ExternalConflict.
    disk.insert("AGENTS.md".to_owned(), "# hand edited\n".to_owned());
    let drifted = plan_writes(&output.files, Some(&plan.next_lock), &disk).unwrap();
    assert!(drifted.has_drift());
    assert!(drifted.has_conflicts());
    let agents_plan = drifted
        .files
        .iter()
        .find(|f| f.path == "AGENTS.md")
        .unwrap();
    assert_eq!(agents_plan.action, FileAction::ExternalConflict);
}
