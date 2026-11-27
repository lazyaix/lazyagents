//! Opt-in `.github/*` and `.gitignore` governance compiler translating `[engineering]`
//! into SHA-pinned CI workflows, managed `CODEOWNERS`, PR templates, and scaffolds.

use core::fmt::Write as _;
use std::collections::BTreeMap;

use crate::catalog::builtin_github_scaffold;
use crate::compiler::{CompileInputs, CompileOutput, GeneratedFile};
use crate::engineering::{AiDisclosureMode, MergeStrategy};
use crate::error::CoreError;
use crate::lock::ManagedRegionStyle;
use crate::manifest::{GitOutputMode, ProjectManifest};
use crate::template::{TemplateContext, TemplateEngine};

/// Opt-in compiler emitting repository `.github/*` governance files and `.gitignore` managed blocks.
#[derive(Clone, Copy, Debug, Default)]
pub struct GitHubGovernanceCompiler;

impl GitHubGovernanceCompiler {
    /// Compiles enabled `.github/*` artifacts and `.gitignore` hygiene blocks from `[engineering]`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::TemplateRender`] if an opt-in scaffold template fails to render.
    pub fn compile(
        manifest: &ProjectManifest,
        inputs: &CompileInputs<'_>,
        generated_agent_files: &BTreeMap<String, GeneratedFile>,
    ) -> Result<CompileOutput, CoreError> {
        let mut files = Vec::new();
        let ctx = TemplateContext {
            project: &inputs.facts,
            engineering: &manifest.engineering,
        };

        files.push(compile_gitignore(manifest, generated_agent_files));

        if manifest.engineering.protected_paths.codeowners_sync
            && !manifest.engineering.protected_paths.entries.is_empty()
        {
            files.push(compile_codeowners(manifest));
        }

        if manifest.engineering.github.enabled {
            if manifest.engineering.github.sync.ci_workflow {
                files.push(compile_lazyagents_ci_workflow(manifest));
            }
            if manifest.engineering.github.sync.pr_template {
                files.push(compile_pr_template(manifest));
            }
            for &scaffold in &manifest.engineering.github.scaffolds {
                let (path, template) = builtin_github_scaffold(scaffold);
                let rendered = TemplateEngine::render(path, template, &ctx)?;
                files.push(GeneratedFile::whole_file(path, rendered));
            }
        }

        Ok(CompileOutput {
            files,
            diagnostics: Vec::new(),
        })
    }
}

fn compile_gitignore(
    manifest: &ProjectManifest,
    generated_agent_files: &BTreeMap<String, GeneratedFile>,
) -> GeneratedFile {
    let mut lines = vec![".lazyagents/local.toml".to_owned()];
    if manifest.project.git_mode == GitOutputMode::Ignore {
        for path in generated_agent_files.keys() {
            if !path.starts_with(".github/") && path != ".gitignore" {
                lines.push(path.clone());
            }
        }
    }
    let body = format!("{}\n", lines.join("\n"));
    GeneratedFile::managed_region(".gitignore", ManagedRegionStyle::HashComment, body)
}

fn compile_codeowners(manifest: &ProjectManifest) -> GeneratedFile {
    let mut lines = Vec::new();
    for entry in &manifest.engineering.protected_paths.entries {
        if entry.owners.is_empty() {
            continue;
        }
        let pattern = normalize_codeowners_pattern(&entry.glob);
        lines.push(format!("{pattern} {}", entry.owners.join(" ")));
    }
    let body = if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    };
    GeneratedFile::managed_region(".github/CODEOWNERS", ManagedRegionStyle::HashComment, body)
}

fn normalize_codeowners_pattern(glob: &str) -> String {
    let trimmed = glob.trim();
    let stripped = trimmed.strip_suffix("/**").map_or(trimmed, |prefix| prefix);
    let mut out = if stripped.starts_with('/') {
        stripped.to_owned()
    } else {
        format!("/{stripped}")
    };
    if trimmed.ends_with("/**") && !out.ends_with('/') {
        out.push('/');
    }
    out
}

fn compile_lazyagents_ci_workflow(manifest: &ProjectManifest) -> GeneratedFile {
    let branch = &manifest.engineering.git.default_branch;
    let yml = format!(
        "name: LazyAgents Check\n\
\n\
on:\n\
  push:\n\
    branches:\n\
      - {branch}\n\
  pull_request:\n\
    branches:\n\
      - {branch}\n\
  merge_group:\n\
    branches:\n\
      - {branch}\n\
\n\
permissions: {{}}\n\
\n\
jobs:\n\
  lazyagents-check:\n\
    name: LazyAgents Check\n\
    runs-on: ubuntu-latest\n\
    permissions:\n\
      contents: read\n\
    steps:\n\
      - name: Checkout repository\n\
        uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1\n\
        with:\n\
          persist-credentials: false\n\
\n\
      - name: Verify AI agent configurations\n\
        run: lazyagents check --strict --format github\n"
    );
    GeneratedFile::whole_file(".github/workflows/lazyagents.yml", yml)
}

fn compile_pr_template(manifest: &ProjectManifest) -> GeneratedFile {
    let mut body = String::from(
        "## Summary\n\n<!-- Describe what this PR changes and why. -->\n\n## Related Issues / Dependencies\n\n",
    );
    if manifest.engineering.git.stacked_prs {
        if manifest.engineering.git.merge_strategy == MergeStrategy::SquashOnly {
            let branch = &manifest.engineering.git.default_branch;
            let _ = writeln!(
                body,
                "<!-- Under squash-only merge: keep downstream stacked PRs as Draft and restack via `git rebase --update-refs --onto origin/{branch} <old-parent-tip> <top-branch>` after upstream merges. -->"
            );
        }
        body.push_str("- Closes #\n- Depends on #\n\n");
    } else {
        body.push_str("- Closes #\n\n");
    }
    body.push_str("## Verification\n\n");
    if manifest.engineering.verify.commands.is_empty() {
        body.push_str("- [ ] Local lint and tests pass\n");
    } else {
        for check in &manifest.engineering.verify.commands {
            let _ = writeln!(body, "- [ ] `{}` ({})", check.cmd, check.label);
        }
    }
    if manifest.engineering.git.ai_disclosure == AiDisclosureMode::PrCheckbox {
        body.push_str(
            "\n## AI-Assisted Disclosure (Optional)\n\n- [ ] Human-reviewed and verified locally\n",
        );
    }
    GeneratedFile::whole_file(".github/pull_request_template.md", body)
}
