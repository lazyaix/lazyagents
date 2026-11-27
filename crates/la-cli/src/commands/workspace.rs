//! Workspace lifecycle CLI commands: `init`, `import`, `generate`/`sync`, `diff`, `check`, `inspect`.

use std::io::Write;
use std::path::Path;

use la_core::manifest::{InstructionSection, InstructionSpec};
use la_runtime::{
    ApplyOptions, ImportSource, MANIFEST_REL_PATH, apply_write_plan, build_initial_manifest,
    import_workspace, load_workspace, probe_repository, save_manifest, write_imported_workspace,
};

use crate::cli::{
    CheckArgs, DiffArgs, GenerateArgs, ImportArgs, InitArgs, InspectArgs, OutputFormat,
};
use crate::commands::CommandOutcome;

pub(super) fn cmd_init(root: &Path, args: &InitArgs, out: &mut impl Write) -> CommandOutcome {
    let manifest_path = root.join(MANIFEST_REL_PATH);
    if manifest_path.is_file() && !args.yes {
        return CommandOutcome::RuntimeError(format!(
            "`{MANIFEST_REL_PATH}` already exists; pass `--yes` to overwrite or run `lazyagents generate`"
        ));
    }

    if args.import {
        let mut imported = match import_workspace(root, ImportSource::Auto) {
            Ok(imp) => imp,
            Err(err) => return CommandOutcome::RuntimeError(err.to_string()),
        };
        imported.manifest.project.git_mode = args.git_mode.into();
        if let Some(preset) = &args.preset {
            append_instruction_preset(&mut imported.manifest, preset);
        }
        if let Err(err) = write_imported_workspace(root, &imported) {
            return CommandOutcome::RuntimeError(err.to_string());
        }
        let _ = writeln!(
            out,
            "Initialized `{MANIFEST_REL_PATH}` (imported existing config):"
        );
        for note in &imported.notes {
            let _ = writeln!(out, "  - {note}");
        }
        return CommandOutcome::Pass;
    }

    let facts = match probe_repository(root) {
        Ok(f) => f,
        Err(err) => return CommandOutcome::RuntimeError(err.to_string()),
    };
    let mut manifest = build_initial_manifest(&facts, args.git_mode.into());
    if let Some(preset) = &args.preset {
        append_instruction_preset(&mut manifest, preset);
    }

    if let Err(err) = save_manifest(root, &manifest, false) {
        return CommandOutcome::RuntimeError(err.to_string());
    }

    let _ = writeln!(
        out,
        "Initialized `{MANIFEST_REL_PATH}` for `{}` ({} language(s) detected).",
        manifest.project.name,
        facts.languages.len()
    );
    let _ = writeln!(
        out,
        "Next: run `lazyagents generate` to compile target outputs."
    );
    CommandOutcome::Pass
}

pub(super) fn cmd_import(root: &Path, args: &ImportArgs, out: &mut impl Write) -> CommandOutcome {
    let source: ImportSource = args.from.into();
    let imported = match import_workspace(root, source) {
        Ok(imp) => imp,
        Err(err) => return CommandOutcome::RuntimeError(err.to_string()),
    };

    if args.dry_run {
        let _ = writeln!(out, "Dry run — would import into `.lazyagents/`:");
    } else {
        if let Err(err) = write_imported_workspace(root, &imported) {
            return CommandOutcome::RuntimeError(err.to_string());
        }
        let _ = writeln!(out, "Imported workspace configuration into `.lazyagents/`:");
    }

    if imported.notes.is_empty() {
        let _ = writeln!(out, "  (no existing agent files discovered)");
    } else {
        for note in &imported.notes {
            let _ = writeln!(out, "  - {note}");
        }
    }
    CommandOutcome::Pass
}

pub(super) fn cmd_generate(
    root: &Path,
    args: &GenerateArgs,
    out: &mut impl Write,
) -> CommandOutcome {
    let ws = match load_workspace(root, args.include_local) {
        Ok(ws) => ws,
        Err(err) => return CommandOutcome::RuntimeError(err.to_string()),
    };
    let (compiled, plan) = match ws.compile_and_plan(args.target.as_filter()) {
        Ok(res) => res,
        Err(err) => return CommandOutcome::RuntimeError(err.to_string()),
    };

    for compat in &compiled.diagnostics {
        let _ = writeln!(
            out,
            "warn[{}] {}: {}",
            compat.target, compat.subject, compat.message
        );
    }

    let summary = match apply_write_plan(
        root,
        &plan,
        ApplyOptions {
            force: args.force,
            dry_run: args.dry_run,
        },
    ) {
        Ok(s) => s,
        Err(err) => return CommandOutcome::RuntimeError(err.to_string()),
    };

    let verb = if args.dry_run {
        "Would apply"
    } else {
        "Generated"
    };
    let _ = writeln!(
        out,
        "{verb}: {} created, {} updated, {} deleted, {} unchanged.",
        summary.created.len(),
        summary.updated.len(),
        summary.deleted.len(),
        summary.unchanged.len()
    );
    for path in &summary.created {
        let _ = writeln!(out, "  + {path}");
    }
    for path in &summary.updated {
        let _ = writeln!(out, "  ~ {path}");
    }
    for path in &summary.deleted {
        let _ = writeln!(out, "  - {path}");
    }
    CommandOutcome::Pass
}

pub(super) fn cmd_diff(root: &Path, args: &DiffArgs, out: &mut impl Write) -> CommandOutcome {
    let ws = match load_workspace(root, args.include_local) {
        Ok(ws) => ws,
        Err(err) => return CommandOutcome::RuntimeError(err.to_string()),
    };
    let (_, plan) = match ws.compile_and_plan(args.target.as_filter()) {
        Ok(res) => res,
        Err(err) => return CommandOutcome::RuntimeError(err.to_string()),
    };

    let mut any_diff = false;
    for file in &plan.files {
        if !file.unified_diff.is_empty() {
            any_diff = true;
            let _ = write!(out, "{}", file.unified_diff);
        }
    }
    if !any_diff {
        let _ = writeln!(out, "No differences (0 drift).");
    }

    if args.exit_code && any_diff {
        CommandOutcome::CheckFailed
    } else {
        CommandOutcome::Pass
    }
}

pub(super) fn cmd_check(root: &Path, args: &CheckArgs, out: &mut impl Write) -> CommandOutcome {
    let ws = match load_workspace(root, args.include_local) {
        Ok(ws) => ws,
        Err(err) => return CommandOutcome::RuntimeError(err.to_string()),
    };
    let (_, _, report) = match ws.check(args.target.as_filter()) {
        Ok(res) => res,
        Err(err) => return CommandOutcome::RuntimeError(err.to_string()),
    };

    match args.format {
        OutputFormat::Text => {
            let _ = write!(out, "{}", report.format_text());
        }
        OutputFormat::Github => {
            let annotations = report.format_github_annotations();
            if annotations.is_empty() {
                let _ = writeln!(out, "OK: 0 lint diagnostics");
            } else {
                let _ = write!(out, "{annotations}");
            }
        }
        OutputFormat::Json => match report.to_json_pretty() {
            Ok(json) => {
                let _ = write!(out, "{json}");
            }
            Err(err) => return CommandOutcome::RuntimeError(err.to_string()),
        },
    }

    if report.has_errors(args.strict) {
        CommandOutcome::CheckFailed
    } else {
        CommandOutcome::Pass
    }
}

pub(super) fn cmd_inspect(root: &Path, args: &InspectArgs, out: &mut impl Write) -> CommandOutcome {
    let ws = match load_workspace(root, args.include_local) {
        Ok(ws) => ws,
        Err(err) => return CommandOutcome::RuntimeError(err.to_string()),
    };
    let (compiled, plan, report) = match ws.check(None) {
        Ok(res) => res,
        Err(err) => return CommandOutcome::RuntimeError(err.to_string()),
    };

    if args.agbom {
        let agbom = la_core::build_agbom(&ws.resolved, &compiled, &ws.source_files);
        if args.format == OutputFormat::Json {
            return match agbom.to_json_pretty() {
                Ok(json) => {
                    let _ = write!(out, "{json}");
                    CommandOutcome::Pass
                }
                Err(err) => CommandOutcome::RuntimeError(err.to_string()),
            };
        }
        let _ = write!(out, "{}", agbom.format_text());
        return CommandOutcome::Pass;
    }

    if args.format == OutputFormat::Json {
        let payload = serde_json::json!({
            "project": ws.resolved.manifest.project,
            "active_scopes": ws.resolved.active_scopes,
            "provenance": ws.resolved.provenance,
            "facts": ws.facts,
            "budget": report.budget,
            "lethal_trifecta": report.lethal_trifecta,
            "has_drift": plan.has_drift(),
            "diagnostics_count": report.diagnostics.len(),
        });
        match serde_json::to_string_pretty(&payload) {
            Ok(json) => {
                let _ = writeln!(out, "{json}");
                return CommandOutcome::Pass;
            }
            Err(err) => return CommandOutcome::RuntimeError(err.to_string()),
        }
    }

    let _ = writeln!(out, "Project: {}", ws.resolved.manifest.project.name);
    let _ = writeln!(out, "Scopes: {:?}", ws.resolved.active_scopes);
    let _ = writeln!(out, "Languages: {:?}", ws.facts.languages);
    let _ = writeln!(
        out,
        "Always-On Budget: {}/{} tokens ({:?}, pull_ratio={}%)",
        report.budget.always_on_tokens,
        report.budget.max_always_on_tokens,
        report.budget.always_on_status,
        report.budget.push_pull.pull_ratio_percent
    );
    let _ = writeln!(
        out,
        "Lethal Trifecta Risk: {:?}",
        report.lethal_trifecta.overall_risk
    );
    let _ = writeln!(
        out,
        "Managed Outputs: {} (drift={})",
        plan.files.len(),
        plan.has_drift()
    );
    let _ = writeln!(out, "Diagnostics: {}", report.diagnostics.len());
    CommandOutcome::Pass
}

fn append_instruction_preset(manifest: &mut la_core::ProjectManifest, preset: &str) {
    let section = InstructionSection::Preset {
        preset: preset.to_owned(),
        when: None,
    };
    if let Some(first) = manifest.instructions.first_mut() {
        first.sections.push(section);
    } else {
        manifest.instructions.push(InstructionSpec {
            output: "AGENTS.md".to_owned(),
            auto_probe: true,
            sections: vec![section],
        });
    }
}
