//! Black-box integration tests covering `LA001`–`LA017` compliance linters,
//! GitHub Actions security auditing, budget telemetry, and `hook_eval` policy engine.

use std::collections::BTreeMap;

use la_core::compiler::GeneratedFile;
use la_core::engineering::{ProtectedPathEntry, VerifyCommand};
use la_core::manifest::{McpServerSpec, McpToolLoading, McpTransport, PluginSpec};
use la_core::{
    CompileInputs, CompileOutput, DiagnosticSeverity, HookEvalContext, HookGuardKind, HookProtocol,
    KebabName, LintCode, LintInputs, PolicyDecision, ProjectFacts, ProjectManifest, ResourceId,
    compile_all, evaluate_hook_json, lint_project, plan_writes,
};

#[test]
fn detects_github_actions_security_and_governance_violations_la014_to_la017() {
    let mut manifest = ProjectManifest::default();
    manifest.engineering.github.enabled = true;
    manifest.engineering.github.sync.lint_workflows = true;
    manifest
        .engineering
        .protected_paths
        .entries
        .push(ProtectedPathEntry {
            glob: ".lazyagents/**".to_owned(),
            owners: vec!["@handoffx".to_owned()],
            decision: Some(PolicyDecision::Deny),
        });
    manifest.engineering.verify.commands.push(VerifyCommand {
        id: ResourceId::parse("test").unwrap(),
        label: "Test".to_owned(),
        cmd: "cargo test --workspace".to_owned(),
        required: true,
    });

    let vulnerable_workflow = r"name: Vulnerable
on:
  pull_request_target:
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          ref: ${{ github.event.pull_request.head.sha }}
      - run: |
          echo ${{ github.event.pull_request.title }}
";

    let mut workspace = BTreeMap::new();
    workspace.insert(
        ".github/workflows/vuln.yml".to_owned(),
        vulnerable_workflow.to_owned(),
    );
    workspace.insert(".github/CODEOWNERS".to_owned(), "* @someone\n".to_owned());
    workspace.insert(
        ".github/pull_request_template.md".to_owned(),
        "## Summary\n".to_owned(),
    );

    let report = lint_project(&LintInputs {
        manifest_toml: "schema_version = 1\n",
        manifest: &manifest,
        source_files: &BTreeMap::new(),
        compile_output: &CompileOutput::default(),
        write_plan: None,
        workspace_files: &workspace,
    });

    let codes: Vec<LintCode> = report.diagnostics.iter().map(|d| d.code).collect();
    assert!(codes.contains(&LintCode::La014));
    assert!(codes.contains(&LintCode::La015));
    assert!(codes.contains(&LintCode::La016));
    assert!(codes.contains(&LintCode::La017));
    assert!(report.has_errors(false));

    let annotations = report.format_github_annotations();
    assert!(annotations.contains("::error file=.github/workflows/vuln.yml,title=LA014"));
    assert!(annotations.contains("title=LA016"));
    assert_ne!(report.to_json_pretty().unwrap(), "");
}

#[test]
fn detects_antigravity_spec_pitfalls_la001_through_la013() {
    let mut manifest = ProjectManifest::default();
    manifest.targets.claude_code.enabled = true;
    manifest.mcp_servers.push(McpServerSpec {
        name: ResourceId::parse("legacy-srv").unwrap(),
        enabled: true,
        transport: McpTransport::LegacySse {
            url: "https://mcp.example.com/sse".to_owned(),
            headers: BTreeMap::from([(
                "Authorization".to_owned(),
                "Bearer ${UNDECLARED_TOKEN}".to_owned(),
            )]),
        },
        variables: Vec::new(),
        tool_loading: McpToolLoading::Eager,
        allowed_tools: Vec::new(),
        denied_tools: Vec::new(),
        risk_surfaces: Vec::new(),
    });
    manifest.mcp_servers.push(McpServerSpec {
        name: ResourceId::parse("plugin-stdio").unwrap(),
        enabled: true,
        transport: McpTransport::Stdio {
            command: "node".to_owned(),
            args: Vec::new(),
            env: BTreeMap::from([("API_KEY".to_owned(), "${API_KEY}".to_owned())]),
            cwd: Some("./relative".to_owned()),
        },
        variables: Vec::new(),
        tool_loading: McpToolLoading::Eager,
        allowed_tools: Vec::new(),
        denied_tools: Vec::new(),
        risk_surfaces: Vec::new(),
    });
    manifest.plugins.push(PluginSpec {
        name: KebabName::parse("bad-plugin").unwrap(),
        display_name: None,
        version: None,
        author: Some("LazyAIX".to_owned()),
        homepage: Some("http://insecure.example.com".to_owned()),
        repository: Some("http://insecure.example.com/repo".to_owned()),
        description: "Test plugin".to_owned(),
        logo: Some("https://evil.example.com/logo.png".to_owned()),
        suggested_prompts: vec!["1".into(), "2".into(), "3".into(), "4".into()],
        enabled: true,
        include_rules: Vec::new(),
        include_skills: Vec::new(),
        include_agents: Vec::new(),
        include_hooks: Vec::new(),
        include_mcp: vec![ResourceId::parse("plugin-stdio").unwrap()],
    });

    let leaked_ghs = format!("ghs_{}.{}", "A".repeat(25), "B".repeat(25));
    let manifest_toml = format!("schema_version = 1\n# leaked {leaked_ghs}\n");

    let mut compile_output = CompileOutput::default();
    compile_output
        .files
        .push(GeneratedFile::whole_file("AGENTS.md", "x".repeat(25_000)));
    compile_output.files.push(GeneratedFile::whole_file(
        ".agents/skills/my-skill/SKILL.md",
        "---\nname: wrong-name\ndescription:\n---\n# Body\n",
    ));
    compile_output
        .diagnostics
        .push(la_core::CompatibilityDiagnostic {
            target: la_core::TargetKind::ClaudeCode,
            severity: la_core::CompatibilitySeverity::Unsupported,
            subject: "target:claude_code".to_owned(),
            message: "reserved".to_owned(),
        });

    let plan = plan_writes(&compile_output.files, None, &BTreeMap::new()).unwrap();
    let mut workspace = BTreeMap::new();
    workspace.insert(
        ".agents/plugins/bad-plugin/hooks/hooks.json".to_owned(),
        "{}".to_owned(),
    );

    let report = lint_project(&LintInputs {
        manifest_toml: &manifest_toml,
        manifest: &manifest,
        source_files: &BTreeMap::new(),
        compile_output: &compile_output,
        write_plan: Some(&plan),
        workspace_files: &workspace,
    });

    let codes: Vec<LintCode> = report.diagnostics.iter().map(|d| d.code).collect();
    for expected in [
        LintCode::La001,
        LintCode::La002,
        LintCode::La004,
        LintCode::La006,
        LintCode::La007,
        LintCode::La008,
        LintCode::La010,
        LintCode::La012,
        LintCode::La013,
    ] {
        assert!(
            codes.contains(&expected),
            "expected diagnostic {expected}, got {codes:?}"
        );
    }
    assert!(report.format_text().contains("error[LA001]"));
}

#[test]
fn compiled_lazyagents_workflow_passes_actions_security_linter_cleanly() {
    let mut manifest = ProjectManifest::default();
    manifest.project.name = "clean-repo".to_owned();
    manifest.engineering.github.enabled = true;
    manifest.engineering.github.sync.ci_workflow = true;
    manifest.engineering.github.sync.lint_workflows = true;

    let inputs = CompileInputs {
        facts: ProjectFacts::default(),
        source_files: BTreeMap::new(),
        target_filter: None,
    };
    let compiled = compile_all(&manifest, &inputs).unwrap();

    let mut workspace = BTreeMap::new();
    for file in &compiled.files {
        workspace.insert(file.path.clone(), file.content.clone());
    }

    let report = lint_project(&LintInputs {
        manifest_toml: "schema_version = 1\n",
        manifest: &manifest,
        source_files: &BTreeMap::new(),
        compile_output: &compiled,
        write_plan: None,
        workspace_files: &workspace,
    });

    let errors: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .collect();
    assert_eq!(errors, Vec::<&la_core::Diagnostic>::new());

    // Sanity-check hook_eval denies force push cleanly via public crate root API.
    let hook_json = evaluate_hook_json(
        HookGuardKind::GitGuard,
        HookProtocol::Antigravity,
        r#"{"toolCall":{"args":{"CommandLine":"git push --force origin main"}}}"#,
        &manifest,
        &HookEvalContext::default(),
    )
    .unwrap();
    assert!(hook_json.contains("\"decision\":\"deny\""));
}
