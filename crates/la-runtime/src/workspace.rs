//! Workspace discovery, layered `.lazyagents/` loading, default manifest scaffolding,
//! and end-to-end compile/plan/lint orchestration backed by `la-core`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use la_core::engineering::VerifyCommand;
use la_core::manifest::{GitOutputMode, InstructionSection, InstructionSpec, PackRef, PackSource};
use la_core::{
    CompileInputs, CompileOutput, ConfigScope, DetectedLanguage, LintInputs, LintReport, Lockfile,
    MergePolicy, ProjectFacts, ProjectManifest, ResolvedConfig, ResourceId, ScopeResolver,
    ScopedLayer, TargetKind, WritePlan, compile_all, lint_project, plan_writes,
};

use crate::error::RuntimeError;
use crate::pack_loader::load_local_packs;
use crate::prober::probe_repository;
use crate::writer::write_atomic;

/// Repository-relative path to the canonical project manifest.
pub const MANIFEST_REL_PATH: &str = ".lazyagents/lazyagents.toml";

/// Repository-relative path to the optional gitignored developer-local override manifest.
pub const LOCAL_MANIFEST_REL_PATH: &str = ".lazyagents/local.toml";

/// Repository-relative path to the managed SHA-256 ownership lockfile.
pub const LOCKFILE_REL_PATH: &str = ".lazyagents/lock.json";

/// Complete loaded `.lazyagents/` workspace state ready for compilation, linting, or TUI editing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedWorkspace {
    /// Absolute or canonical workspace root directory.
    pub root: PathBuf,
    /// Raw text of `.lazyagents/lazyagents.toml` (for secret scanning).
    pub raw_manifest_toml: String,
    /// Scope-resolved effective configuration and provenance map.
    pub resolved: ResolvedConfig,
    /// Offline probed repository metadata.
    pub facts: ProjectFacts,
    /// Source files loaded from local packs and `.lazyagents/` (`.lazyagents/` overrides packs).
    pub source_files: BTreeMap<String, String>,
    /// Existing managed/governance files currently on disk (for diffing, includes, and linting).
    pub workspace_files: BTreeMap<String, String>,
    /// Previously persisted `.lazyagents/lock.json` if present.
    pub lockfile: Option<Lockfile>,
}

/// Loads `.lazyagents/lazyagents.toml` (and optional `.lazyagents/local.toml`), probes repo facts,
/// resolves scopes via [`ScopeResolver`], and reads `.lazyagents/` source files + relevant disk files.
///
/// # Errors
///
/// Returns [`RuntimeError::ManifestNotFound`] if `.lazyagents/lazyagents.toml` is missing,
/// or [`RuntimeError::Core`] / [`RuntimeError::Io`] / [`RuntimeError::Pack`] on parse or I/O failures.
pub fn load_workspace(root: &Path, include_local: bool) -> Result<LoadedWorkspace, RuntimeError> {
    let manifest_path = root.join(MANIFEST_REL_PATH);
    if !manifest_path.is_file() {
        return Err(RuntimeError::ManifestNotFound {
            root: root.display().to_string(),
        });
    }

    let raw_manifest_toml = fs::read_to_string(&manifest_path)
        .map_err(|err| RuntimeError::io(manifest_path.display().to_string(), &err))?;
    let project_manifest = ProjectManifest::from_toml_str(&raw_manifest_toml)?;

    let mut layers = vec![ScopedLayer::new(
        ConfigScope::Project,
        MergePolicy::Append,
        project_manifest,
    )];

    if include_local {
        let local_path = root.join(LOCAL_MANIFEST_REL_PATH);
        if local_path.is_file() {
            let local_raw = fs::read_to_string(&local_path)
                .map_err(|err| RuntimeError::io(local_path.display().to_string(), &err))?;
            let local_manifest = ProjectManifest::from_toml_str(&local_raw)?;
            layers.push(ScopedLayer::new(
                ConfigScope::ProjectLocal,
                MergePolicy::Override,
                local_manifest,
            ));
        }
    }

    let resolved = ScopeResolver::resolve(layers)?;
    let facts = probe_repository(root)?;

    let mut source_files = load_local_packs(root, &resolved.manifest.packs)?;
    let lazyagents_dir = root.join(".lazyagents");
    collect_lazyagents_sources(&lazyagents_dir, &lazyagents_dir, &mut source_files)?;

    let lockfile = load_lockfile(root)?;
    let workspace_files = collect_workspace_files(root, &resolved.manifest, lockfile.as_ref())?;

    Ok(LoadedWorkspace {
        root: root.to_path_buf(),
        raw_manifest_toml,
        resolved,
        facts,
        source_files,
        workspace_files,
        lockfile,
    })
}

/// Builds a sensible initial [`ProjectManifest`] pre-populated from probed [`ProjectFacts`].
#[must_use]
pub fn build_initial_manifest(facts: &ProjectFacts, git_mode: GitOutputMode) -> ProjectManifest {
    let mut manifest = ProjectManifest::default();
    manifest.project.name.clone_from(&facts.name);
    manifest.project.git_mode = git_mode;

    if let Some(branch) = &facts.git_default_branch {
        manifest.engineering.git.default_branch.clone_from(branch);
    }
    if let Some(repo) = &facts.git_remote_repo {
        manifest.engineering.github.enabled = true;
        manifest.engineering.github.repo = Some(repo.clone());
        manifest.engineering.github.sync.lint_workflows = facts.has_github_actions();
    } else if facts.has_github_actions() {
        manifest.engineering.github.enabled = true;
        manifest.engineering.github.sync.lint_workflows = true;
    }

    populate_verify_commands(facts, &mut manifest);

    let mut sections = vec![
        InstructionSection::Preset {
            preset: "builtin:engineering/git-workflow".to_owned(),
            when: None,
        },
        InstructionSection::Preset {
            preset: "builtin:engineering/github-hygiene".to_owned(),
            when: Some("project.has_github_actions".to_owned()),
        },
        InstructionSection::Preset {
            preset: "builtin:engineering/verify-and-supply-chain".to_owned(),
            when: None,
        },
    ];

    for lang in &facts.languages {
        let (pack_id, pack_name, section_preset) = match lang {
            DetectedLanguage::Rust => ("lang-rust", "lang/rust", "builtin:lang/rust/core"),
            DetectedLanguage::TypeScript => (
                "lang-typescript",
                "lang/typescript",
                "builtin:lang/typescript/core",
            ),
            DetectedLanguage::Python => ("lang-python", "lang/python", "builtin:lang/python/core"),
            DetectedLanguage::Go => ("lang-go", "lang/go", "builtin:lang/go/core"),
        };
        if let Ok(id) = ResourceId::parse(pack_id) {
            manifest.packs.push(PackRef {
                id,
                source: PackSource::Builtin {
                    name: pack_name.to_owned(),
                },
                include_only: Vec::new(),
                exclude: Vec::new(),
            });
        }
        sections.push(InstructionSection::Preset {
            preset: section_preset.to_owned(),
            when: None,
        });
    }

    manifest.instructions.push(InstructionSpec {
        output: "AGENTS.md".to_owned(),
        auto_probe: true,
        sections,
    });

    manifest
}

/// Atomically saves a [`ProjectManifest`] to `.lazyagents/lazyagents.toml` (or `.lazyagents/local.toml`).
///
/// # Errors
///
/// Returns [`RuntimeError::Core`] if TOML serialization fails or [`RuntimeError::Io`] on disk failure.
pub fn save_manifest(
    root: &Path,
    manifest: &ProjectManifest,
    local: bool,
) -> Result<(), RuntimeError> {
    manifest.validate_invariants()?;
    let toml_text = manifest.to_toml_pretty()?;
    let rel = if local {
        LOCAL_MANIFEST_REL_PATH
    } else {
        MANIFEST_REL_PATH
    };
    write_atomic(&root.join(rel), &toml_text)
}

impl LoadedWorkspace {
    /// Compiles all configured targets and plans deterministic disk writes against `lock.json`.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError::Core`] if template rendering, compilation, or diff planning fails.
    pub fn compile_and_plan(
        &self,
        target_filter: Option<&[TargetKind]>,
    ) -> Result<(CompileOutput, WritePlan), RuntimeError> {
        let inputs = CompileInputs {
            facts: self.facts.clone(),
            source_files: self.source_files.clone(),
            target_filter,
        };
        let compiled = compile_all(&self.resolved.manifest, &inputs)?;
        let plan = plan_writes(
            &compiled.files,
            self.lockfile.as_ref(),
            &self.workspace_files,
        )?;
        Ok((compiled, plan))
    }

    /// Runs the full compilation, drift plan, budget telemetry, and `LA001`–`LA017` compliance check.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError::Core`] if compilation or diff planning fails.
    pub fn check(
        &self,
        target_filter: Option<&[TargetKind]>,
    ) -> Result<(CompileOutput, WritePlan, LintReport), RuntimeError> {
        let (compiled, plan) = self.compile_and_plan(target_filter)?;
        let report = lint_project(&LintInputs {
            manifest_toml: &self.raw_manifest_toml,
            manifest: &self.resolved.manifest,
            source_files: &self.source_files,
            compile_output: &compiled,
            write_plan: Some(&plan),
            workspace_files: &self.workspace_files,
        });
        Ok((compiled, plan, report))
    }
}

fn populate_verify_commands(facts: &ProjectFacts, manifest: &mut ProjectManifest) {
    let candidates: [(&str, &str, &Option<String>); 4] = [
        ("fmt", "Format", &facts.commands.fmt),
        ("lint", "Lint", &facts.commands.lint),
        ("test", "Test", &facts.commands.test),
        ("doc", "Docs", &facts.commands.doc),
    ];
    for (id_str, label, cmd_opt) in candidates {
        if let Some(cmd) = cmd_opt
            && let Ok(id) = ResourceId::parse(id_str)
        {
            manifest.engineering.verify.commands.push(VerifyCommand {
                id,
                label: label.to_owned(),
                cmd: cmd.clone(),
                required: true,
            });
        }
    }
}

fn load_lockfile(root: &Path) -> Result<Option<Lockfile>, RuntimeError> {
    let path = root.join(LOCKFILE_REL_PATH);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .map_err(|err| RuntimeError::io(path.display().to_string(), &err))?;
    Ok(Some(Lockfile::from_json_str(&raw)?))
}

fn collect_lazyagents_sources(
    base: &Path,
    dir: &Path,
    out: &mut BTreeMap<String, String>,
) -> Result<(), RuntimeError> {
    if !dir.is_dir() {
        return Ok(());
    }
    let entries =
        fs::read_dir(dir).map_err(|err| RuntimeError::io(dir.display().to_string(), &err))?;
    for entry_res in entries {
        let entry = entry_res.map_err(|err| RuntimeError::io(dir.display().to_string(), &err))?;
        let path = entry.path();
        if path.is_dir() {
            collect_lazyagents_sources(base, &path, out)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let Ok(rel) = path.strip_prefix(base) else {
            continue;
        };
        let rel_posix = rel.to_string_lossy().replace('\\', "/");
        if matches!(
            rel_posix.as_str(),
            "lazyagents.toml" | "local.toml" | "lock.json"
        ) {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&path) {
            out.insert(rel_posix, text);
        }
    }
    Ok(())
}

fn collect_workspace_files(
    root: &Path,
    manifest: &ProjectManifest,
    lockfile: Option<&Lockfile>,
) -> Result<BTreeMap<String, String>, RuntimeError> {
    let mut files = BTreeMap::new();

    for static_rel in [
        "AGENTS.md",
        "GEMINI.md",
        ".gitignore",
        ".github/CODEOWNERS",
        ".github/pull_request_template.md",
        ".github/dependabot.yml",
        "CONTRIBUTING.md",
        "SECURITY.md",
    ] {
        read_if_exists(root, static_rel, &mut files)?;
    }

    for inst in &manifest.instructions {
        read_if_exists(root, &inst.output, &mut files)?;
    }

    if let Some(lock) = lockfile {
        for locked_path in lock.entries.keys() {
            read_if_exists(root, locked_path, &mut files)?;
        }
    }

    collect_dir_text_files(root, &root.join(".agents"), &mut files)?;
    collect_dir_text_files(root, &root.join(".github").join("workflows"), &mut files)?;
    collect_dir_text_files(
        root,
        &root.join(".github").join("ISSUE_TEMPLATE"),
        &mut files,
    )?;
    collect_dir_text_files(root, &root.join("docs"), &mut files)?;

    Ok(files)
}

fn read_if_exists(
    root: &Path,
    rel: &str,
    out: &mut BTreeMap<String, String>,
) -> Result<(), RuntimeError> {
    let full = root.join(rel);
    if !full.is_file() {
        return Ok(());
    }
    let content = fs::read_to_string(&full)
        .map_err(|err| RuntimeError::io(full.display().to_string(), &err))?;
    out.insert(rel.to_owned(), content);
    Ok(())
}

fn collect_dir_text_files(
    root: &Path,
    dir: &Path,
    out: &mut BTreeMap<String, String>,
) -> Result<(), RuntimeError> {
    if !dir.is_dir() {
        return Ok(());
    }
    let entries =
        fs::read_dir(dir).map_err(|err| RuntimeError::io(dir.display().to_string(), &err))?;
    for entry_res in entries {
        let entry = entry_res.map_err(|err| RuntimeError::io(dir.display().to_string(), &err))?;
        let path = entry.path();
        if path.is_dir() {
            collect_dir_text_files(root, &path, out)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        let rel_posix = rel.to_string_lossy().replace('\\', "/");
        if let Ok(content) = fs::read_to_string(&path) {
            out.insert(rel_posix, content);
        }
    }
    Ok(())
}
