//! Cross-target shared compilation helpers (`render_instruction_section`,
//! `collect_effective_hooks`, `collect_effective_mcp`) reused by Antigravity and M4-A adapters.

use std::collections::BTreeSet;

use crate::catalog::{builtin_github_mcp_preset, builtin_section_template};
use crate::compiler::CompileInputs;
use crate::engineering::StopHookMode;
use crate::error::CoreError;
use crate::id::{HookEvent, PolicyDecision, ResourceId, ToolCapability};
use crate::manifest::{
    HookMatcher, HookSpec, InstructionSection, LintLevel, McpServerSpec, ProjectManifest,
};
use crate::template::{TemplateContext, TemplateEngine};

/// Renders one modular `AGENTS.md` / instruction section after evaluating any `when` predicate.
pub(crate) fn render_instruction_section(
    section: &InstructionSection,
    inputs: &CompileInputs<'_>,
    ctx: &TemplateContext<'_>,
) -> Result<Option<String>, CoreError> {
    let when_expr = match section {
        InstructionSection::Preset { when, .. }
        | InstructionSection::File { when, .. }
        | InstructionSection::Inline { when, .. } => when.as_deref(),
    };
    if let Some(expr) = when_expr
        && !TemplateEngine::eval_condition(expr, ctx)?
    {
        return Ok(None);
    }

    match section {
        InstructionSection::Preset { preset, .. } => {
            let tpl = builtin_section_template(preset).ok_or_else(|| CoreError::Compile {
                message: format!("unknown built-in instruction preset `{preset}`"),
            })?;
            TemplateEngine::render(preset, tpl, ctx).map(Some)
        }
        InstructionSection::File { file, .. } => {
            let src = inputs
                .source_files
                .get(file)
                .ok_or_else(|| CoreError::Compile {
                    message: format!("missing instruction section source `{file}`"),
                })?;
            TemplateEngine::render(file, src, ctx).map(Some)
        }
        InstructionSection::Inline { inline, .. } => {
            TemplateEngine::render("inline-section", inline, ctx).map(Some)
        }
    }
}

/// Collects synthesized built-in guardrail hooks from `[engineering]` followed by user-authored hooks.
#[must_use]
pub(crate) fn collect_effective_hooks(manifest: &ProjectManifest) -> Vec<HookSpec> {
    let mut hooks = Vec::new();
    let existing_names: BTreeSet<&str> = manifest.hooks.iter().map(|h| h.name.as_str()).collect();

    let git = &manifest.engineering.git.guardrails;
    let git_guard_active = git.block_push_default_branch != PolicyDecision::Allow
        || git.block_force_push != PolicyDecision::Allow
        || git.block_destructive_reset != PolicyDecision::Allow
        || git.guard_broad_staging != PolicyDecision::Allow;

    if git_guard_active && !existing_names.contains("lazyagents-git-guard") {
        hooks.push(HookSpec {
            name: ResourceId::from_static_valid("lazyagents-git-guard"),
            enabled: true,
            event: HookEvent::PreToolUse,
            matcher: Some(HookMatcher::Capability {
                capability: ToolCapability::Shell,
            }),
            command: "lazyagents hook eval git-guard".to_owned(),
            timeout_secs: 15,
        });
    }

    if !manifest.engineering.protected_paths.entries.is_empty()
        && !existing_names.contains("lazyagents-protected-paths")
    {
        hooks.push(HookSpec {
            name: ResourceId::from_static_valid("lazyagents-protected-paths"),
            enabled: true,
            event: HookEvent::PreToolUse,
            matcher: Some(HookMatcher::Capability {
                capability: ToolCapability::FileWrite,
            }),
            command: "lazyagents hook eval protected-paths".to_owned(),
            timeout_secs: 15,
        });
    }

    if manifest.lint.secrets != LintLevel::Off
        && !existing_names.contains("lazyagents-secret-guard")
    {
        hooks.push(HookSpec {
            name: ResourceId::from_static_valid("lazyagents-secret-guard"),
            enabled: true,
            event: HookEvent::PreToolUse,
            matcher: Some(HookMatcher::Capability {
                capability: ToolCapability::FileWrite,
            }),
            command: "lazyagents hook eval secret-guard".to_owned(),
            timeout_secs: 15,
        });
    }

    if manifest.engineering.verify.stop_hook_mode != StopHookMode::Off
        && !manifest.engineering.verify.commands.is_empty()
        && !existing_names.contains("lazyagents-verify-gate")
    {
        hooks.push(HookSpec {
            name: ResourceId::from_static_valid("lazyagents-verify-gate"),
            enabled: true,
            event: HookEvent::Stop,
            matcher: None,
            command: "lazyagents hook eval verify-gate".to_owned(),
            timeout_secs: 15,
        });
    }

    if manifest.engineering.shell_guard.is_enabled()
        && !existing_names.contains("lazyagents-shell-guard")
    {
        hooks.push(HookSpec {
            name: ResourceId::from_static_valid("lazyagents-shell-guard"),
            enabled: true,
            event: HookEvent::PreToolUse,
            matcher: Some(HookMatcher::Capability {
                capability: ToolCapability::Shell,
            }),
            command: "lazyagents hook eval shell-guard".to_owned(),
            timeout_secs: 15,
        });
    }

    if manifest.engineering.format.enabled
        && !manifest.engineering.format.rules.is_empty()
        && !existing_names.contains("lazyagents-auto-format")
    {
        hooks.push(HookSpec {
            name: ResourceId::from_static_valid("lazyagents-auto-format"),
            enabled: true,
            event: HookEvent::PostToolUse,
            matcher: Some(HookMatcher::Capability {
                capability: ToolCapability::FileWrite,
            }),
            command: "lazyagents hook eval auto-format".to_owned(),
            timeout_secs: 15,
        });
    }

    hooks.extend(manifest.hooks.iter().cloned());
    hooks
}

/// Collects configured MCP servers plus the opt-in built-in Streamable HTTP GitHub MCP preset.
#[must_use]
pub(crate) fn collect_effective_mcp(manifest: &ProjectManifest) -> Vec<McpServerSpec> {
    let mut servers = manifest.mcp_servers.clone();
    let has_github = servers.iter().any(|s| s.name.as_str() == "github");
    if manifest.engineering.github.enabled
        && manifest.engineering.github.enable_mcp_preset
        && !has_github
    {
        servers.push(builtin_github_mcp_preset());
    }
    servers
}
