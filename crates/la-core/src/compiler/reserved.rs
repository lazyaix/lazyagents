//! Reserved cross-agent target adapter stubs (`ClaudeCode`, `Codex`, `Cursor`, `GitHubCopilot`)
//! emitting explicit [`CompatibilityDiagnostic`] reports until Slice 4 adapters ship.

use crate::compiler::{CompatibilityDiagnostic, CompatibilitySeverity};
use crate::id::TargetKind;
use crate::manifest::ProjectManifest;

/// Inspects reserved target toggles and emits structured compatibility diagnostics whenever
/// a post-V1 target is enabled in the manifest.
#[must_use]
pub fn compile_reserved_targets(
    manifest: &ProjectManifest,
    target_filter: Option<&[TargetKind]>,
) -> Vec<CompatibilityDiagnostic> {
    let mut diagnostics = Vec::new();
    let checks = [
        (TargetKind::ClaudeCode, manifest.targets.claude_code.enabled),
        (TargetKind::Codex, manifest.targets.codex.enabled),
        (TargetKind::Cursor, manifest.targets.cursor.enabled),
        (
            TargetKind::GitHubCopilot,
            manifest.targets.github_copilot.enabled,
        ),
    ];

    for (target, enabled) in checks {
        if !enabled {
            continue;
        }
        if target_filter.is_some_and(|filter| !filter.contains(&target)) {
            continue;
        }
        diagnostics.push(CompatibilityDiagnostic {
            target,
            severity: CompatibilitySeverity::Unsupported,
            subject: format!("target:{target}"),
            message: format!(
                "target `{target}` adapter is scheduled for Slice 4; canonical IR is preserved without emitting target files yet"
            ),
        });
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_explicit_diagnostic_when_reserved_target_enabled() {
        let mut manifest = ProjectManifest::default();
        assert_eq!(compile_reserved_targets(&manifest, None), Vec::new());

        manifest.targets.claude_code.enabled = true;
        manifest.targets.codex.enabled = true;
        manifest.targets.cursor.enabled = true;
        let diags = compile_reserved_targets(&manifest, None);
        assert_eq!(diags.len(), 3);
        assert_eq!(diags[0].target, TargetKind::ClaudeCode);
        assert_eq!(diags[1].target, TargetKind::Codex);
        assert_eq!(diags[2].target, TargetKind::Cursor);
        assert_eq!(diags[0].severity, CompatibilitySeverity::Unsupported);
    }
}
