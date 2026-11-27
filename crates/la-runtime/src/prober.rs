//! Offline repository introspection (`probe_repository`) and live Git working-tree context
//! probing (`probe_git_hook_context`) for dynamic `AGENTS.md` rendering and `hook eval`.

use std::fs;
use std::path::Path;
use std::process::Command;

use la_core::{CiWorkflowFact, CommandHints, DetectedLanguage, HookEvalContext, ProjectFacts};
use tracing::debug;

use crate::error::RuntimeError;

const SENSITIVE_FILENAMES: [&str; 6] = [
    ".env",
    ".env.local",
    ".env.production",
    "id_rsa",
    "id_ed25519",
    "credentials.json",
];

/// Performs fast, read-only offline introspection of a repository at `root` to build
/// deterministic [`ProjectFacts`] for `{{ project.* }}` templates and `lazyagents init`.
///
/// # Errors
///
/// Returns [`RuntimeError::Io`] if unreadable files are encountered where metadata is expected.
pub fn probe_repository(root: &Path) -> Result<ProjectFacts, RuntimeError> {
    let mut facts = ProjectFacts {
        name: infer_repo_name(root),
        ..ProjectFacts::default()
    };

    probe_rust(root, &mut facts);
    probe_typescript(root, &mut facts);
    probe_python(root, &mut facts);
    probe_go(root, &mut facts);
    probe_git_metadata(root, &mut facts);
    probe_github_workflows(root, &mut facts)?;

    facts.refresh_flags();
    debug!(
        repo = %facts.name,
        languages = facts.languages.len(),
        workflows = facts.ci_workflows.len(),
        "probed repository facts"
    );
    Ok(facts)
}

/// Inspects live local Git working-tree state (`current_branch`, untracked sensitive files,
/// modified tracked files) to enrich [`la_core::evaluate_hook_json`].
///
/// Falls back gracefully with empty fields if `git` is unavailable or `root` is not a Git repo.
#[must_use]
pub fn probe_git_hook_context(root: &Path) -> HookEvalContext {
    let workspace_root = Some(root.to_string_lossy().into_owned());
    let current_branch = run_git_capture(root, &["rev-parse", "--abbrev-ref", "HEAD"])
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty() && s != "HEAD");

    let mut untracked_sensitive_files = Vec::new();
    let mut has_modified_tracked_files = false;

    if let Some(status) = run_git_capture(root, &["status", "--porcelain"]) {
        for line in status.lines() {
            if line.len() < 4 {
                continue;
            }
            let code = line.get(..2).unwrap_or_default();
            let path = line.get(3..).unwrap_or_default().trim();
            if code != "??" {
                has_modified_tracked_files = true;
            }
            if is_sensitive_working_tree_file(path) {
                untracked_sensitive_files.push(path.to_owned());
            }
        }
    }

    HookEvalContext {
        workspace_root,
        current_branch,
        untracked_sensitive_files,
        has_modified_tracked_files,
    }
}

fn infer_repo_name(root: &Path) -> String {
    root.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .map_or_else(|| "unnamed-project".to_owned(), str::to_owned)
}

fn probe_rust(root: &Path, facts: &mut ProjectFacts) {
    let cargo_path = root.join("Cargo.toml");
    let Ok(raw) = fs::read_to_string(&cargo_path) else {
        return;
    };
    facts.languages.push(DetectedLanguage::Rust);

    if let Ok(doc) = raw.parse::<toml::Table>() {
        if let Some(ws) = doc.get("workspace").and_then(toml::Value::as_table) {
            if let Some(members) = ws.get("members").and_then(toml::Value::as_array) {
                for m in members.iter().filter_map(toml::Value::as_str) {
                    facts.workspace_members.push(m.to_owned());
                }
            }
            if let Some(pkg) = ws.get("package").and_then(toml::Value::as_table) {
                if let Some(ed) = pkg.get("edition").and_then(toml::Value::as_str) {
                    facts.language_edition = Some(ed.to_owned());
                }
                if let Some(rv) = pkg.get("rust-version").and_then(toml::Value::as_str) {
                    facts.toolchain_version = Some(rv.to_owned());
                }
            }
        }
        if let Some(pkg) = doc.get("package").and_then(toml::Value::as_table) {
            if let Some(name) = pkg.get("name").and_then(toml::Value::as_str) {
                name.clone_into(&mut facts.name);
            }
            if facts.language_edition.is_none()
                && let Some(ed) = pkg.get("edition").and_then(toml::Value::as_str)
            {
                facts.language_edition = Some(ed.to_owned());
            }
        }
    }

    if let Ok(tc_raw) = fs::read_to_string(root.join("rust-toolchain.toml"))
        && let Ok(doc) = tc_raw.parse::<toml::Table>()
        && let Some(channel) = doc
            .get("toolchain")
            .and_then(toml::Value::as_table)
            .and_then(|t| t.get("channel"))
            .and_then(toml::Value::as_str)
    {
        facts.toolchain_version = Some(channel.to_owned());
    }

    facts.commands = CommandHints {
        fmt: Some("cargo fmt --all -- --check".to_owned()),
        lint: Some(
            "CARGO_BUILD_WARNINGS=deny cargo clippy --workspace --all-targets --all-features -- -D warnings"
                .to_owned(),
        ),
        test: Some("cargo test --workspace --all-features".to_owned()),
        doc: Some("RUSTDOCFLAGS=\"-D warnings\" cargo doc --workspace --no-deps --all-features".to_owned()),
        build: Some("cargo build --workspace --all-features".to_owned()),
    };
}

fn probe_typescript(root: &Path, facts: &mut ProjectFacts) {
    let pkg_path = root.join("package.json");
    if !pkg_path.is_file() && !root.join("tsconfig.json").is_file() {
        return;
    }
    facts.languages.push(DetectedLanguage::TypeScript);
    if facts.commands.test.is_none() {
        let pm = if root.join("pnpm-lock.yaml").is_file() {
            "pnpm"
        } else if root.join("bun.lock").is_file() || root.join("bun.lockb").is_file() {
            "bun"
        } else {
            "npm"
        };
        facts.commands.lint = Some(format!("{pm} run lint"));
        facts.commands.test = Some(format!("{pm} test"));
        facts.commands.build = Some(format!("{pm} run build"));
    }
}

fn probe_python(root: &Path, facts: &mut ProjectFacts) {
    if !root.join("pyproject.toml").is_file() {
        return;
    }
    facts.languages.push(DetectedLanguage::Python);
    if facts.commands.test.is_none() {
        facts.commands.fmt = Some("ruff format --check .".to_owned());
        facts.commands.lint = Some("ruff check .".to_owned());
        facts.commands.test = Some("pytest".to_owned());
    }
}

fn probe_go(root: &Path, facts: &mut ProjectFacts) {
    if !root.join("go.mod").is_file() {
        return;
    }
    facts.languages.push(DetectedLanguage::Go);
    if facts.commands.test.is_none() {
        facts.commands.fmt = Some("gofmt -l .".to_owned());
        facts.commands.lint = Some("golangci-lint run".to_owned());
        facts.commands.test = Some("go test -race ./...".to_owned());
        facts.commands.build = Some("go build ./...".to_owned());
    }
}

fn probe_git_metadata(root: &Path, facts: &mut ProjectFacts) {
    let git_dir = resolve_git_dir(root);
    let Some(git_dir) = git_dir else {
        return;
    };

    if let Ok(head) = fs::read_to_string(git_dir.join("HEAD"))
        && let Some(branch) = head.trim().strip_prefix("ref: refs/heads/")
    {
        facts.git_default_branch = Some(branch.to_owned());
    }

    if let Ok(cfg) = fs::read_to_string(git_dir.join("config"))
        && let Some(slug) = extract_github_slug_from_git_config(&cfg)
    {
        facts.git_remote_repo = Some(slug);
    }
}

fn resolve_git_dir(root: &Path) -> Option<std::path::PathBuf> {
    let dot_git = root.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }
    if dot_git.is_file()
        && let Ok(content) = fs::read_to_string(&dot_git)
        && let Some(raw_dir) = content.trim().strip_prefix("gitdir:")
    {
        let p = Path::new(raw_dir.trim());
        return Some(if p.is_absolute() {
            p.to_path_buf()
        } else {
            root.join(p)
        });
    }
    None
}

fn extract_github_slug_from_git_config(cfg: &str) -> Option<String> {
    let mut in_origin = false;
    for line in cfg.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_origin = trimmed == "[remote \"origin\"]";
            continue;
        }
        if in_origin && let Some(url) = trimmed.strip_prefix("url =") {
            return parse_github_repo_slug(url.trim());
        }
    }
    None
}

pub(crate) fn parse_github_repo_slug(url: &str) -> Option<String> {
    let stripped = url.trim().trim_end_matches(".git");
    let rest = stripped
        .strip_prefix("https://github.com/")
        .or_else(|| stripped.strip_prefix("http://github.com/"))
        .or_else(|| stripped.strip_prefix("git@github.com:"))
        .or_else(|| {
            let (_, after_colon) = stripped.split_once(':')?;
            if after_colon.contains('/') && !after_colon.contains("://") {
                Some(after_colon)
            } else {
                None
            }
        })?;
    let mut parts = rest.split('/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    if owner.is_empty() || repo.is_empty() || parts.next().is_some() {
        return None;
    }
    Some(format!("{owner}/{repo}"))
}

fn probe_github_workflows(root: &Path, facts: &mut ProjectFacts) -> Result<(), RuntimeError> {
    let dir = root.join(".github").join("workflows");
    if !dir.is_dir() {
        return Ok(());
    }
    let entries =
        fs::read_dir(&dir).map_err(|err| RuntimeError::io(dir.display().to_string(), &err))?;
    let mut workflows = Vec::new();

    for entry_res in entries {
        let entry = entry_res.map_err(|err| RuntimeError::io(dir.display().to_string(), &err))?;
        let path = entry.path();
        let is_yaml = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("yml") || e.eq_ignore_ascii_case("yaml"));
        if !is_yaml || !path.is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let raw = fs::read_to_string(&path)
            .map_err(|err| RuntimeError::io(path.display().to_string(), &err))?;
        workflows.push(parse_workflow_fact(file_name, &raw));
    }

    workflows.sort_by(|a, b| a.file.cmp(&b.file));
    facts.ci_workflows = workflows;
    Ok(())
}

fn parse_workflow_fact(file: String, raw: &str) -> CiWorkflowFact {
    let mut name = None;
    let mut jobs = Vec::new();
    let mut in_jobs = false;

    for line in raw.lines() {
        let indent = line.bytes().take_while(|b| *b == b' ').count();
        let trimmed = line.trim();
        if indent == 0 {
            if let Some(wf_name) = trimmed.strip_prefix("name:") {
                name = Some(
                    wf_name
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_owned(),
                );
            }
            in_jobs = trimmed == "jobs:";
        } else if in_jobs
            && indent == 4
            && let Some(job_name) = trimmed.strip_prefix("name:")
        {
            let clean = job_name.trim().trim_matches('"').trim_matches('\'');
            if !clean.is_empty() {
                jobs.push(clean.to_owned());
            }
        }
    }

    CiWorkflowFact { file, name, jobs }
}

fn run_git_capture(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn is_sensitive_working_tree_file(path: &str) -> bool {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path);
    SENSITIVE_FILENAMES.contains(&file_name)
        || Path::new(path).extension().is_some_and(|e| {
            e.eq_ignore_ascii_case("pem")
                || e.eq_ignore_ascii_case("key")
                || e.eq_ignore_ascii_case("p12")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_remote_slugs_across_ssh_and_https_formats() {
        assert_eq!(
            parse_github_repo_slug("https://github.com/lazyaix/lazyagents.git"),
            Some("lazyaix/lazyagents".to_owned())
        );
        assert_eq!(
            parse_github_repo_slug("git@github.com:lazyaix/lazyagents.git"),
            Some("lazyaix/lazyagents".to_owned())
        );
        assert_eq!(
            parse_github_repo_slug("github-lazyaix:lazyaix/lazyagents.git"),
            Some("lazyaix/lazyagents".to_owned())
        );
    }
}
