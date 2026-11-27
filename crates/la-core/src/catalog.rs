//! Embedded offline-first curated preset catalog for dynamic `AGENTS.md` sections,
//! Git/GitHub collaboration skills, language packs, MCP presets, and `.github/*` scaffolds.

use std::collections::BTreeMap;

use crate::engineering::GitHubScaffold;
use crate::id::ResourceId;
use crate::manifest::{McpServerSpec, McpTransport, McpVariable};

/// Metadata and rendered body of a built-in skill shipped in the offline catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BuiltinSkillTemplate {
    /// Lowercase kebab-case skill name (matches directory and `SKILL.md` frontmatter `name`).
    pub name: &'static str,
    /// Concise activation description read by agents during progressive disclosure.
    pub description: &'static str,
    /// Full `SKILL.md` body (including YAML frontmatter).
    pub skill_md: &'static str,
}

/// Summary descriptor for listing built-in section presets in CLI or TUI pickers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresetSummary {
    /// Canonical preset identifier (e.g., `"builtin:engineering/git-workflow"`).
    pub id: &'static str,
    /// Short human-readable summary.
    pub summary: &'static str,
}

/// Returns all built-in `AGENTS.md` section presets available offline.
#[must_use]
pub const fn list_section_presets() -> &'static [PresetSummary] {
    &[
        PresetSummary {
            id: "builtin:engineering/git-workflow",
            summary: "Git branching, worktree isolation, Conventional Commits, and stacked PR rules",
        },
        PresetSummary {
            id: "builtin:engineering/github-hygiene",
            summary: "GitHub Actions SHA-pinning, least-privilege permissions, and token redaction",
        },
        PresetSummary {
            id: "builtin:engineering/verify-and-supply-chain",
            summary: "Mandatory local verification commands and dependency supply-chain baseline",
        },
        PresetSummary {
            id: "builtin:lang/rust/core",
            summary: "Rust workspace toolchain, Clippy, rustdoc, and testing guidelines",
        },
        PresetSummary {
            id: "builtin:lang/typescript/core",
            summary: "TypeScript strict typing, package manager, lint, and test guidelines",
        },
        PresetSummary {
            id: "builtin:lang/python/core",
            summary: "Python uv/ruff/mypy/pytest verification and virtual environment guidelines",
        },
        PresetSummary {
            id: "builtin:lang/go/core",
            summary: "Go module formatting, golangci-lint, race-detector testing guidelines",
        },
    ]
}

/// Resolves a built-in `AGENTS.md` section template by its canonical preset ID.
#[must_use]
pub fn builtin_section_template(preset_id: &str) -> Option<&'static str> {
    match preset_id {
        "builtin:engineering/git-workflow" => Some(SECTION_GIT_WORKFLOW),
        "builtin:engineering/github-hygiene" => Some(SECTION_GITHUB_HYGIENE),
        "builtin:engineering/verify-and-supply-chain" => Some(SECTION_VERIFY_SUPPLY_CHAIN),
        "builtin:lang/rust/core" => Some(SECTION_LANG_RUST),
        "builtin:lang/typescript/core" => Some(SECTION_LANG_TYPESCRIPT),
        "builtin:lang/python/core" => Some(SECTION_LANG_PYTHON),
        "builtin:lang/go/core" => Some(SECTION_LANG_GO),
        _ => None,
    }
}

/// Resolves a built-in Git or GitHub collaboration skill by kebab-case name.
#[must_use]
pub fn builtin_skill_template(name: &str) -> Option<BuiltinSkillTemplate> {
    match name {
        "worktree-isolation" => Some(BuiltinSkillTemplate {
            name: "worktree-isolation",
            description: "Create and clean up isolated git worktrees for parallel AI agent tasks without workspace or build-cache collisions.",
            skill_md: SKILL_WORKTREE_ISOLATION,
        }),
        "pr-author" => Some(BuiltinSkillTemplate {
            name: "pr-author",
            description: "Draft and verify a Pull Request with a Conventional Commits 1.0.0 title, local verification checks, and repository PR template.",
            skill_md: SKILL_PR_AUTHOR,
        }),
        "ci-remediation" => Some(BuiltinSkillTemplate {
            name: "ci-remediation",
            description: "Inspect failing GitHub Actions PR checks via gh CLI, diagnose root causes, and apply bounded local fixes until CI passes.",
            skill_md: SKILL_CI_REMEDIATION,
        }),
        "stacked-pr" => Some(BuiltinSkillTemplate {
            name: "stacked-pr",
            description: "Manage sequential Stacked Pull Requests using Depends-on links and git rebase --update-refs.",
            skill_md: SKILL_STACKED_PR,
        }),
        "release-prep" => Some(BuiltinSkillTemplate {
            name: "release-prep",
            description: "Prepare and validate semantic version releases, changelogs, and immutable release tag checks.",
            skill_md: SKILL_RELEASE_PREP,
        }),
        _ => None,
    }
}

/// Returns the built-in Streamable HTTP GitHub MCP server preset (`variables`-only, zero plaintext secrets).
#[must_use]
pub fn builtin_github_mcp_preset() -> McpServerSpec {
    let mut headers = BTreeMap::new();
    headers.insert(
        "Authorization".to_owned(),
        "Bearer ${GITHUB_MCP_TOKEN}".to_owned(),
    );

    let name = ResourceId::from_static_valid("github");
    McpServerSpec {
        name,
        enabled: true,
        transport: McpTransport::StreamableHttp {
            url: "https://api.githubcopilot.com/mcp/".to_owned(),
            headers,
        },
        variables: vec![McpVariable {
            name: "GITHUB_MCP_TOKEN".to_owned(),
            title: "GitHub MCP Token".to_owned(),
            description: "Fine-grained PAT or GitHub App token for remote GitHub MCP access."
                .to_owned(),
        }],
    }
}

/// Returns the `(workspace_path, template_body)` pair for an opt-in `.github/*` governance scaffold.
#[must_use]
pub const fn builtin_github_scaffold(scaffold: GitHubScaffold) -> (&'static str, &'static str) {
    match scaffold {
        GitHubScaffold::PrTitle => (".github/workflows/pr-title.yml", SCAFFOLD_PR_TITLE_YML),
        GitHubScaffold::SecurityZizmor => (
            ".github/workflows/security-zizmor.yml",
            SCAFFOLD_SECURITY_ZIZMOR_YML,
        ),
        GitHubScaffold::Dependabot => (".github/dependabot.yml", SCAFFOLD_DEPENDABOT_YML),
        GitHubScaffold::IssueTemplates => (
            ".github/ISSUE_TEMPLATE/bug_report.yml",
            SCAFFOLD_BUG_REPORT_YML,
        ),
        GitHubScaffold::Contributing => ("CONTRIBUTING.md", SCAFFOLD_CONTRIBUTING_MD),
        GitHubScaffold::SecurityPolicy => ("SECURITY.md", SCAFFOLD_SECURITY_MD),
    }
}

const SECTION_GIT_WORKFLOW: &str = r"## Git Workflow & Branch Isolation

- **Never push directly to `{{ engineering.git.default_branch }}`**; use short-lived branches (`{{ engineering.git.branch_prefixes | join(', ') }}`).
{% if engineering.git.worktree.enabled %}- **Use `git worktree`** for concurrent agent tasks (`{{ engineering.git.worktree.pattern }}`) so working trees and build caches never collide.
{% endif %}- **Merge Strategy**: `{{ engineering.git.merge_strategy }}` with Conventional Commits (`{{ engineering.git.conventional_commits }}`).
{% if engineering.git.stacked_prs %}- Keep PRs focused on one vertical slice.{% if engineering.git.merge_strategy == 'squash-only' %} Under `squash-only` merge, prefer **Sequential PRs** (merge PR 1 → sync `{{ engineering.git.default_branch }}` → branch PR 2); when stacking for async review, open downstream PRs as **Draft** with `Depends on #<pr>`, never use plain `git rebase` or GitHub *Update branch*, and restack after upstream squash-merges via `git rebase --update-refs --onto origin/{{ engineering.git.default_branch }} <old-parent-tip> <top-branch>` + `git push --force-with-lease`.{% else %} For stacked work, link `Depends on #<pr>` in the PR description.{% endif %}
{% endif %}{% if engineering.protected_paths.entries | length > 0 %}- **Protected Paths** (require explicit human review): {% for entry in engineering.protected_paths.entries %}`{{ entry.glob }}`{% if not loop.last %}, {% endif %}{% endfor %}.
{% endif %}";

const SECTION_GITHUB_HYGIENE: &str = r"## GitHub Actions & Secret Hygiene

- Pin every `uses:` action in `.github/workflows/*.yml` to a **full 40-character commit SHA** with a `# vX.Y.Z` version comment.
- Declare top-level `permissions: {}` and minimal per-job permissions, and set `persist-credentials: false` on `actions/checkout`.
- Never use `pull_request_target` with untrusted PR checkouts or interpolate `{% raw %}${{ github.event.* }}{% endraw %}` directly into `run:` scripts.
{% if engineering.supply_chain.stateless_github_tokens %}- Treat GitHub tokens (including ~520-char stateless `ghs_*` App installation tokens) as variable-length opaque strings and redact `ghs_[A-Za-z0-9._-]{36,}` in logs and traces.
{% endif %}- Never commit plaintext secrets, API keys, or `.env` files.";

const SECTION_VERIFY_SUPPLY_CHAIN: &str = r"## Mandatory Local Verification & Supply-Chain Baseline

{% if engineering.verify.commands | length > 0 %}Run all required checks before pushing or updating a PR:

```bash
{% for check in engineering.verify.commands %}{% if check.required %}{{ check.cmd }}
{% endif %}{% endfor %}```
{% else %}Run the project's formatter, linter, and test suite locally before pushing.
{% endif %}
{% if engineering.supply_chain.forbid_copyleft %}- **License Policy**: Permissive licenses only (`{{ engineering.supply_chain.allowed_licenses | join(', ') }}`); reject Copyleft (MPL/GPL/AGPL) dependencies.
{% endif %}- **Publish Cooldown**: Enforce a {{ engineering.supply_chain.publish_cooldown_days }}-day minimum publish age for new dependency updates.";

const SECTION_LANG_RUST: &str = r#"## Rust Engineering Baseline

{% if project.toolchain_version %}- **Toolchain**: Rust `{{ project.toolchain_version }}`{% if project.language_edition %} (`edition = "{{ project.language_edition }}"`){% endif %}.
{% endif %}- Keep core domain logic pure Sans-I/O, enforce strict Clippy lints without `unwrap()`/`expect()`/`panic!()` in production code, and document all public APIs.
{% if engineering.architecture.layers | length > 0 %}- **Layer Boundaries**:
{% for layer in engineering.architecture.layers %}  - `{{ layer.name }}` (`{{ layer.glob }}`): {{ layer.rule }}
{% endfor %}{% endif %}"#;

const SECTION_LANG_TYPESCRIPT: &str = r"## TypeScript Engineering Baseline

- Enable strict TypeScript type-checking (`strict: true`), avoid `any`, and keep runtime boundaries validated with typed schemas.
- Run formatting, typecheck, lint, and unit tests locally before pushing.";

const SECTION_LANG_PYTHON: &str = r"## Python Engineering Baseline

- Use isolated virtual environments (`uv` / `venv`), type annotations validated with `mypy`/`pyright`, `ruff` formatting/linting, and `pytest` before pushing.";

const SECTION_LANG_GO: &str = r"## Go Engineering Baseline

- Run `gofmt`, `go vet ./...`, `golangci-lint run`, and `go test -race ./...` before opening or updating a PR.";

const SKILL_WORKTREE_ISOLATION: &str = r"---
name: worktree-isolation
description: Create and clean up isolated git worktrees for parallel AI agent tasks without workspace or build-cache collisions.
---

# Git Worktree Isolation

1. Fetch the latest default branch (`git fetch origin --prune`).
2. Create an isolated sibling worktree:
   `git worktree add ../<repo>-wt-<task> -b <prefix>/<task> main`
3. Execute edits, builds, and tests strictly inside the isolated worktree.
4. After the PR merges, remove the worktree cleanly:
   `git worktree remove ../<repo>-wt-<task> && git worktree prune`
";

const SKILL_PR_AUTHOR: &str = r"---
name: pr-author
description: Draft and verify a Pull Request with a Conventional Commits 1.0.0 title, local verification checks, and repository PR template.
---

# Pull Request Authoring Workflow

1. Confirm you are on a short-lived topic branch (never the default branch) and inspect `git diff --stat`.
2. Run every required local verification command declared in `AGENTS.md`.
3. Compose a Conventional Commits 1.0.0 PR title (`<type>(<scope>): <concise summary>`).
4. Fill out the repository PR template completely, including verification checkboxes and `Depends on #<pr>` for stacked PRs.
5. Open the PR with `gh pr create` and monitor required checks.
";

const SKILL_CI_REMEDIATION: &str = r"---
name: ci-remediation
description: Inspect failing GitHub Actions PR checks via gh CLI, diagnose root causes, and apply bounded local fixes until CI passes.
---

# CI Self-Remediation Workflow

1. Inspect PR check status via `gh pr checks <pr>`.
2. Fetch failing job logs via `gh run view <run-id> --log-failed`.
3. Reproduce and fix the root cause locally; re-run local verification commands before pushing.
4. Limit autonomous remediation to at most 3 bounded iterations; escalate clearly if blocked by external permissions or secrets.
";

const SKILL_STACKED_PR: &str = r"---
name: stacked-pr
description: Manage Sequential and Stacked Pull Requests safely under Squash-Only merge using Draft PRs, Depends-on links, and git rebase --update-refs --onto.
---

# Stacked PR Workflow (Squash-Only Safe)

1. **Prefer Sequential PRs when merging rapidly**: merge the base PR into `main`, run `git fetch origin --prune`, fast-forward `main`, then branch the next PR from updated `main`.
2. **When async review requires a stack**:
   - Keep only the bottom PR (`base: main`) Ready; open downstream PRs as **Draft** (`gh pr create --draft`) with `Depends on #<upstream-pr>`.
   - Propagate mid-review edits up the stack locally via `git rebase --update-refs` and `git push --force-with-lease`.
3. **After an upstream PR squash-merges (critical)**:
   - **Never** click GitHub's *Update branch* button or run plain `git rebase origin/main` (replays already-squashed parent commits and causes false conflicts).
   - Skip the old pre-squash parent commit tip and restack the remaining chain cleanly:
     `git fetch origin --prune && git rebase --update-refs --onto origin/main <old-parent-tip> <top-branch>`
   - Force-push updated child branches safely with `git push --force-with-lease`.
   - Confirm the newly promoted PR targets `main` with `MERGEABLE` status, then run `gh pr ready <pr>`.
";

const SKILL_RELEASE_PREP: &str = r"---
name: release-prep
description: Prepare and validate semantic version releases, changelogs, and immutable release tag checks.
---

# Release Preparation Workflow

1. Verify `main` is green across all required CI, security, and cross-platform checks.
2. Review Conventional Commits since the last release tag to confirm version bump (`major`/`minor`/`patch`) and changelog entries.
3. Validate that release tags and immutable release protections remain active before merging the release PR.
";

const SCAFFOLD_PR_TITLE_YML: &str = r"name: PR Title

on:
  pull_request:
    types: [opened, edited, synchronize, reopened]
  merge_group:
    branches: [{{ engineering.git.default_branch }}]

permissions: {}

jobs:
  pr-title:
    name: PR Title
    runs-on: ubuntu-latest
    permissions:
      pull-requests: read
    steps:
      - name: Validate Conventional Commit PR title
        if: github.event_name == 'pull_request'
        uses: amannn/action-semantic-pull-request@66e72920af32222769c386d81f89656ebcb6d6d5 # v6.1.4
        env:
          GITHUB_TOKEN: {% raw %}${{ secrets.GITHUB_TOKEN }}{% endraw %}
        with:
          requireScope: false
          wip: false
";

const SCAFFOLD_SECURITY_ZIZMOR_YML: &str = r"name: Actions Security

on:
  push:
    branches: [{{ engineering.git.default_branch }}]
  pull_request:
    branches: [{{ engineering.git.default_branch }}]
  merge_group:
    branches: [{{ engineering.git.default_branch }}]

permissions: {}

jobs:
  zizmor:
    name: Zizmor
    runs-on: ubuntu-latest
    permissions:
      contents: read
    steps:
      - name: Checkout repository
        uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
        with:
          persist-credentials: false

      - name: Audit GitHub Actions workflows
        uses: zizmorcore/zizmor-action@1b4550e929643b8b26a62303c6bd64814271a4f1 # v0.9.2
        with:
          advanced-security: false
          annotations: true
          online-audits: true
";

const SCAFFOLD_DEPENDABOT_YML: &str = r#"version: 2
updates:
  - package-ecosystem: "github-actions"
    directory: "/"
    schedule:
      interval: "weekly"
    cooldown:
      default-days: {{ engineering.supply_chain.publish_cooldown_days }}
    commit-message:
      prefix: "chore"
      include: "scope"
"#;

const SCAFFOLD_BUG_REPORT_YML: &str = r"name: Bug Report
description: Report a reproducible bug
labels: [bug]
body:
  - type: textarea
    id: summary
    attributes:
      label: What happened?
      description: Concise summary of the bug and expected behavior.
    validations:
      required: true
  - type: textarea
    id: repro
    attributes:
      label: Reproduction Steps
    validations:
      required: true
";

const SCAFFOLD_CONTRIBUTING_MD: &str = r"# Contributing to {{ project.name }}

1. Branch off `{{ engineering.git.default_branch }}` using one of: `{{ engineering.git.branch_prefixes | join(', ') }}`.
2. Run all required verification checks before opening a Pull Request:
{% for check in engineering.verify.commands %}{% if check.required %}   - `{{ check.cmd }}`
{% endif %}{% endfor %}3. Follow Conventional Commits 1.0.0 (`{{ engineering.git.conventional_commits }}`).
";

const SCAFFOLD_SECURITY_MD: &str = r"# Security Policy

Please report security vulnerabilities privately via GitHub Security Advisories rather than opening a public issue.

## Supply-Chain Baseline
- All GitHub Actions workflows pin actions to full 40-character commit SHAs with minimal `permissions`.
- Dependency updates enforce a {{ engineering.supply_chain.publish_cooldown_days }}-day publish cooldown.
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_listed_presets_resolve_to_non_empty_templates() {
        for preset in list_section_presets() {
            let tpl = builtin_section_template(preset.id).expect("preset must resolve");
            assert_ne!(tpl.trim(), "");
        }
    }

    #[test]
    fn builtin_skills_and_github_mcp_preset_are_well_formed() {
        for name in [
            "worktree-isolation",
            "pr-author",
            "ci-remediation",
            "stacked-pr",
            "release-prep",
        ] {
            let skill = builtin_skill_template(name).expect("skill exists");
            assert_eq!(skill.name, name);
            assert!(skill.skill_md.contains(&format!("name: {name}")));
        }

        let stacked = builtin_skill_template("stacked-pr").expect("stacked-pr exists");
        assert!(stacked.skill_md.contains("--update-refs --onto"));

        let git_tpl = builtin_section_template("builtin:engineering/git-workflow")
            .expect("git preset exists");
        assert!(git_tpl.contains("squash-only"));
        assert!(git_tpl.contains("--update-refs --onto"));

        let mcp = builtin_github_mcp_preset();
        assert_eq!(mcp.name.as_str(), "github");
        assert_eq!(mcp.variables.len(), 1);
    }
}
