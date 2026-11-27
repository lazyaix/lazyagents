//! Modal form builders and submit appliers for `[engineering.verify]`,
//! `[engineering.supply_chain]`, `[engineering.architecture]`, and `[engineering.release]`.

use la_core::engineering::{
    ArchitectureLayer, EngineeringPolicy, ReleaseTool, StopHookMode, VerifyCommand,
};
use la_core::{ProjectManifest, ResourceId};

use crate::form::{FieldValidator, FormField, KeyValueEntry, KeyValueListState, ModalFormState};
use crate::i18n::I18n;
use crate::state::{FormTarget, GovernanceDomain};

use super::opt_text;

pub(super) fn build_verify_domain_form(
    manifest: &ProjectManifest,
    i18n: I18n,
) -> (ModalFormState, FormTarget) {
    let mode_idx = match manifest.engineering.verify.stop_hook_mode {
        StopHookMode::Off => 0,
        StopHookMode::Remind => 1,
        StopHookMode::Enforce => 2,
    };
    let form = ModalFormState::new(
        i18n.pick("Configure [engineering.verify]", "配置本地校验门禁"),
        vec![
            FormField::enum_select(
                "stop_hook_mode",
                i18n.pick("Stop Hook Mode", "Stop 钩子模式"),
                vec!["off".into(), "remind".into(), "enforce".into()],
                mode_idx,
            )
            .with_hint(i18n.pick(
                "Tip: press `a` to add a Verify Command or select one and press `E` / `d`.",
                "提示：按 `a` 新增校验命令，或选中某条命令后按 `E` 编辑 / `d` 删除。",
            )),
        ],
    );
    (
        form,
        FormTarget::EngineeringDomain(GovernanceDomain::Verify),
    )
}

pub(super) fn build_verify_command_form(
    edit_idx: Option<usize>,
    manifest: &ProjectManifest,
    i18n: I18n,
) -> (ModalFormState, FormTarget) {
    let existing = edit_idx.and_then(|idx| manifest.engineering.verify.commands.get(idx));
    let title = if edit_idx.is_some() {
        i18n.pick("Edit Verify Command", "编辑本地校验命令")
    } else {
        i18n.pick("Add Verify Command", "新增本地校验命令")
    };
    let form = ModalFormState::new(
        title,
        vec![
            FormField::text(
                "id",
                i18n.pick("Command ID", "命令 ID"),
                existing.map_or("", |c| c.id.as_str()),
                "clippy",
                FieldValidator::ResourceId,
            ),
            FormField::text(
                "label",
                i18n.pick("Display Label", "展示名称"),
                existing.map_or("", |c| c.label.as_str()),
                "Clippy Pedantic",
                FieldValidator::Required,
            ),
            FormField::text(
                "cmd",
                i18n.pick("Shell Command", "执行命令"),
                existing.map_or("", |c| c.cmd.as_str()),
                "cargo clippy --workspace --all-targets -- -D warnings",
                FieldValidator::Required,
            ),
            FormField::toggle(
                "required",
                i18n.pick("Required Gate", "强制必过"),
                existing.is_none_or(|c| c.required),
            ),
        ],
    );
    (form, FormTarget::VerifyCommand { edit_idx })
}

pub(super) fn build_supply_chain_form(
    manifest: &ProjectManifest,
    i18n: I18n,
) -> (ModalFormState, FormTarget) {
    let sc = &manifest.engineering.supply_chain;
    let form = ModalFormState::new(
        i18n.pick("Configure [engineering.supply_chain]", "配置供应链安全基线"),
        vec![
            FormField::toggle(
                "forbid_copyleft",
                i18n.pick("Forbid Copyleft", "禁止 Copyleft 协议"),
                sc.forbid_copyleft,
            ),
            FormField::toggle(
                "stateless_github_tokens",
                i18n.pick("Stateless ghs_*", "无状态 ghs_* 脱敏"),
                sc.stateless_github_tokens,
            ),
            FormField::text(
                "publish_cooldown_days",
                i18n.pick("Cooldown (days)", "依赖冷静期 (天)"),
                sc.publish_cooldown_days.to_string(),
                "7",
                FieldValidator::Required,
            )
            .with_hint(i18n.pick(
                "Non-negative integer days (0 disables publish cooldown).",
                "非负整数天数（0 表示关闭冷静期）。",
            )),
            FormField::string_list(
                "allowed_licenses",
                i18n.pick("Allowed Licenses", "允许 SPDX 协议"),
                sc.allowed_licenses.clone(),
                "Apache-2.0",
                FieldValidator::Required,
            ),
        ],
    );
    (
        form,
        FormTarget::EngineeringDomain(GovernanceDomain::SupplyChain),
    )
}

pub(super) fn build_architecture_form(
    manifest: &ProjectManifest,
    i18n: I18n,
) -> (ModalFormState, FormTarget) {
    let arch = &manifest.engineering.architecture;
    let layer_entries: Vec<KeyValueEntry> = arch
        .layers
        .iter()
        .map(|l| KeyValueEntry {
            key: l.name.to_string(),
            value: l.glob.clone(),
            extra: Some(l.rule.clone()),
        })
        .collect();

    let form = ModalFormState::new(
        i18n.pick(
            "Configure [engineering.architecture]",
            "配置架构文档与分层边界",
        ),
        vec![
            FormField::string_list(
                "docs",
                i18n.pick("Spec Docs", "架构文档路径"),
                arch.docs.clone(),
                "docs/architecture.md",
                FieldValidator::Required,
            ),
            FormField::kv_list(
                "layers",
                i18n.pick("Boundary Layers", "架构分层边界"),
                KeyValueListState::new(
                    layer_entries,
                    "Layer ID (la-core)",
                    "Glob (crates/la-core/**)",
                    Some("Architectural Invariant Rule".to_owned()),
                    false,
                ),
            ),
        ],
    );
    (
        form,
        FormTarget::EngineeringDomain(GovernanceDomain::Architecture),
    )
}

pub(super) fn build_release_form(
    manifest: &ProjectManifest,
    i18n: I18n,
) -> (ModalFormState, FormTarget) {
    let rel = &manifest.engineering.release;
    let tool_idx = match rel.tool {
        ReleaseTool::Manual => 0,
        ReleaseTool::ReleasePlz => 1,
        ReleaseTool::Changesets => 2,
    };
    let form = ModalFormState::new(
        i18n.pick("Configure [engineering.release]", "配置版本发布策略"),
        vec![
            FormField::enum_select(
                "tool",
                i18n.pick("Release Driver", "发布工具"),
                vec!["manual".into(), "release-plz".into(), "changesets".into()],
                tool_idx,
            ),
            FormField::text(
                "tag_pattern",
                i18n.pick("Tag Pattern", "标签格式模板"),
                &rel.tag_pattern,
                "v{{ version }}",
                FieldValidator::Required,
            ),
            FormField::toggle(
                "immutable_releases",
                i18n.pick("Immutable Release", "不可变发布校验"),
                rel.immutable_releases,
            ),
        ],
    );
    (
        form,
        FormTarget::EngineeringDomain(GovernanceDomain::Release),
    )
}

pub(super) fn apply_verify_domain(form: &ModalFormState, policy: &mut EngineeringPolicy) -> String {
    policy.verify.stop_hook_mode = match form.enum_value("stop_hook_mode").unwrap_or("remind") {
        "off" => StopHookMode::Off,
        "enforce" => StopHookMode::Enforce,
        _ => StopHookMode::Remind,
    };
    "Updated [engineering.verify] stop-hook policy".to_owned()
}

pub(super) fn apply_verify_command(
    edit_idx: Option<usize>,
    form: &ModalFormState,
    policy: &mut EngineeringPolicy,
) -> Result<String, String> {
    let id = ResourceId::parse(form.text_value("id").unwrap_or_default())
        .map_err(|err| err.to_string())?;
    let label = opt_text(form, "label").ok_or_else(|| "Command label is required".to_owned())?;
    let cmd = opt_text(form, "cmd").ok_or_else(|| "Shell command is required".to_owned())?;
    let required = form.bool_value("required").unwrap_or(true);

    let duplicate = policy
        .verify
        .commands
        .iter()
        .enumerate()
        .any(|(i, c)| Some(i) != edit_idx && c.id == id);
    if duplicate {
        return Err(format!("Verify command `{id}` already exists"));
    }

    let spec = VerifyCommand {
        id: id.clone(),
        label,
        cmd,
        required,
    };
    if let Some(idx) = edit_idx {
        let Some(slot) = policy.verify.commands.get_mut(idx) else {
            return Err("Verify command no longer exists".to_owned());
        };
        *slot = spec;
        Ok(format!("Updated verify command `{id}`"))
    } else {
        policy.verify.commands.push(spec);
        Ok(format!("Added verify command `{id}`"))
    }
}

pub(super) fn apply_supply_chain(
    form: &ModalFormState,
    policy: &mut EngineeringPolicy,
) -> Result<String, String> {
    let cooldown: u16 = form
        .text_value("publish_cooldown_days")
        .unwrap_or("7")
        .parse()
        .map_err(|_| "Publish cooldown must be a non-negative integer (0..=65535)".to_owned())?;
    policy.supply_chain.forbid_copyleft = form.bool_value("forbid_copyleft").unwrap_or(true);
    policy.supply_chain.stateless_github_tokens =
        form.bool_value("stateless_github_tokens").unwrap_or(true);
    policy.supply_chain.publish_cooldown_days = cooldown;
    policy.supply_chain.allowed_licenses = form
        .string_list_values("allowed_licenses")
        .map_or_default(<[String]>::to_vec);
    Ok("Updated [engineering.supply_chain] policy".to_owned())
}

pub(super) fn apply_architecture(
    form: &ModalFormState,
    policy: &mut EngineeringPolicy,
) -> Result<String, String> {
    let mut layers = Vec::new();
    for entry in form.kv_entries("layers").unwrap_or(&[]) {
        let name = ResourceId::parse(&entry.key).map_err(|err| err.to_string())?;
        let glob = entry.value.trim().to_owned();
        if glob.is_empty() {
            return Err(format!(
                "Architecture layer `{name}` requires a glob pattern"
            ));
        }
        let rule = entry
            .extra
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| format!("Architecture layer `{name}` requires an invariant rule"))?
            .to_owned();
        layers.push(ArchitectureLayer { name, glob, rule });
    }
    policy.architecture.docs = form
        .string_list_values("docs")
        .map_or_default(<[String]>::to_vec);
    policy.architecture.layers = layers;
    Ok("Updated [engineering.architecture] policy".to_owned())
}

pub(super) fn apply_release(
    form: &ModalFormState,
    policy: &mut EngineeringPolicy,
) -> Result<String, String> {
    let tag_pattern =
        opt_text(form, "tag_pattern").ok_or_else(|| "Tag pattern is required".to_owned())?;
    policy.release.tool = match form.enum_value("tool").unwrap_or("manual") {
        "release-plz" => ReleaseTool::ReleasePlz,
        "changesets" => ReleaseTool::Changesets,
        _ => ReleaseTool::Manual,
    };
    policy.release.tag_pattern = tag_pattern;
    policy.release.immutable_releases = form.bool_value("immutable_releases").unwrap_or(true);
    Ok("Updated [engineering.release] policy".to_owned())
}
