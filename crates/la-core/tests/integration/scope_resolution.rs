//! Four-layer scope resolution and organization `EnforceLock` black-box tests.

use core::assert_matches;

use la_core::manifest::LintLevel;
use la_core::{
    ConfigScope, CoreError, MergePolicy, PolicyDecision, ProjectManifest, ScopeResolver,
    ScopedLayer,
};

#[test]
fn minimal_project_local_preserves_project_engineering_and_merges_mcp() {
    let project = ProjectManifest::from_toml_str(
        r#"
        schema_version = 1
        [project]
        name = "lazyagents"

        [engineering.github]
        enabled = true
        repo = "lazyaix/lazyagents"

        [engineering.verify]
        commands = [
          { id = "test", label = "Test", cmd = "cargo test --workspace", required = true },
        ]
        "#,
    )
    .unwrap();

    let local = ProjectManifest::from_toml_str(
        r#"
        schema_version = 1
        [[mcp_servers]]
        name = "local-inspector"
        transport = "stdio"
        command = "inspector-mcp"
        "#,
    )
    .unwrap();

    let resolved = ScopeResolver::resolve(vec![
        ScopedLayer::new(ConfigScope::Project, MergePolicy::Append, project),
        ScopedLayer::new(ConfigScope::ProjectLocal, MergePolicy::Override, local),
    ])
    .unwrap();

    assert_eq!(resolved.manifest.project.name, "lazyagents");
    assert!(resolved.manifest.engineering.github.enabled);
    assert_eq!(resolved.manifest.engineering.verify.commands.len(), 1);
    assert_eq!(resolved.manifest.mcp_servers.len(), 1);
    assert_eq!(
        resolved.provenance.get("engineering"),
        Some(&ConfigScope::Project)
    );
    assert_eq!(
        resolved.provenance.get("mcp:local-inspector"),
        Some(&ConfigScope::ProjectLocal)
    );
}

#[test]
fn four_layer_stack_enforces_organization_lock_while_allowing_stricter_overrides() {
    let mut org = ProjectManifest::default();
    org.project.name = "org-policy".to_owned();
    org.lint.secrets = LintLevel::Error;
    org.engineering.git.guardrails.block_force_push = PolicyDecision::Deny;
    org.engineering.supply_chain.forbid_copyleft = true;

    let mut user = ProjectManifest::default();
    user.lint.compatibility_loss = LintLevel::Error;

    let mut project = ProjectManifest::default();
    project.project.name = "service-a".to_owned();
    project.lint.secrets = LintLevel::Error;
    project.engineering.git.guardrails.block_force_push = PolicyDecision::Deny;

    let ok = ScopeResolver::resolve(vec![
        ScopedLayer::new(ConfigScope::Project, MergePolicy::Override, project.clone()),
        ScopedLayer::new(
            ConfigScope::Organization,
            MergePolicy::EnforceLock,
            org.clone(),
        ),
        ScopedLayer::new(ConfigScope::User, MergePolicy::Append, user),
    ])
    .unwrap();

    assert_eq!(
        ok.active_scopes,
        vec![
            ConfigScope::Organization,
            ConfigScope::User,
            ConfigScope::Project
        ]
    );
    assert_eq!(ok.manifest.project.name, "service-a");

    let mut weakened = project;
    weakened.engineering.supply_chain.forbid_copyleft = false;
    let err = ScopeResolver::resolve(vec![
        ScopedLayer::new(ConfigScope::Organization, MergePolicy::EnforceLock, org),
        ScopedLayer::new(ConfigScope::Project, MergePolicy::Override, weakened),
    ]);
    assert_matches!(
        err,
        Err(CoreError::LockedPolicyViolation {
            setting,
            locked_by: ConfigScope::Organization,
            attempted: ConfigScope::Project,
        }) if setting == "engineering.supply_chain.forbid_copyleft"
    );
}
