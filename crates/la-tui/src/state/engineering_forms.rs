//! Structured shadowed modal form builders and submit appliers for all 7 `[engineering]`
//! governance domains (`Git`, `ProtectedPaths`, `Verify`, `GitHub`, `SupplyChain`,
//! `Architecture`, `Release`).

mod git_github;
mod quality;

use la_core::{PolicyDecision, ProjectManifest};

use crate::action::StudioCommand;
use crate::form::ModalFormState;
use crate::i18n::I18n;
use crate::state::{FormTarget, GovernanceDomain, WorkspaceSnapshot};

/// Builds a modal form when pressing `a` inside `NavSection::Engineering`.
#[must_use]
pub(super) fn build_engineering_add_form(
    domain: GovernanceDomain,
    manifest: &ProjectManifest,
    i18n: I18n,
) -> (ModalFormState, FormTarget) {
    match domain {
        GovernanceDomain::Verify => quality::build_verify_command_form(None, manifest, i18n),
        GovernanceDomain::ProtectedPaths => {
            let (mut form, target) = git_github::build_protected_paths_form(manifest, i18n);
            focus_field_by_key(&mut form, "entries");
            (form, target)
        }
        GovernanceDomain::Architecture => {
            let (mut form, target) = quality::build_architecture_form(manifest, i18n);
            focus_field_by_key(&mut form, "layers");
            (form, target)
        }
        other => build_engineering_domain_form(other, manifest, i18n),
    }
}

/// Builds a modal form when pressing `E` inside `NavSection::Engineering`.
#[must_use]
pub(super) fn build_engineering_edit_form(
    domain: GovernanceDomain,
    row_idx: usize,
    manifest: &ProjectManifest,
    i18n: I18n,
) -> (ModalFormState, FormTarget) {
    if domain == GovernanceDomain::Verify
        && row_idx > 0
        && row_idx.saturating_sub(1) < manifest.engineering.verify.commands.len()
    {
        return quality::build_verify_command_form(Some(row_idx.saturating_sub(1)), manifest, i18n);
    }
    build_engineering_domain_form(domain, manifest, i18n)
}

/// Builds the full structured shadowed modal form for one [`GovernanceDomain`].
#[must_use]
pub(super) fn build_engineering_domain_form(
    domain: GovernanceDomain,
    manifest: &ProjectManifest,
    i18n: I18n,
) -> (ModalFormState, FormTarget) {
    match domain {
        GovernanceDomain::Git => git_github::build_git_form(manifest, i18n),
        GovernanceDomain::ProtectedPaths => git_github::build_protected_paths_form(manifest, i18n),
        GovernanceDomain::Verify => quality::build_verify_domain_form(manifest, i18n),
        GovernanceDomain::GitHub => git_github::build_github_form(manifest, i18n),
        GovernanceDomain::SupplyChain => quality::build_supply_chain_form(manifest, i18n),
        GovernanceDomain::Architecture => quality::build_architecture_form(manifest, i18n),
        GovernanceDomain::Release => quality::build_release_form(manifest, i18n),
    }
}

/// Applies a validated [`GovernanceDomain`] modal form to `workspace.manifest.engineering`.
pub(super) fn apply_engineering_domain(
    domain: GovernanceDomain,
    form: &ModalFormState,
    workspace: &mut WorkspaceSnapshot,
) -> Result<(String, Vec<StudioCommand>), String> {
    let summary = match domain {
        GovernanceDomain::Git => git_github::apply_git(form, &mut workspace.manifest.engineering)?,
        GovernanceDomain::ProtectedPaths => {
            git_github::apply_protected_paths(form, &mut workspace.manifest.engineering)?
        }
        GovernanceDomain::Verify => {
            quality::apply_verify_domain(form, &mut workspace.manifest.engineering)
        }
        GovernanceDomain::GitHub => {
            git_github::apply_github(form, &mut workspace.manifest.engineering)?
        }
        GovernanceDomain::SupplyChain => {
            quality::apply_supply_chain(form, &mut workspace.manifest.engineering)?
        }
        GovernanceDomain::Architecture => {
            quality::apply_architecture(form, &mut workspace.manifest.engineering)?
        }
        GovernanceDomain::Release => {
            quality::apply_release(form, &mut workspace.manifest.engineering)?
        }
    };
    Ok((summary, Vec::new()))
}

/// Applies a dedicated `VerifyCommand` add/edit modal form to `workspace.manifest.engineering.verify`.
pub(super) fn apply_verify_command(
    edit_idx: Option<usize>,
    form: &ModalFormState,
    workspace: &mut WorkspaceSnapshot,
) -> Result<(String, Vec<StudioCommand>), String> {
    let summary =
        quality::apply_verify_command(edit_idx, form, &mut workspace.manifest.engineering)?;
    Ok((summary, Vec::new()))
}

pub(super) const fn decision_options() -> [&'static str; 4] {
    ["allow", "ask", "force_ask", "deny"]
}

pub(super) const fn decision_to_idx(decision: PolicyDecision) -> usize {
    match decision {
        PolicyDecision::Allow => 0,
        PolicyDecision::Ask => 1,
        PolicyDecision::ForceAsk => 2,
        PolicyDecision::Deny => 3,
    }
}

pub(super) fn parse_decision(raw: &str) -> Result<PolicyDecision, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "allow" => Ok(PolicyDecision::Allow),
        "ask" => Ok(PolicyDecision::Ask),
        "force_ask" | "force-ask" => Ok(PolicyDecision::ForceAsk),
        "deny" => Ok(PolicyDecision::Deny),
        other => Err(format!(
            "Invalid policy decision `{other}` (expected allow, ask, force_ask, or deny)"
        )),
    }
}

pub(super) fn opt_text(form: &ModalFormState, key: &str) -> Option<String> {
    form.text_value(key)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

pub(super) fn split_csv(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

fn focus_field_by_key(form: &mut ModalFormState, key: &str) {
    if let Some((idx, field)) = form.fields.iter().enumerate().find(|(_, f)| f.key == key) {
        form.active_tab = field.tab;
        form.focused_field = idx;
    }
}
