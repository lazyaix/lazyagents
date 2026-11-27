//! Google Antigravity V1 compiler orchestrating all 8 repository-checked-in customization
//! assets (`AGENTS.md`, rules, skills, subagents, hooks, MCP, index manifests, and plugins).

mod docs;
mod plugins;
mod runtime_assets;

use crate::compiler::shared::{collect_effective_hooks, collect_effective_mcp};
use crate::compiler::{CompileInputs, CompileOutput, GeneratedFile};
use crate::error::CoreError;
use crate::manifest::ProjectManifest;
use crate::template::TemplateContext;

use self::docs::{compile_instructions, compile_rules, compile_skills, compile_subagents};
use self::plugins::compile_plugins;
use self::runtime_assets::{compile_index_manifests, render_hooks_json, render_mcp_config_json};

/// Google Antigravity target compiler (V1 primary target).
#[derive(Clone, Copy, Debug, Default)]
pub struct AntigravityCompiler;

impl AntigravityCompiler {
    /// Compiles all enabled Antigravity project assets (`AGENTS.md`, `.agents/rules/*.md`,
    /// `.agents/skills/**`, `.agents/agents/*.md`, `.agents/hooks.json`, `.agents/mcp_config.json`,
    /// `.agents/{skills,rules,plugins}.json`, and `.agents/plugins/<name>/**`).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::TemplateRender`] or [`CoreError::Compile`] when templates or
    /// referenced source files cannot be resolved or serialized.
    pub fn compile(
        manifest: &ProjectManifest,
        inputs: &CompileInputs<'_>,
    ) -> Result<CompileOutput, CoreError> {
        let mut files = Vec::new();
        let mut diagnostics = Vec::new();
        let ctx = TemplateContext {
            project: &inputs.facts,
            engineering: &manifest.engineering,
        };

        compile_instructions(manifest, inputs, &ctx, &mut files)?;
        compile_rules(manifest, inputs, &mut files)?;
        compile_skills(manifest, inputs, &mut files)?;
        compile_subagents(manifest, inputs, &mut files)?;

        let effective_hooks = collect_effective_hooks(manifest);
        if !effective_hooks.is_empty() {
            let json = render_hooks_json(&effective_hooks)?;
            files.push(GeneratedFile::whole_file(".agents/hooks.json", json));
        }

        let effective_mcp = collect_effective_mcp(manifest);
        if !effective_mcp.is_empty() {
            let json = render_mcp_config_json(&effective_mcp, &mut diagnostics)?;
            files.push(GeneratedFile::whole_file(".agents/mcp_config.json", json));
        }

        compile_index_manifests(manifest, &mut files)?;
        compile_plugins(
            manifest,
            inputs,
            &effective_hooks,
            &effective_mcp,
            &mut files,
            &mut diagnostics,
        )?;

        Ok(CompileOutput { files, diagnostics })
    }
}

/// Normalizes a list of glob patterns into Antigravity's single comma-separated `globs` string:
/// expands simple `{a,b}` brace lists and prefixes workspace-relative paths with `**/`.
#[must_use]
pub fn normalize_antigravity_globs(globs: &[String]) -> String {
    let mut expanded = Vec::new();
    for raw in globs {
        for part in expand_braces(raw.trim()) {
            let normalized = if part.is_empty() {
                continue;
            } else if part.starts_with("**/") || part.starts_with('/') || part.starts_with("*.") {
                part
            } else {
                format!("**/{part}")
            };
            if !expanded.contains(&normalized) {
                expanded.push(normalized);
            }
        }
    }
    expanded.join(", ")
}

fn expand_braces(pattern: &str) -> Vec<String> {
    let Some(open) = pattern.find('{') else {
        return vec![pattern.to_owned()];
    };
    let rest = pattern.get(open.saturating_add(1)..).unwrap_or_default();
    let Some(rel_close) = rest.find('}') else {
        return vec![pattern.to_owned()];
    };
    let prefix = pattern.get(..open).unwrap_or_default();
    let body = rest.get(..rel_close).unwrap_or_default();
    let suffix = rest.get(rel_close.saturating_add(1)..).unwrap_or_default();

    let mut results = Vec::new();
    for option in body.split(',') {
        let combined = format!("{prefix}{}{suffix}", option.trim());
        results.extend(expand_braces(&combined));
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_antigravity_globs_with_brace_expansion_and_prefix() {
        let globs = vec![
            "crates/**/*.rs".to_owned(),
            "src/**/*.{ts,tsx}".to_owned(),
            "**/already/**".to_owned(),
        ];
        assert_eq!(
            normalize_antigravity_globs(&globs),
            "**/crates/**/*.rs, **/src/**/*.ts, **/src/**/*.tsx, **/already/**"
        );
    }
}
