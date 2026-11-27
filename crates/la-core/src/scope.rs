//! Four-layer scope resolution (`Organization -> User -> Project -> ProjectLocal`)
//! with provenance tracking and `EnforceLock` policy protection.

use core::fmt;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::engineering::{
    ArchitectureLayer, EngineeringPolicy, FormatPolicy, FormatRule, ProtectedPathEntry,
    ShellGuardPolicy, VerifyCommand,
};
use crate::error::CoreError;
use crate::id::PolicyDecision;
use crate::manifest::{
    HookSpec, IndexEntry, IndexManifestSpec, InstructionSpec, LintConfig, LintLevel, McpServerSpec,
    PackRef, PluginSpec, ProjectManifest, RuleSpec, ScopeSettings, SkillSpec, SubagentSpec,
    TargetConfigs,
};

/// Configuration precedence layer ordered from broadest (`Organization`) to narrowest (`ProjectLocal`).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigScope {
    /// Organization-wide baseline policy (reserved for post-V1; supports `EnforceLock`).
    Organization,
    /// Personal user defaults (`~/.config/lazyagents/`, reserved for post-V1).
    User,
    /// Repository-tracked project manifest (`.lazyagents/lazyagents.toml`, active in V1).
    Project,
    /// Gitignored developer-local override (`.lazyagents/local.toml`, optional in V1).
    ProjectLocal,
}

/// Strategy describing how a scope layer combines with adjacent layers.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergePolicy {
    /// Merge collections by deduplicating on resource identifier while appending new items.
    #[default]
    Append,
    /// Higher-precedence layer replaces matching resource identifiers and scalar settings.
    Override,
    /// Prevent lower-precedence scopes from weakening security guardrails or lint floors.
    EnforceLock,
}

/// One loaded manifest layer paired with its originating scope and merge semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopedLayer {
    /// Precedence tier of this layer.
    pub scope: ConfigScope,
    /// Merge semantics applied when layering subsequent scopes on top of this layer.
    pub policy: MergePolicy,
    /// Manifest payload contributed by this layer.
    pub manifest: ProjectManifest,
}

/// Resolved effective project configuration paired with per-resource scope provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedConfig {
    /// Final merged canonical manifest ready for linting and target compilation.
    pub manifest: ProjectManifest,
    /// Ordered scopes that participated in resolution.
    pub active_scopes: Vec<ConfigScope>,
    /// Winning scope provenance keyed by logical resource key (e.g., `"rule:rust-safety"`).
    pub provenance: BTreeMap<String, ConfigScope>,
}

/// Pure Sans-I/O scope resolver implementing the 4-layer merge hierarchy.
#[derive(Clone, Copy, Debug, Default)]
pub struct ScopeResolver;

impl ConfigScope {
    /// Returns the canonical human-readable scope identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Organization => "organization",
            Self::User => "user",
            Self::Project => "project",
            Self::ProjectLocal => "project_local",
        }
    }

    /// Returns `true` when this scope is active in V1 project-level mode.
    #[must_use]
    pub const fn is_active_in_v1(self) -> bool {
        matches!(self, Self::Project | Self::ProjectLocal)
    }
}

impl fmt::Display for ConfigScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl ScopedLayer {
    /// Creates a new [`ScopedLayer`].
    #[must_use]
    pub const fn new(scope: ConfigScope, policy: MergePolicy, manifest: ProjectManifest) -> Self {
        Self {
            scope,
            policy,
            manifest,
        }
    }
}

impl ScopeResolver {
    /// Resolves an arbitrary set of [`ScopedLayer`] entries into a single [`ResolvedConfig`].
    ///
    /// Layers are sorted from broadest (`Organization`) to narrowest (`ProjectLocal`).
    /// If any upper layer declares [`MergePolicy::EnforceLock`], lower layers are rejected with
    /// [`CoreError::LockedPolicyViolation`] whenever they attempt to weaken locked lint levels
    /// or destructive-Git guardrail decisions.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::ScopeResolution`] when `layers` is empty or contains duplicate scopes,
    /// or [`CoreError::LockedPolicyViolation`] when a lower scope weakens a locked security floor.
    pub fn resolve(mut layers: Vec<ScopedLayer>) -> Result<ResolvedConfig, CoreError> {
        if layers.is_empty() {
            return Err(CoreError::ScopeResolution {
                message: "at least one scope layer is required".to_owned(),
            });
        }

        layers.sort_by_key(|layer| layer.scope);
        ensure_distinct_scopes(&layers)?;

        let mut locked_floors: Vec<(ConfigScope, ProjectManifest)> = Vec::new();
        let mut provenance = BTreeMap::new();
        let mut active_scopes = Vec::with_capacity(layers.len());

        let mut iter = layers.into_iter();
        let first = iter.next().ok_or_else(|| CoreError::ScopeResolution {
            message: "at least one scope layer is required".to_owned(),
        })?;

        let mut merged = first.manifest.clone();
        record_provenance(&merged, first.scope, &mut provenance);
        active_scopes.push(first.scope);
        if first.policy == MergePolicy::EnforceLock {
            locked_floors.push((first.scope, first.manifest));
        }

        for layer in iter {
            for (locked_by, floor) in &locked_floors {
                verify_locked_floors(*locked_by, floor, layer.scope, &layer.manifest)?;
            }
            merge_manifest(&mut merged, &layer.manifest, layer.scope, &mut provenance);
            active_scopes.push(layer.scope);
            if layer.policy == MergePolicy::EnforceLock {
                locked_floors.push((layer.scope, layer.manifest));
            }
        }

        merged.validate_invariants()?;

        Ok(ResolvedConfig {
            manifest: merged,
            active_scopes,
            provenance,
        })
    }
}

fn ensure_distinct_scopes(layers: &[ScopedLayer]) -> Result<(), CoreError> {
    for [prev, next] in layers.array_windows() {
        if prev.scope == next.scope {
            return Err(CoreError::ScopeResolution {
                message: format!("duplicate scope layer `{}`", next.scope),
            });
        }
    }
    Ok(())
}

fn verify_locked_floors(
    locked_by: ConfigScope,
    floor: &ProjectManifest,
    attempted: ConfigScope,
    candidate: &ProjectManifest,
) -> Result<(), CoreError> {
    check_lint_floor(
        "lint.secrets",
        floor.lint.secrets,
        candidate.lint.secrets,
        locked_by,
        attempted,
    )?;
    check_lint_floor(
        "lint.github_actions_security",
        floor.lint.github_actions_security,
        candidate.lint.github_actions_security,
        locked_by,
        attempted,
    )?;
    check_lint_floor(
        "lint.prompt_injection",
        floor.lint.prompt_injection,
        candidate.lint.prompt_injection,
        locked_by,
        attempted,
    )?;
    check_decision_floor(
        "engineering.git.guardrails.block_push_default_branch",
        floor.engineering.git.guardrails.block_push_default_branch,
        candidate
            .engineering
            .git
            .guardrails
            .block_push_default_branch,
        locked_by,
        attempted,
    )?;
    check_decision_floor(
        "engineering.git.guardrails.block_force_push",
        floor.engineering.git.guardrails.block_force_push,
        candidate.engineering.git.guardrails.block_force_push,
        locked_by,
        attempted,
    )?;
    check_decision_floor(
        "engineering.git.guardrails.block_destructive_reset",
        floor.engineering.git.guardrails.block_destructive_reset,
        candidate.engineering.git.guardrails.block_destructive_reset,
        locked_by,
        attempted,
    )?;
    if floor.engineering.shell_guard.is_enabled() {
        if candidate.engineering.shell_guard.enabled == Some(false) {
            return Err(CoreError::LockedPolicyViolation {
                setting: "engineering.shell_guard.enabled".to_owned(),
                locked_by,
                attempted,
            });
        }
        check_decision_floor(
            "engineering.shell_guard.block_destructive_rm",
            floor.engineering.shell_guard.block_destructive_rm,
            candidate.engineering.shell_guard.block_destructive_rm,
            locked_by,
            attempted,
        )?;
        check_decision_floor(
            "engineering.shell_guard.block_pipe_to_shell",
            floor.engineering.shell_guard.block_pipe_to_shell,
            candidate.engineering.shell_guard.block_pipe_to_shell,
            locked_by,
            attempted,
        )?;
        check_decision_floor(
            "engineering.shell_guard.block_credential_reads",
            floor.engineering.shell_guard.block_credential_reads,
            candidate.engineering.shell_guard.block_credential_reads,
            locked_by,
            attempted,
        )?;
    }
    if floor.engineering.supply_chain.forbid_copyleft
        && !candidate.engineering.supply_chain.forbid_copyleft
    {
        return Err(CoreError::LockedPolicyViolation {
            setting: "engineering.supply_chain.forbid_copyleft".to_owned(),
            locked_by,
            attempted,
        });
    }
    Ok(())
}

fn check_lint_floor(
    setting: &str,
    floor: LintLevel,
    candidate: LintLevel,
    locked_by: ConfigScope,
    attempted: ConfigScope,
) -> Result<(), CoreError> {
    if candidate.is_at_least_as_strict_as(floor) {
        Ok(())
    } else {
        Err(CoreError::LockedPolicyViolation {
            setting: setting.to_owned(),
            locked_by,
            attempted,
        })
    }
}

fn check_decision_floor(
    setting: &str,
    floor: PolicyDecision,
    candidate: PolicyDecision,
    locked_by: ConfigScope,
    attempted: ConfigScope,
) -> Result<(), CoreError> {
    if candidate.is_at_least_as_strict_as(floor) {
        Ok(())
    } else {
        Err(CoreError::LockedPolicyViolation {
            setting: setting.to_owned(),
            locked_by,
            attempted,
        })
    }
}

fn record_provenance(
    manifest: &ProjectManifest,
    scope: ConfigScope,
    provenance: &mut BTreeMap<String, ConfigScope>,
) {
    provenance.insert("project".to_owned(), scope);
    provenance.insert("engineering".to_owned(), scope);
    for pack in &manifest.packs {
        provenance.insert(format!("pack:{}", pack.id), scope);
    }
    for inst in &manifest.instructions {
        provenance.insert(format!("instruction:{}", inst.output), scope);
    }
    for rule in &manifest.rules {
        provenance.insert(format!("rule:{}", rule.id), scope);
    }
    for skill in &manifest.skills {
        provenance.insert(format!("skill:{}", skill.name), scope);
    }
    for agent in &manifest.agents {
        provenance.insert(format!("agent:{}", agent.name), scope);
    }
    for hook in &manifest.hooks {
        provenance.insert(format!("hook:{}", hook.name), scope);
    }
    for mcp in &manifest.mcp_servers {
        provenance.insert(format!("mcp:{}", mcp.name), scope);
    }
    for plugin in &manifest.plugins {
        provenance.insert(format!("plugin:{}", plugin.name), scope);
    }
}

fn merge_manifest(
    base: &mut ProjectManifest,
    overlay: &ProjectManifest,
    scope: ConfigScope,
    provenance: &mut BTreeMap<String, ConfigScope>,
) {
    if !overlay.project.name.trim().is_empty() && overlay.project.name != "unnamed-project" {
        base.project = overlay.project.clone();
        provenance.insert("project".to_owned(), scope);
    }
    if overlay.scopes != ScopeSettings::default() {
        base.scopes = overlay.scopes.clone();
    }
    if overlay.targets != TargetConfigs::default() {
        base.targets = overlay.targets.clone();
    }
    if overlay.engineering != EngineeringPolicy::default() {
        merge_engineering(&mut base.engineering, &overlay.engineering);
        provenance.insert("engineering".to_owned(), scope);
    }
    if overlay.lint != LintConfig::default() {
        base.lint = overlay.lint.clone();
        provenance.insert("lint".to_owned(), scope);
    }

    merge_by_key(&mut base.packs, &overlay.packs, |p: &PackRef| p.id.as_str());
    merge_by_key(
        &mut base.instructions,
        &overlay.instructions,
        |i: &InstructionSpec| i.output.as_str(),
    );
    merge_by_key(&mut base.rules, &overlay.rules, |r: &RuleSpec| {
        r.id.as_str()
    });
    merge_by_key(&mut base.skills, &overlay.skills, |s: &SkillSpec| {
        s.name.as_str()
    });
    merge_by_key(&mut base.agents, &overlay.agents, |a: &SubagentSpec| {
        a.name.as_str()
    });
    merge_by_key(&mut base.hooks, &overlay.hooks, |h: &HookSpec| {
        h.name.as_str()
    });
    merge_by_key(
        &mut base.mcp_servers,
        &overlay.mcp_servers,
        |m: &McpServerSpec| m.name.as_str(),
    );
    merge_by_key(&mut base.plugins, &overlay.plugins, |p: &PluginSpec| {
        p.name.as_str()
    });

    merge_index_manifest(&mut base.manifests.skills, &overlay.manifests.skills);
    merge_index_manifest(&mut base.manifests.rules, &overlay.manifests.rules);
    merge_index_manifest(&mut base.manifests.plugins, &overlay.manifests.plugins);

    record_resource_provenance(overlay, scope, provenance);
}

fn merge_engineering(base: &mut EngineeringPolicy, overlay: &EngineeringPolicy) {
    if overlay.git != crate::engineering::GitPolicy::default() {
        base.git = overlay.git.clone();
    }
    if overlay.protected_paths != crate::engineering::ProtectedPathsPolicy::default() {
        base.protected_paths.default_decision = overlay.protected_paths.default_decision;
        base.protected_paths.codeowners_sync |= overlay.protected_paths.codeowners_sync;
        merge_by_key(
            &mut base.protected_paths.entries,
            &overlay.protected_paths.entries,
            |e: &ProtectedPathEntry| e.glob.as_str(),
        );
    }
    if overlay.verify != crate::engineering::VerifyPolicy::default() {
        base.verify.stop_hook_mode = overlay.verify.stop_hook_mode;
        merge_by_key(
            &mut base.verify.commands,
            &overlay.verify.commands,
            |c: &VerifyCommand| c.id.as_str(),
        );
    }
    if overlay.github != crate::engineering::GitHubPolicy::default() {
        base.github = overlay.github.clone();
    }
    if overlay.supply_chain != crate::engineering::SupplyChainPolicy::default() {
        base.supply_chain = overlay.supply_chain.clone();
    }
    if overlay.architecture != crate::engineering::ArchitecturePolicy::default() {
        merge_by_key(
            &mut base.architecture.docs,
            &overlay.architecture.docs,
            String::as_str,
        );
        merge_by_key(
            &mut base.architecture.layers,
            &overlay.architecture.layers,
            |l: &ArchitectureLayer| l.name.as_str(),
        );
    }
    if overlay.release != crate::engineering::ReleasePolicy::default() {
        base.release = overlay.release.clone();
    }
    if overlay.format != FormatPolicy::default() {
        base.format.enabled |= overlay.format.enabled;
        merge_by_key(
            &mut base.format.rules,
            &overlay.format.rules,
            |r: &FormatRule| r.id.as_str(),
        );
    }
    if overlay.shell_guard != ShellGuardPolicy::default() {
        if let Some(enabled) = overlay.shell_guard.enabled {
            base.shell_guard.enabled = Some(enabled);
        }
        base.shell_guard.block_destructive_rm = overlay.shell_guard.block_destructive_rm;
        base.shell_guard.block_pipe_to_shell = overlay.shell_guard.block_pipe_to_shell;
        base.shell_guard.block_credential_reads = overlay.shell_guard.block_credential_reads;
        merge_by_key(
            &mut base.shell_guard.extra_deny_patterns,
            &overlay.shell_guard.extra_deny_patterns,
            String::as_str,
        );
    }
}

fn record_resource_provenance(
    manifest: &ProjectManifest,
    scope: ConfigScope,
    provenance: &mut BTreeMap<String, ConfigScope>,
) {
    for pack in &manifest.packs {
        provenance.insert(format!("pack:{}", pack.id), scope);
    }
    for inst in &manifest.instructions {
        provenance.insert(format!("instruction:{}", inst.output), scope);
    }
    for rule in &manifest.rules {
        provenance.insert(format!("rule:{}", rule.id), scope);
    }
    for skill in &manifest.skills {
        provenance.insert(format!("skill:{}", skill.name), scope);
    }
    for agent in &manifest.agents {
        provenance.insert(format!("agent:{}", agent.name), scope);
    }
    for hook in &manifest.hooks {
        provenance.insert(format!("hook:{}", hook.name), scope);
    }
    for mcp in &manifest.mcp_servers {
        provenance.insert(format!("mcp:{}", mcp.name), scope);
    }
    for plugin in &manifest.plugins {
        provenance.insert(format!("plugin:{}", plugin.name), scope);
    }
}

fn merge_by_key<T, F>(base: &mut Vec<T>, overlay: &[T], key: F)
where
    T: Clone,
    F: Fn(&T) -> &str,
{
    for item in overlay {
        let item_key = key(item);
        if let Some(existing) = base.iter_mut().find(|cur| key(cur) == item_key) {
            *existing = item.clone();
        } else {
            base.push(item.clone());
        }
    }
}

fn merge_index_manifest(base: &mut IndexManifestSpec, overlay: &IndexManifestSpec) {
    merge_by_key(&mut base.inherits, &overlay.inherits, |e: &IndexEntry| {
        e.path.as_str()
    });
    merge_by_key(&mut base.entries, &overlay.entries, |e: &IndexEntry| {
        e.path.as_str()
    });
}

#[cfg(test)]
mod tests {
    use core::assert_matches;

    use crate::id::ResourceId;
    use crate::manifest::{McpAuthMode, McpServerSpec, McpToolLoading, McpTransport};

    use super::*;

    #[test]
    fn project_local_overrides_project_and_tracks_provenance() {
        let mut project = ProjectManifest::default();
        project.project.name = "lazyagents".to_owned();
        project.mcp_servers.push(McpServerSpec {
            name: ResourceId::parse("github").unwrap(),
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

        let mut local = ProjectManifest::default();
        local.mcp_servers.push(McpServerSpec {
            name: ResourceId::parse("local-debug").unwrap(),
            enabled: true,
            transport: McpTransport::Stdio {
                command: "debug-mcp".to_owned(),
                args: Vec::new(),
                env: BTreeMap::new(),
                cwd: None,
            },
            variables: Vec::new(),
            tool_loading: McpToolLoading::Eager,
            allowed_tools: Vec::new(),
            denied_tools: Vec::new(),
            risk_surfaces: Vec::new(),
        });

        let resolved = ScopeResolver::resolve(vec![
            ScopedLayer::new(ConfigScope::ProjectLocal, MergePolicy::Override, local),
            ScopedLayer::new(ConfigScope::Project, MergePolicy::Append, project),
        ])
        .unwrap();

        assert_eq!(resolved.manifest.project.name, "lazyagents");
        assert_eq!(resolved.manifest.mcp_servers.len(), 2);
        assert_eq!(
            resolved.active_scopes,
            vec![ConfigScope::Project, ConfigScope::ProjectLocal]
        );
        assert_eq!(
            resolved.provenance.get("mcp:github"),
            Some(&ConfigScope::Project)
        );
        assert_eq!(
            resolved.provenance.get("mcp:local-debug"),
            Some(&ConfigScope::ProjectLocal)
        );
    }

    #[test]
    fn organization_enforce_lock_blocks_weakening_secrets_or_git_guardrails() {
        let mut org = ProjectManifest::default();
        org.project.name = "org-baseline".to_owned();
        org.lint.secrets = LintLevel::Error;
        org.engineering.git.guardrails.block_force_push = PolicyDecision::Deny;

        let mut project = ProjectManifest::default();
        project.project.name = "app".to_owned();
        project.lint.secrets = LintLevel::Warn;

        let err = ScopeResolver::resolve(vec![
            ScopedLayer::new(ConfigScope::Organization, MergePolicy::EnforceLock, org),
            ScopedLayer::new(ConfigScope::Project, MergePolicy::Override, project),
        ]);
        assert_matches!(
            err,
            Err(CoreError::LockedPolicyViolation {
                locked_by: ConfigScope::Organization,
                attempted: ConfigScope::Project,
                ..
            })
        );
    }

    #[test]
    fn organization_enforce_lock_protects_prompt_injection_and_shell_guard() {
        let mut org = ProjectManifest::default();
        org.project.name = "org-baseline".to_owned();
        org.lint.prompt_injection = LintLevel::Error;
        org.engineering.shell_guard.enabled = Some(true);

        // Omitting [engineering.shell_guard] in project inherits the org lock cleanly.
        let mut clean_project = ProjectManifest::default();
        clean_project.project.name = "clean-app".to_owned();
        let resolved = ScopeResolver::resolve(vec![
            ScopedLayer::new(
                ConfigScope::Organization,
                MergePolicy::EnforceLock,
                org.clone(),
            ),
            ScopedLayer::new(ConfigScope::Project, MergePolicy::Override, clean_project),
        ])
        .unwrap();
        assert!(resolved.manifest.engineering.shell_guard.is_enabled());

        // Explicitly disabling shell_guard violates EnforceLock.
        let mut disable_shell = ProjectManifest::default();
        disable_shell.project.name = "bad-app".to_owned();
        disable_shell.engineering.shell_guard.enabled = Some(false);
        assert_matches!(
            ScopeResolver::resolve(vec![
                ScopedLayer::new(ConfigScope::Organization, MergePolicy::EnforceLock, org.clone()),
                ScopedLayer::new(ConfigScope::Project, MergePolicy::Override, disable_shell),
            ]),
            Err(CoreError::LockedPolicyViolation { setting, .. })
                if setting == "engineering.shell_guard.enabled"
        );

        // Weakening LA020 prompt_injection lint violates EnforceLock.
        let mut weaken_la020 = ProjectManifest::default();
        weaken_la020.project.name = "bad-app-2".to_owned();
        weaken_la020.lint.prompt_injection = LintLevel::Warn;
        assert_matches!(
            ScopeResolver::resolve(vec![
                ScopedLayer::new(ConfigScope::Organization, MergePolicy::EnforceLock, org),
                ScopedLayer::new(ConfigScope::Project, MergePolicy::Override, weaken_la020),
            ]),
            Err(CoreError::LockedPolicyViolation { setting, .. })
                if setting == "lint.prompt_injection"
        );
    }

    #[test]
    fn rejects_empty_or_duplicate_scope_layers() {
        assert_matches!(
            ScopeResolver::resolve(Vec::new()),
            Err(CoreError::ScopeResolution { .. })
        );

        let m = ProjectManifest::default();
        assert_matches!(
            ScopeResolver::resolve(vec![
                ScopedLayer::new(ConfigScope::Project, MergePolicy::Append, m.clone()),
                ScopedLayer::new(ConfigScope::Project, MergePolicy::Override, m),
            ]),
            Err(CoreError::ScopeResolution { .. })
        );
    }
}
