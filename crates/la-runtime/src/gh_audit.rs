//! Read-only live GitHub repository & Rulesets compliance auditor backed by the user's
//! locally authenticated `gh` CLI (zero credential storage inside `lazyagents`).

use core::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use la_core::engineering::{ExpectedRulesets, MergeStrategy};
use la_core::{ProjectManifest, redact_ghs_tokens};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::error::RuntimeError;
use crate::prober::probe_repository;

/// Evaluation status for one GitHub governance audit check.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditCheckStatus {
    /// Remote repository setting matches the declared policy.
    Pass,
    /// Remote setting diverges from `.lazyagents/lazyagents.toml`.
    Mismatch,
    /// Check could not be evaluated (e.g., optional API field unavailable).
    Skipped,
}

/// One audited GitHub governance expectation and its copyable `gh api` remediation command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitHubAuditCheck {
    /// Stable check identifier (e.g., `"merge-strategy"`, `"ruleset-default-branch"`).
    pub id: String,
    /// Human-readable check label.
    pub label: String,
    /// Audit outcome.
    pub status: AuditCheckStatus,
    /// Concise explanation of observed vs expected state.
    pub detail: String,
    /// Optional copyable `gh api` command to remediate a mismatch (never executed automatically).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation_cmd: Option<String>,
}

/// Complete read-only GitHub audit report for CLI text/JSON output and the TUI GH Audit panel.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitHubAuditReport {
    /// Audited repository slug (`owner/name`).
    pub repo: String,
    /// Ordered audit checks.
    pub checks: Vec<GitHubAuditCheck>,
}

#[derive(Deserialize)]
struct GhRepoResponse {
    #[serde(default)]
    allow_squash_merge: Option<bool>,
    #[serde(default)]
    allow_merge_commit: Option<bool>,
    #[serde(default)]
    allow_rebase_merge: Option<bool>,
    #[serde(default)]
    delete_branch_on_merge: Option<bool>,
}

#[derive(Deserialize)]
struct GhRulesetSummary {
    #[serde(default)]
    name: String,
    #[serde(default)]
    target: String,
    #[serde(default)]
    enforcement: String,
    #[serde(default)]
    conditions: Option<GhRulesetConditions>,
    #[serde(default)]
    rules: Vec<GhRuleEntry>,
}

#[derive(Deserialize)]
struct GhRulesetConditions {
    #[serde(default)]
    ref_name: Option<GhRefNameCondition>,
}

#[derive(Deserialize)]
struct GhRefNameCondition {
    #[serde(default)]
    include: Vec<String>,
}

#[derive(Deserialize)]
struct GhRuleEntry {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    parameters: serde_json::Value,
}

impl GitHubAuditReport {
    /// Returns `true` if any audited check reported [`AuditCheckStatus::Mismatch`].
    #[must_use]
    pub fn has_mismatches(&self) -> bool {
        self.checks
            .iter()
            .any(|c| c.status == AuditCheckStatus::Mismatch)
    }

    /// Formats the audit report as human-readable terminal text.
    #[must_use]
    pub fn format_text(&self) -> String {
        let mut out = format!("GitHub Audit ({})\n", self.repo);
        for check in &self.checks {
            let tag = match check.status {
                AuditCheckStatus::Pass => "PASS",
                AuditCheckStatus::Mismatch => "DRIFT",
                AuditCheckStatus::Skipped => "SKIP",
            };
            let _ = writeln!(out, "  [{tag}] {}: {}", check.label, check.detail);
            if let Some(cmd) = &check.remediation_cmd {
                let _ = writeln!(out, "         fix: {cmd}");
            }
        }
        out
    }

    /// Serializes the audit report to pretty JSON.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError::GitHubAudit`] on JSON serialization failure.
    pub fn to_json_pretty(&self) -> Result<String, RuntimeError> {
        let mut json =
            serde_json::to_string_pretty(self).map_err(|err| RuntimeError::GitHubAudit {
                message: err.to_string(),
            })?;
        json.push('\n');
        Ok(json)
    }
}

/// Runs a read-only GitHub compliance audit against live repository settings and Rulesets
/// using the locally authenticated `gh` CLI (zero credential storage inside `lazyagents`).
///
/// # Errors
///
/// Returns [`RuntimeError::GitHubAudit`] if no repository slug can be inferred, `gh` is not
/// installed/authenticated, or GitHub API responses cannot be parsed.
pub fn audit_github_repo(
    root: &Path,
    manifest: &ProjectManifest,
    repo_override: Option<&str>,
) -> Result<GitHubAuditReport, RuntimeError> {
    let repo = resolve_repo_slug(root, manifest, repo_override)?;
    debug!(repo = %repo, "running read-only GitHub audit via local gh CLI");

    let repo_json = run_gh_api(root, &format!("repos/{repo}"))?;
    let rulesets_json = run_gh_api(
        root,
        &format!("repos/{repo}/rulesets?includes_parents=true"),
    )?;
    let immutable_enabled = run_gh_api(root, &format!("repos/{repo}/immutable-releases"))
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|v| v.get("enabled").and_then(serde_json::Value::as_bool));

    compare_github_state(
        &repo,
        &manifest.engineering.github.expected_rulesets,
        manifest.engineering.git.merge_strategy,
        &repo_json,
        &rulesets_json,
        immutable_enabled,
    )
}

/// Pure offline comparison of GitHub API JSON payloads against declared `[engineering]`
/// expectations, producing actionable checks and copyable `gh api` remediation commands.
///
/// # Errors
///
/// Returns [`RuntimeError::GitHubAudit`] if either JSON payload is malformed.
pub fn compare_github_state(
    repo: &str,
    expected: &ExpectedRulesets,
    merge_strategy: MergeStrategy,
    repo_json: &str,
    rulesets_json: &str,
    immutable_enabled: Option<bool>,
) -> Result<GitHubAuditReport, RuntimeError> {
    let repo_info: GhRepoResponse =
        serde_json::from_str(repo_json).map_err(|err| RuntimeError::GitHubAudit {
            message: format!("failed to parse GitHub repo JSON: {err}"),
        })?;
    let rulesets: Vec<GhRulesetSummary> =
        serde_json::from_str(rulesets_json).map_err(|err| RuntimeError::GitHubAudit {
            message: format!("failed to parse GitHub rulesets JSON: {err}"),
        })?;

    let mut checks = Vec::new();
    check_merge_strategy(repo, merge_strategy, &repo_info, &mut checks);
    check_default_branch_ruleset(repo, expected, &rulesets, &mut checks);
    check_release_tags_and_immutable(repo, expected, &rulesets, immutable_enabled, &mut checks);

    Ok(GitHubAuditReport {
        repo: repo.to_owned(),
        checks,
    })
}

fn check_merge_strategy(
    repo: &str,
    strategy: MergeStrategy,
    repo_info: &GhRepoResponse,
    checks: &mut Vec<GitHubAuditCheck>,
) {
    if strategy != MergeStrategy::SquashOnly {
        return;
    }
    let squash_only = repo_info.allow_squash_merge == Some(true)
        && repo_info.allow_merge_commit == Some(false)
        && repo_info.allow_rebase_merge == Some(false);
    let delete_head = repo_info.delete_branch_on_merge == Some(true);

    if squash_only && delete_head {
        checks.push(GitHubAuditCheck {
            id: "merge-strategy".to_owned(),
            label: "Merge Strategy (Squash-only + Auto-delete)".to_owned(),
            status: AuditCheckStatus::Pass,
            detail: "Repository enforces squash-only merges and auto-deletes head branches."
                .to_owned(),
            remediation_cmd: None,
        });
    } else {
        checks.push(GitHubAuditCheck {
            id: "merge-strategy".to_owned(),
            label: "Merge Strategy (Squash-only + Auto-delete)".to_owned(),
            status: AuditCheckStatus::Mismatch,
            detail: format!(
                "Expected squash-only + delete_branch_on_merge=true (got squash={:?}, merge={:?}, rebase={:?}, delete_branch={:?})",
                repo_info.allow_squash_merge,
                repo_info.allow_merge_commit,
                repo_info.allow_rebase_merge,
                repo_info.delete_branch_on_merge
            ),
            remediation_cmd: Some(format!(
                "gh api -X PATCH repos/{repo} -F allow_squash_merge=true -F allow_merge_commit=false -F allow_rebase_merge=false -F delete_branch_on_merge=true"
            )),
        });
    }
}

fn check_default_branch_ruleset(
    repo: &str,
    expected: &ExpectedRulesets,
    rulesets: &[GhRulesetSummary],
    checks: &mut Vec<GitHubAuditCheck>,
) {
    if !expected.protect_default_branch {
        return;
    }
    let active_branch_rulesets: Vec<&GhRulesetSummary> = rulesets
        .iter()
        .filter(|r| {
            r.enforcement == "active"
                && r.target == "branch"
                && r.conditions
                    .as_ref()
                    .and_then(|c| c.ref_name.as_ref())
                    .is_none_or(|rn| {
                        rn.include.is_empty()
                            || rn.include.iter().any(|inc| inc == "~DEFAULT_BRANCH")
                    })
        })
        .collect();

    if active_branch_rulesets.is_empty() {
        checks.push(GitHubAuditCheck {
            id: "ruleset-default-branch".to_owned(),
            label: "Default Branch Ruleset".to_owned(),
            status: AuditCheckStatus::Mismatch,
            detail: "No active GitHub Repository Ruleset protects `~DEFAULT_BRANCH`.".to_owned(),
            remediation_cmd: Some(format!(
                "gh api repos/{repo}/rulesets  # Create an active `Protect main` Ruleset in GitHub Settings -> Rules"
            )),
        });
        return;
    }

    checks.push(GitHubAuditCheck {
        id: "ruleset-default-branch".to_owned(),
        label: "Default Branch Ruleset".to_owned(),
        status: AuditCheckStatus::Pass,
        detail: format!(
            "Protected by active Ruleset(s): {}",
            active_branch_rulesets
                .iter()
                .map(|r| r.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        remediation_cmd: None,
    });

    if !expected.required_checks.is_empty() {
        let mut active_checks = Vec::new();
        for rs in &active_branch_rulesets {
            for rule in &rs.rules {
                if rule.kind == "required_status_checks"
                    && let Some(arr) = rule
                        .parameters
                        .get("required_status_checks")
                        .and_then(serde_json::Value::as_array)
                {
                    for item in arr {
                        if let Some(ctx) = item.get("context").and_then(serde_json::Value::as_str) {
                            active_checks.push(ctx.to_owned());
                        }
                    }
                }
            }
        }
        let missing: Vec<&str> = expected
            .required_checks
            .iter()
            .filter(|req| !active_checks.iter().any(|a| a == *req))
            .map(String::as_str)
            .collect();
        if missing.is_empty() {
            checks.push(GitHubAuditCheck {
                id: "required-checks".to_owned(),
                label: "Required Status Checks".to_owned(),
                status: AuditCheckStatus::Pass,
                detail: format!(
                    "All {} expected status checks are enforced.",
                    expected.required_checks.len()
                ),
                remediation_cmd: None,
            });
        } else {
            checks.push(GitHubAuditCheck {
                id: "required-checks".to_owned(),
                label: "Required Status Checks".to_owned(),
                status: AuditCheckStatus::Mismatch,
                detail: format!("Missing required status check(s): {}", missing.join(", ")),
                remediation_cmd: Some(format!(
                    "gh api repos/{repo}/rulesets  # Add missing checks to the default-branch Ruleset"
                )),
            });
        }
    }
}

fn check_release_tags_and_immutable(
    repo: &str,
    expected: &ExpectedRulesets,
    rulesets: &[GhRulesetSummary],
    immutable_enabled: Option<bool>,
    checks: &mut Vec<GitHubAuditCheck>,
) {
    if let Some(pattern) = &expected.release_tags.pattern {
        let expected_ref = format!("refs/tags/{pattern}");
        let has_tag_ruleset = rulesets.iter().any(|r| {
            r.enforcement == "active"
                && r.target == "tag"
                && r.conditions
                    .as_ref()
                    .and_then(|c| c.ref_name.as_ref())
                    .is_some_and(|rn| {
                        rn.include
                            .iter()
                            .any(|inc| inc == &expected_ref || inc == pattern)
                    })
        });
        checks.push(if has_tag_ruleset {
            GitHubAuditCheck {
                id: "ruleset-release-tags".to_owned(),
                label: "Release Tag Ruleset".to_owned(),
                status: AuditCheckStatus::Pass,
                detail: format!("Active tag Ruleset protects `{expected_ref}`."),
                remediation_cmd: None,
            }
        } else {
            GitHubAuditCheck {
                id: "ruleset-release-tags".to_owned(),
                label: "Release Tag Ruleset".to_owned(),
                status: AuditCheckStatus::Mismatch,
                detail: format!("No active tag Ruleset matches `{expected_ref}`."),
                remediation_cmd: Some(format!(
                    "gh api repos/{repo}/rulesets  # Add an active tag Ruleset protecting `{expected_ref}`"
                )),
            }
        });
    }

    if expected.release_tags.immutable_releases {
        let check = match immutable_enabled {
            Some(true) => GitHubAuditCheck {
                id: "immutable-releases".to_owned(),
                label: "Immutable Releases".to_owned(),
                status: AuditCheckStatus::Pass,
                detail: "GitHub Immutable Releases is enabled.".to_owned(),
                remediation_cmd: None,
            },
            Some(false) => GitHubAuditCheck {
                id: "immutable-releases".to_owned(),
                label: "Immutable Releases".to_owned(),
                status: AuditCheckStatus::Mismatch,
                detail: "GitHub Immutable Releases is currently disabled.".to_owned(),
                remediation_cmd: Some(format!("gh api -X PUT repos/{repo}/immutable-releases")),
            },
            None => GitHubAuditCheck {
                id: "immutable-releases".to_owned(),
                label: "Immutable Releases".to_owned(),
                status: AuditCheckStatus::Skipped,
                detail:
                    "Immutable Releases status could not be queried (requires admin/read support)."
                        .to_owned(),
                remediation_cmd: Some(format!("gh api -X PUT repos/{repo}/immutable-releases")),
            },
        };
        checks.push(check);
    }
}

fn resolve_repo_slug(
    root: &Path,
    manifest: &ProjectManifest,
    repo_override: Option<&str>,
) -> Result<String, RuntimeError> {
    if let Some(r) = repo_override
        && !r.trim().is_empty()
    {
        return Ok(r.trim().to_owned());
    }
    if let Some(r) = &manifest.engineering.github.repo
        && !r.trim().is_empty()
    {
        return Ok(r.trim().to_owned());
    }
    if let Ok(facts) = probe_repository(root)
        && let Some(r) = facts.git_remote_repo
    {
        return Ok(r);
    }
    Err(RuntimeError::GitHubAudit {
        message: "unable to determine GitHub `owner/repo`; pass `--repo <owner/name>` or set `[engineering.github].repo`".to_owned(),
    })
}

fn run_gh_api(root: &Path, endpoint: &str) -> Result<String, RuntimeError> {
    let output = Command::new("gh")
        .args(["api", endpoint])
        .current_dir(root)
        .output()
        .map_err(|err| RuntimeError::GitHubAudit {
            message: format!(
                "failed to execute `gh api {endpoint}` (is GitHub CLI installed?): {err}"
            ),
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let safe_err = redact_ghs_tokens(stderr.trim());
        return Err(RuntimeError::GitHubAudit {
            message: format!("`gh api {endpoint}` failed: {safe_err}"),
        });
    }
    String::from_utf8(output.stdout).map_err(|err| RuntimeError::GitHubAudit {
        message: format!("non-UTF-8 output from `gh api {endpoint}`: {err}"),
    })
}
