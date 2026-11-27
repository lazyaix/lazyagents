//! 7-domain `[engineering]` governance inspector rows and in-TUI toggle/cycle mutations.

use la_core::PolicyDecision;
use la_core::engineering::{
    AiDisclosureMode, BranchingModel, ConventionalCommitMode, EngineeringPolicy, MergeStrategy,
    ReleaseTool, StopHookMode,
};

use crate::i18n::I18n;

/// Selectable sub-tabs covering all 7 declarative `[engineering]` governance domains.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum GovernanceDomain {
    /// `[engineering.git]` branching, worktree isolation, and guardrails.
    #[default]
    Git,
    /// `[engineering.protected_paths]` blast-radius globs and `CODEOWNERS` sync.
    ProtectedPaths,
    /// `[engineering.verify]` local verification gates and `Stop` hook mode.
    Verify,
    /// `[engineering.github]` CI workflow, PR template, Actions linter, MCP & Skills presets.
    GitHub,
    /// `[engineering.supply_chain]` license allowlist, publish cooldown, and `ghs_*` hygiene.
    SupplyChain,
    /// `[engineering.architecture]` doc references and path-scoped boundary layers.
    Architecture,
    /// `[engineering.release]` release driver, tag pattern, and immutable releases.
    Release,
}

/// One key-value row displayed inside the active [`GovernanceDomain`] panel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovernanceRow {
    /// Localized field title.
    pub label: String,
    /// Formatted current value.
    pub value: String,
    /// Whether pressing `Space` / `Enter` mutates this row interactively.
    pub interactive: bool,
}

impl GovernanceDomain {
    /// Ordered list of all 7 `[engineering]` domains.
    pub const ALL: [Self; 7] = [
        Self::Git,
        Self::ProtectedPaths,
        Self::Verify,
        Self::GitHub,
        Self::SupplyChain,
        Self::Architecture,
        Self::Release,
    ];

    /// Advances to the next governance domain tab.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Git => Self::ProtectedPaths,
            Self::ProtectedPaths => Self::Verify,
            Self::Verify => Self::GitHub,
            Self::GitHub => Self::SupplyChain,
            Self::SupplyChain => Self::Architecture,
            Self::Architecture => Self::Release,
            Self::Release => Self::Git,
        }
    }

    /// Moves to the previous governance domain tab.
    #[must_use]
    pub const fn prev(self) -> Self {
        match self {
            Self::Git => Self::Release,
            Self::ProtectedPaths => Self::Git,
            Self::Verify => Self::ProtectedPaths,
            Self::GitHub => Self::Verify,
            Self::SupplyChain => Self::GitHub,
            Self::Architecture => Self::SupplyChain,
            Self::Release => Self::Architecture,
        }
    }

    /// Returns the localized domain tab label.
    #[must_use]
    pub const fn label(self, i18n: I18n) -> &'static str {
        match self {
            Self::Git => i18n.pick("Git", "Git 规范"),
            Self::ProtectedPaths => i18n.pick("Protected", "受保护路径"),
            Self::Verify => i18n.pick("Verify", "本地校验"),
            Self::GitHub => i18n.pick("GitHub", "GitHub 同步"),
            Self::SupplyChain => i18n.pick("Supply Chain", "供应链"),
            Self::Architecture => i18n.pick("Architecture", "架构边界"),
            Self::Release => i18n.pick("Release", "发布策略"),
        }
    }
}

/// Builds the display rows for one [`GovernanceDomain`] from [`EngineeringPolicy`].
#[must_use]
pub fn governance_rows(
    domain: GovernanceDomain,
    policy: &EngineeringPolicy,
    i18n: I18n,
) -> Vec<GovernanceRow> {
    match domain {
        GovernanceDomain::Git => git_rows(policy, i18n),
        GovernanceDomain::ProtectedPaths => protected_path_rows(policy, i18n),
        GovernanceDomain::Verify => verify_rows(policy, i18n),
        GovernanceDomain::GitHub => github_rows(policy, i18n),
        GovernanceDomain::SupplyChain => supply_chain_rows(policy, i18n),
        GovernanceDomain::Architecture => architecture_rows(policy, i18n),
        GovernanceDomain::Release => release_rows(policy, i18n),
    }
}

/// Applies an interactive toggle or enum cycle to `row_idx` inside `domain`.
/// Returns `true` when `policy` was mutated.
pub(super) fn toggle_governance_row(
    domain: GovernanceDomain,
    row_idx: usize,
    policy: &mut EngineeringPolicy,
) -> bool {
    match domain {
        GovernanceDomain::Git => toggle_git_row(row_idx, policy),
        GovernanceDomain::ProtectedPaths => toggle_protected_paths_row(row_idx, policy),
        GovernanceDomain::Verify => toggle_verify_row(row_idx, policy),
        GovernanceDomain::GitHub => toggle_github_row(row_idx, policy),
        GovernanceDomain::SupplyChain => toggle_supply_chain_row(row_idx, policy),
        GovernanceDomain::Architecture => false,
        GovernanceDomain::Release => toggle_release_row(row_idx, policy),
    }
}

fn git_rows(policy: &EngineeringPolicy, i18n: I18n) -> Vec<GovernanceRow> {
    vec![
        row(
            i18n.pick("Branching Model", "分支模型"),
            format!("{:?}", policy.git.branching_model),
            true,
        ),
        row(
            i18n.pick("Merge Strategy", "合并策略"),
            format!("{:?}", policy.git.merge_strategy),
            true,
        ),
        row(
            i18n.pick("Conventional Commits", "语义化提交"),
            format!("{:?}", policy.git.conventional_commits),
            true,
        ),
        row(
            i18n.pick("Worktree Isolation", "Worktree 并发隔离"),
            bool_str(policy.git.worktree.enabled),
            true,
        ),
        row(
            i18n.pick("Stacked PRs", "堆叠 PR 工作流"),
            bool_str(policy.git.stacked_prs),
            true,
        ),
        row(
            i18n.pick("AI Disclosure Mode", "AI 辅助声明"),
            format!("{:?}", policy.git.ai_disclosure),
            true,
        ),
        row(
            i18n.pick("Block Push Default Branch", "拦截直推默认分支"),
            policy
                .git
                .guardrails
                .block_push_default_branch
                .as_wire_str()
                .to_owned(),
            true,
        ),
        row(
            i18n.pick("Block Force Push", "拦截强制推送 (-f)"),
            policy
                .git
                .guardrails
                .block_force_push
                .as_wire_str()
                .to_owned(),
            true,
        ),
        row(
            i18n.pick("Default Branch", "默认分支"),
            policy.git.default_branch.clone(),
            false,
        ),
        row(
            i18n.pick("Worktree Pattern", "Worktree 路径模板"),
            policy.git.worktree.pattern.clone(),
            false,
        ),
        row(
            i18n.pick("Branch Prefixes", "推荐分支前缀"),
            policy.git.branch_prefixes.join(", "),
            false,
        ),
    ]
}

fn protected_path_rows(policy: &EngineeringPolicy, i18n: I18n) -> Vec<GovernanceRow> {
    let mut rows = vec![
        row(
            i18n.pick("Default Hook Decision", "默认拦截策略"),
            policy
                .protected_paths
                .default_decision
                .as_wire_str()
                .to_owned(),
            true,
        ),
        row(
            i18n.pick("Sync .github/CODEOWNERS", "同步 CODEOWNERS 受管区"),
            bool_str(policy.protected_paths.codeowners_sync),
            true,
        ),
    ];
    for entry in &policy.protected_paths.entries {
        rows.push(row(
            entry.glob.clone(),
            format!(
                "{}  {}",
                entry
                    .decision
                    .unwrap_or(policy.protected_paths.default_decision)
                    .as_wire_str(),
                entry.owners.join(", ")
            ),
            false,
        ));
    }
    rows
}

fn verify_rows(policy: &EngineeringPolicy, i18n: I18n) -> Vec<GovernanceRow> {
    let mut rows = vec![row(
        i18n.pick("Stop Hook Verify Gate", "Stop 钩子校验门禁"),
        format!("{:?}", policy.verify.stop_hook_mode),
        true,
    )];
    for cmd in &policy.verify.commands {
        rows.push(row(
            format!("{} ({})", cmd.label, cmd.id),
            format!("[{}] {}", if cmd.required { "REQ" } else { "OPT" }, cmd.cmd),
            true,
        ));
    }
    rows
}

fn github_rows(policy: &EngineeringPolicy, i18n: I18n) -> Vec<GovernanceRow> {
    vec![
        row(
            i18n.pick("GitHub Governance Enabled", "启用 GitHub 工程治理"),
            bool_str(policy.github.enabled),
            true,
        ),
        row(
            i18n.pick("Generate CI Workflow", "生成 SHA 锁定 CI 工作流"),
            bool_str(policy.github.sync.ci_workflow),
            true,
        ),
        row(
            i18n.pick("Sync PR Template", "同步 PR 模板受管区"),
            bool_str(policy.github.sync.pr_template),
            true,
        ),
        row(
            i18n.pick(
                "Lint Actions Security (LA014-016)",
                "审计 Actions 安全 (LA014-016)",
            ),
            bool_str(policy.github.sync.lint_workflows),
            true,
        ),
        row(
            i18n.pick("GitHub Streamable HTTP MCP", "启用 GitHub MCP 预设"),
            bool_str(policy.github.enable_mcp_preset),
            true,
        ),
        row(
            i18n.pick("Repository Slug", "仓库标识"),
            policy
                .github
                .repo
                .clone()
                .unwrap_or_else(|| "<auto-probed>".to_owned()),
            false,
        ),
        row(
            i18n.pick("CI Install Mode", "CI 运行模式"),
            format!("{:?}", policy.github.sync.ci_install),
            false,
        ),
        row(
            i18n.pick("PR Template Mode", "PR 模板所有权"),
            format!("{:?}", policy.github.sync.pr_template_mode),
            false,
        ),
        row(
            i18n.pick("Expected Required Checks", "预期必过 CI 检查"),
            if policy.github.expected_rulesets.required_checks.is_empty() {
                "<none>".to_owned()
            } else {
                policy.github.expected_rulesets.required_checks.join(", ")
            },
            false,
        ),
    ]
}

fn supply_chain_rows(policy: &EngineeringPolicy, i18n: I18n) -> Vec<GovernanceRow> {
    vec![
        row(
            i18n.pick("Forbid Copyleft (MPL/GPL/AGPL)", "禁止 Copyleft 协议"),
            bool_str(policy.supply_chain.forbid_copyleft),
            true,
        ),
        row(
            i18n.pick("Stateless ghs_* Token Redaction", "无状态 ghs_* 令牌脱敏"),
            bool_str(policy.supply_chain.stateless_github_tokens),
            true,
        ),
        row(
            i18n.pick("Publish Cooldown (days)", "依赖发布冷静期 (天)"),
            policy.supply_chain.publish_cooldown_days.to_string(),
            true,
        ),
        row(
            i18n.pick("Allowed Licenses", "允许许可证列表"),
            policy.supply_chain.allowed_licenses.join(", "),
            false,
        ),
    ]
}

fn architecture_rows(policy: &EngineeringPolicy, i18n: I18n) -> Vec<GovernanceRow> {
    let mut rows = Vec::new();
    for doc in &policy.architecture.docs {
        rows.push(row(i18n.pick("Spec Doc", "架构文档"), doc.clone(), false));
    }
    for layer in &policy.architecture.layers {
        rows.push(row(
            format!("{} ({})", layer.name, layer.glob),
            layer.rule.clone(),
            false,
        ));
    }
    if rows.is_empty() {
        rows.push(row(
            i18n.pick("Architecture Layers", "架构分层"),
            i18n.pick(
                "None declared yet (press a / E to configure)",
                "暂未声明（按 a / E 打开配置表单）",
            )
            .to_owned(),
            false,
        ));
    }
    rows
}

fn release_rows(policy: &EngineeringPolicy, i18n: I18n) -> Vec<GovernanceRow> {
    vec![
        row(
            i18n.pick("Release Driver", "发布工具"),
            format!("{:?}", policy.release.tool),
            true,
        ),
        row(
            i18n.pick("Immutable Releases Expected", "要求 Immutable Releases"),
            bool_str(policy.release.immutable_releases),
            true,
        ),
        row(
            i18n.pick("Tag Pattern", "标签模板"),
            policy.release.tag_pattern.clone(),
            false,
        ),
    ]
}

const fn toggle_git_row(row_idx: usize, policy: &mut EngineeringPolicy) -> bool {
    match row_idx {
        0 => {
            policy.git.branching_model = match policy.git.branching_model {
                BranchingModel::TrunkBased => BranchingModel::GithubFlow,
                BranchingModel::GithubFlow => BranchingModel::Gitflow,
                BranchingModel::Gitflow => BranchingModel::TrunkBased,
            };
            true
        }
        1 => {
            policy.git.merge_strategy = match policy.git.merge_strategy {
                MergeStrategy::SquashOnly => MergeStrategy::Merge,
                MergeStrategy::Merge => MergeStrategy::Rebase,
                MergeStrategy::Rebase => MergeStrategy::SquashOnly,
            };
            true
        }
        2 => {
            policy.git.conventional_commits = match policy.git.conventional_commits {
                ConventionalCommitMode::PrTitle => ConventionalCommitMode::EveryCommit,
                ConventionalCommitMode::EveryCommit => ConventionalCommitMode::Off,
                ConventionalCommitMode::Off => ConventionalCommitMode::PrTitle,
            };
            true
        }
        3 => {
            policy.git.worktree.enabled = !policy.git.worktree.enabled;
            true
        }
        4 => {
            policy.git.stacked_prs = !policy.git.stacked_prs;
            true
        }
        5 => {
            policy.git.ai_disclosure = match policy.git.ai_disclosure {
                AiDisclosureMode::PrCheckbox => AiDisclosureMode::CoAuthoredBy,
                AiDisclosureMode::CoAuthoredBy => AiDisclosureMode::None,
                AiDisclosureMode::None => AiDisclosureMode::PrCheckbox,
            };
            true
        }
        6 => {
            policy.git.guardrails.block_push_default_branch =
                cycle_decision(policy.git.guardrails.block_push_default_branch);
            true
        }
        7 => {
            policy.git.guardrails.block_force_push =
                cycle_decision(policy.git.guardrails.block_force_push);
            true
        }
        _ => false,
    }
}

const fn toggle_protected_paths_row(row_idx: usize, policy: &mut EngineeringPolicy) -> bool {
    match row_idx {
        0 => {
            policy.protected_paths.default_decision =
                cycle_decision(policy.protected_paths.default_decision);
            true
        }
        1 => {
            policy.protected_paths.codeowners_sync = !policy.protected_paths.codeowners_sync;
            true
        }
        _ => false,
    }
}

fn toggle_verify_row(row_idx: usize, policy: &mut EngineeringPolicy) -> bool {
    if row_idx == 0 {
        policy.verify.stop_hook_mode = match policy.verify.stop_hook_mode {
            StopHookMode::Off => StopHookMode::Remind,
            StopHookMode::Remind => StopHookMode::Enforce,
            StopHookMode::Enforce => StopHookMode::Off,
        };
        true
    } else if let Some(cmd) = policy.verify.commands.get_mut(row_idx.saturating_sub(1)) {
        cmd.required = !cmd.required;
        true
    } else {
        false
    }
}

const fn toggle_github_row(row_idx: usize, policy: &mut EngineeringPolicy) -> bool {
    match row_idx {
        0 => {
            policy.github.enabled = !policy.github.enabled;
            true
        }
        1 => {
            policy.github.sync.ci_workflow = !policy.github.sync.ci_workflow;
            true
        }
        2 => {
            policy.github.sync.pr_template = !policy.github.sync.pr_template;
            true
        }
        3 => {
            policy.github.sync.lint_workflows = !policy.github.sync.lint_workflows;
            true
        }
        4 => {
            policy.github.enable_mcp_preset = !policy.github.enable_mcp_preset;
            true
        }
        _ => false,
    }
}

const fn toggle_supply_chain_row(row_idx: usize, policy: &mut EngineeringPolicy) -> bool {
    match row_idx {
        0 => {
            policy.supply_chain.forbid_copyleft = !policy.supply_chain.forbid_copyleft;
            true
        }
        1 => {
            policy.supply_chain.stateless_github_tokens =
                !policy.supply_chain.stateless_github_tokens;
            true
        }
        2 => {
            policy.supply_chain.publish_cooldown_days =
                match policy.supply_chain.publish_cooldown_days {
                    0 => 3,
                    1..=6 => 7,
                    7..=13 => 14,
                    _ => 0,
                };
            true
        }
        _ => false,
    }
}

const fn toggle_release_row(row_idx: usize, policy: &mut EngineeringPolicy) -> bool {
    match row_idx {
        0 => {
            policy.release.tool = match policy.release.tool {
                ReleaseTool::Manual => ReleaseTool::ReleasePlz,
                ReleaseTool::ReleasePlz => ReleaseTool::Changesets,
                ReleaseTool::Changesets => ReleaseTool::Manual,
            };
            true
        }
        1 => {
            policy.release.immutable_releases = !policy.release.immutable_releases;
            true
        }
        _ => false,
    }
}

const fn cycle_decision(decision: PolicyDecision) -> PolicyDecision {
    match decision {
        PolicyDecision::Allow => PolicyDecision::Ask,
        PolicyDecision::Ask => PolicyDecision::ForceAsk,
        PolicyDecision::ForceAsk => PolicyDecision::Deny,
        PolicyDecision::Deny => PolicyDecision::Allow,
    }
}

fn row(label: impl Into<String>, value: String, interactive: bool) -> GovernanceRow {
    GovernanceRow {
        label: label.into(),
        value,
        interactive,
    }
}

fn bool_str(value: bool) -> String {
    if value { "ON" } else { "OFF" }.to_owned()
}
