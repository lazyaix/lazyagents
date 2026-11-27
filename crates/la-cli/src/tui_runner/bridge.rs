//! Structured async bridge executing [`la_tui::StudioCommand`] via `la-runtime` on
//! `tokio::task::spawn_blocking` tracked by [`TaskTracker`] and [`CancellationToken`].

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use la_core::ProjectManifest;
use la_core::manifest::GitOutputMode;
use la_runtime::{
    ApplyOptions, AuditCheckStatus, GitHubAuditReport, ImportSource, LoadedWorkspace,
    apply_write_plan, audit_github_repo, build_initial_manifest, import_workspace, load_workspace,
    probe_repository, save_manifest, write_atomic, write_imported_workspace,
};
use la_tui::{
    Action, AuditCheckView, AuditStatusView, DecodedRgbaImage, GitHubAuditSnapshot, StudioCommand,
    ToastLevel, WorkspaceSnapshot,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{info, warn};

use crate::prefs::{preferences_path, save_preferences_to};

/// Loads a workspace snapshot for TUI startup or live reload. If `.lazyagents/` is not yet
/// initialized, probes repo facts and returns an uninitialized snapshot to open the Init Wizard.
#[must_use]
pub(crate) fn load_tui_workspace_snapshot(root: &Path) -> WorkspaceSnapshot {
    if let Ok(loaded) = load_workspace(root, true) {
        return convert_loaded_workspace(loaded);
    }

    let facts = probe_repository(root).unwrap_or_default();
    let manifest = build_initial_manifest(&facts, GitOutputMode::Commit);
    WorkspaceSnapshot::from_manifest(
        root.display().to_string(),
        false,
        manifest,
        facts,
        BTreeMap::new(),
        BTreeMap::new(),
        None,
    )
}

/// Spawns off-thread plugin logo decoding tasks for every plugin declaring a relative `logo` path.
pub(crate) fn spawn_plugin_logo_loaders(
    root: &Path,
    manifest: &ProjectManifest,
    tx: &mpsc::Sender<Action>,
    tracker: &TaskTracker,
    cancel: &CancellationToken,
) {
    for plugin in &manifest.plugins {
        let Some(logo_rel) = plugin.logo.clone() else {
            continue;
        };
        let plugin_name = plugin.name.as_str().to_owned();
        let full_path = root.join(".lazyagents").join(&logo_rel);
        let tx_clone = tx.clone();
        let token = cancel.clone();
        tracker.spawn(async move {
            if token.is_cancelled() {
                return;
            }
            let decoded = tokio::task::spawn_blocking(move || {
                let bytes = fs::read(&full_path).ok()?;
                DecodedRgbaImage::decode_bytes(&bytes).ok()
            })
            .await
            .ok()
            .flatten();

            if let Some(image) = decoded {
                let _ = tx_clone
                    .send(Action::PluginLogoDecoded { plugin_name, image })
                    .await;
            }
        });
    }
}

/// Dispatches an asynchronous [`StudioCommand`] onto the structured [`TaskTracker`].
pub(crate) fn spawn_studio_command(
    cmd: StudioCommand,
    root: &Path,
    tx: &mpsc::Sender<Action>,
    tracker: &TaskTracker,
    cancel: &CancellationToken,
) {
    let ctx = SpawnCtx {
        root: root.to_path_buf(),
        tx: tx.clone(),
        cancel: cancel.clone(),
    };
    match cmd {
        StudioCommand::SaveManifest(manifest) => spawn_save_manifest(ctx, tracker, *manifest),
        StudioCommand::SaveSourceFile { rel_path, content } => {
            spawn_save_source(ctx, tracker, rel_path, content);
        }
        StudioCommand::ApplySync { force } => spawn_apply_sync(ctx, tracker, force),
        StudioCommand::InitializeWorkspace {
            git_mode,
            import_existing,
        } => spawn_init_workspace(ctx, tracker, git_mode, import_existing),
        StudioCommand::RunGitHubAudit => spawn_github_audit(ctx, tracker),
        StudioCommand::ReloadWorkspace => spawn_reload_workspace(ctx, tracker),
        StudioCommand::SavePreferences(prefs) => spawn_save_preferences(ctx, tracker, prefs),
        StudioCommand::LoadPluginLogo {
            plugin_name,
            rel_path,
        } => spawn_single_logo_loader(ctx, tracker, plugin_name, rel_path),
        StudioCommand::OpenExternalEditor { .. }
        | StudioCommand::CopyToClipboard(_)
        | StudioCommand::Quit => {}
    }
}

struct SpawnCtx {
    root: PathBuf,
    tx: mpsc::Sender<Action>,
    cancel: CancellationToken,
}

fn spawn_save_manifest(ctx: SpawnCtx, tracker: &TaskTracker, manifest: ProjectManifest) {
    tracker.spawn(async move {
        if ctx.cancel.is_cancelled() {
            return;
        }
        let res =
            tokio::task::spawn_blocking(move || save_manifest(&ctx.root, &manifest, false)).await;
        match res {
            Ok(Ok(())) => info!("saved .lazyagents/lazyagents.toml"),
            Ok(Err(err)) => {
                warn!(error = %err, "failed to save manifest");
                let _ = ctx
                    .tx
                    .send(Action::Notify {
                        level: ToastLevel::Error,
                        message: err.to_string(),
                    })
                    .await;
            }
            Err(join_err) => warn!(error = %join_err, "save task panicked"),
        }
    });
}

fn spawn_save_source(ctx: SpawnCtx, tracker: &TaskTracker, rel_path: String, content: String) {
    tracker.spawn(async move {
        if ctx.cancel.is_cancelled() {
            return;
        }
        let target = ctx.root.join(".lazyagents").join(&rel_path);
        let res = tokio::task::spawn_blocking(move || write_atomic(&target, &content)).await;
        if let Ok(Err(err)) = res {
            let _ = ctx
                .tx
                .send(Action::Notify {
                    level: ToastLevel::Error,
                    message: err.to_string(),
                })
                .await;
        } else {
            info!(path = %rel_path, "saved .lazyagents source file");
        }
    });
}

fn spawn_apply_sync(ctx: SpawnCtx, tracker: &TaskTracker, force: bool) {
    tracker.spawn(async move {
        if ctx.cancel.is_cancelled() {
            return;
        }
        let res = tokio::task::spawn_blocking(move || run_sync(&ctx.root, force)).await;
        match res {
            Ok(Ok((created, updated, deleted))) => {
                info!(created, updated, deleted, "sync applied");
                let _ = ctx
                    .tx
                    .send(Action::SyncCompleted {
                        created,
                        updated,
                        deleted,
                    })
                    .await;
            }
            Ok(Err(msg)) => {
                warn!(error = %msg, "sync failed");
                let _ = ctx.tx.send(Action::SyncFailed(msg)).await;
            }
            Err(err) => {
                let _ = ctx
                    .tx
                    .send(Action::SyncFailed(format!("sync worker failed: {err}")))
                    .await;
            }
        }
    });
}

fn spawn_init_workspace(
    ctx: SpawnCtx,
    tracker: &TaskTracker,
    git_mode: GitOutputMode,
    import_existing: bool,
) {
    tracker.spawn(async move {
        if ctx.cancel.is_cancelled() {
            return;
        }
        let res = tokio::task::spawn_blocking(move || {
            init_workspace_sync(&ctx.root, git_mode, import_existing)
        })
        .await;
        match res {
            Ok(Ok(snapshot)) => {
                info!("initialized workspace from guided wizard");
                let _ = ctx
                    .tx
                    .send(Action::WorkspaceLoaded(Box::new(snapshot)))
                    .await;
            }
            Ok(Err(msg)) => {
                let _ = ctx
                    .tx
                    .send(Action::Notify {
                        level: ToastLevel::Error,
                        message: msg,
                    })
                    .await;
            }
            Err(err) => warn!(error = %err, "init task failed"),
        }
    });
}

fn spawn_github_audit(ctx: SpawnCtx, tracker: &TaskTracker) {
    tracker.spawn(async move {
        if ctx.cancel.is_cancelled() {
            return;
        }
        let res = tokio::task::spawn_blocking(move || run_gh_audit(&ctx.root)).await;
        match res {
            Ok(Ok(report)) => {
                info!(repo = %report.repo, checks = report.checks.len(), "gh audit finished");
                let _ = ctx.tx.send(Action::GitHubAuditLoaded(report)).await;
            }
            Ok(Err(msg)) => {
                warn!(error = %msg, "gh audit failed");
                let _ = ctx.tx.send(Action::GitHubAuditFailed(msg)).await;
            }
            Err(err) => {
                let _ = ctx
                    .tx
                    .send(Action::GitHubAuditFailed(format!(
                        "audit task failed: {err}"
                    )))
                    .await;
            }
        }
    });
}

fn spawn_reload_workspace(ctx: SpawnCtx, tracker: &TaskTracker) {
    tracker.spawn(async move {
        if ctx.cancel.is_cancelled() {
            return;
        }
        let snapshot =
            tokio::task::spawn_blocking(move || load_tui_workspace_snapshot(&ctx.root)).await;
        if let Ok(ws) = snapshot {
            let _ = ctx.tx.send(Action::WorkspaceLoaded(Box::new(ws))).await;
        }
    });
}

fn spawn_save_preferences(ctx: SpawnCtx, tracker: &TaskTracker, prefs: la_tui::StudioPreferences) {
    tracker.spawn(async move {
        if ctx.cancel.is_cancelled() {
            return;
        }
        let _ = tokio::task::spawn_blocking(move || {
            if let Some(path) = preferences_path() {
                let _ = save_preferences_to(&path, prefs);
            }
        })
        .await;
    });
}

fn spawn_single_logo_loader(
    ctx: SpawnCtx,
    tracker: &TaskTracker,
    plugin_name: String,
    rel_path: String,
) {
    tracker.spawn(async move {
        if ctx.cancel.is_cancelled() {
            return;
        }
        let full_path = ctx.root.join(".lazyagents").join(rel_path);
        let decoded = tokio::task::spawn_blocking(move || {
            let bytes = fs::read(&full_path).ok()?;
            DecodedRgbaImage::decode_bytes(&bytes).ok()
        })
        .await
        .ok()
        .flatten();
        if let Some(image) = decoded {
            let _ = ctx
                .tx
                .send(Action::PluginLogoDecoded { plugin_name, image })
                .await;
        }
    });
}

fn convert_loaded_workspace(loaded: LoadedWorkspace) -> WorkspaceSnapshot {
    let mut snapshot = WorkspaceSnapshot::from_manifest(
        loaded.root.display().to_string(),
        true,
        loaded.resolved.manifest,
        loaded.facts,
        loaded.source_files,
        loaded.workspace_files,
        loaded.lockfile,
    );
    snapshot.active_scopes = loaded.resolved.active_scopes;
    snapshot.provenance = loaded.resolved.provenance;
    snapshot
}

fn run_sync(root: &Path, force: bool) -> Result<(usize, usize, usize), String> {
    let ws = load_workspace(root, true).map_err(|err| err.to_string())?;
    let (_, plan) = ws.compile_and_plan(None).map_err(|err| err.to_string())?;
    let summary = apply_write_plan(
        root,
        &plan,
        ApplyOptions {
            force,
            dry_run: false,
        },
    )
    .map_err(|err| err.to_string())?;
    Ok((
        summary.created.len(),
        summary.updated.len(),
        summary.deleted.len(),
    ))
}

fn init_workspace_sync(
    root: &Path,
    git_mode: GitOutputMode,
    import_existing: bool,
) -> Result<WorkspaceSnapshot, String> {
    if import_existing {
        let mut imported =
            import_workspace(root, ImportSource::Auto).map_err(|err| err.to_string())?;
        imported.manifest.project.git_mode = git_mode;
        write_imported_workspace(root, &imported).map_err(|err| err.to_string())?;
    } else {
        let facts = probe_repository(root).map_err(|err| err.to_string())?;
        let manifest = build_initial_manifest(&facts, git_mode);
        save_manifest(root, &manifest, false).map_err(|err| err.to_string())?;
    }
    Ok(load_tui_workspace_snapshot(root))
}

fn run_gh_audit(root: &Path) -> Result<GitHubAuditSnapshot, String> {
    let manifest = load_workspace(root, true)
        .map_or_else(|_| ProjectManifest::default(), |ws| ws.resolved.manifest);
    let report = audit_github_repo(root, &manifest, None).map_err(|err| err.to_string())?;
    Ok(convert_gh_report(report))
}

fn convert_gh_report(report: GitHubAuditReport) -> GitHubAuditSnapshot {
    GitHubAuditSnapshot {
        repo: report.repo,
        checks: report
            .checks
            .into_iter()
            .map(|c| AuditCheckView {
                id: c.id,
                label: c.label,
                status: match c.status {
                    AuditCheckStatus::Pass => AuditStatusView::Pass,
                    AuditCheckStatus::Mismatch => AuditStatusView::Mismatch,
                    AuditCheckStatus::Skipped => AuditStatusView::Skipped,
                },
                detail: c.detail,
                remediation_cmd: c.remediation_cmd,
            })
            .collect(),
    }
}
