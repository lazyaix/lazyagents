//! Modal form builders and submit appliers for `[engineering.git]`,
//! `[engineering.protected_paths]`, and `[engineering.github]`.

use la_core::engineering::{
    AiDisclosureMode, BranchingModel, CiInstallMode, ConventionalCommitMode, EngineeringPolicy,
    GitHubScaffold, MergeStrategy, PrTemplateSyncMode, ProtectedPathEntry,
};
use la_core::{KebabName, ProjectManifest};

use crate::form::{
    FieldValidator, FormField, FormTab, KeyValueEntry, KeyValueListState, ModalFormState,
};
use crate::i18n::I18n;
use crate::state::{FormTarget, GovernanceDomain};

use super::{decision_options, decision_to_idx, opt_text, parse_decision, split_csv};

pub(super) fn build_git_form(
    manifest: &ProjectManifest,
    i18n: I18n,
) -> (ModalFormState, FormTarget) {
    let git = &manifest.engineering.git;
    let mut fields = git_workflow_fields(git, i18n);
    fields.extend(git_guardrail_fields(git, i18n));

    let form = ModalFormState::new(
        i18n.pick("Configure [engineering.git]", "配置 Git 工程规范"),
        fields,
    )
    .with_tabs(vec![
        FormTab::general(i18n.pick("Workflow", "分支与工作流")),
        FormTab::general(i18n.pick("Worktree & Guardrails", "隔离与安全护栏")),
    ]);
    (form, FormTarget::EngineeringDomain(GovernanceDomain::Git))
}

fn git_workflow_fields(git: &la_core::engineering::GitPolicy, i18n: I18n) -> Vec<FormField> {
    let branch_idx = match git.branching_model {
        BranchingModel::TrunkBased => 0,
        BranchingModel::GithubFlow => 1,
        BranchingModel::Gitflow => 2,
    };
    let merge_idx = match git.merge_strategy {
        MergeStrategy::SquashOnly => 0,
        MergeStrategy::Merge => 1,
        MergeStrategy::Rebase => 2,
    };
    let cc_idx = match git.conventional_commits {
        ConventionalCommitMode::PrTitle => 0,
        ConventionalCommitMode::EveryCommit => 1,
        ConventionalCommitMode::Off => 2,
    };
    let ai_idx = match git.ai_disclosure {
        AiDisclosureMode::PrCheckbox => 0,
        AiDisclosureMode::CoAuthoredBy => 1,
        AiDisclosureMode::None => 2,
    };
    vec![
        FormField::text(
            "default_branch",
            i18n.pick("Default Branch", "默认分支"),
            &git.default_branch,
            "main",
            FieldValidator::Required,
        ),
        FormField::enum_select(
            "branching_model",
            i18n.pick("Branching Model", "分支模型"),
            vec!["trunk-based".into(), "github-flow".into(), "gitflow".into()],
            branch_idx,
        ),
        FormField::enum_select(
            "merge_strategy",
            i18n.pick("Merge Strategy", "合并策略"),
            vec!["squash-only".into(), "merge".into(), "rebase".into()],
            merge_idx,
        ),
        FormField::enum_select(
            "conventional_commits",
            i18n.pick("Conv. Commits", "语义化提交"),
            vec!["pr-title".into(), "every-commit".into(), "off".into()],
            cc_idx,
        ),
        FormField::toggle(
            "stacked_prs",
            i18n.pick("Stacked PRs", "堆叠 PR 协议"),
            git.stacked_prs,
        ),
        FormField::enum_select(
            "ai_disclosure",
            i18n.pick("AI Disclosure", "AI 辅助声明"),
            vec!["pr-checkbox".into(), "co-authored-by".into(), "none".into()],
            ai_idx,
        ),
        FormField::toggle(
            "recommend_ssh_signing",
            i18n.pick("SSH Signing", "推荐 SSH 签名"),
            git.recommend_ssh_signing,
        ),
        FormField::toggle(
            "local_git_hooks",
            i18n.pick("Local Git Hooks", "生成本地 Git 钩子"),
            git.local_git_hooks,
        ),
        FormField::string_list(
            "branch_prefixes",
            i18n.pick("Branch Prefixes", "分支前缀"),
            git.branch_prefixes.clone(),
            "feat/",
            FieldValidator::Required,
        ),
    ]
}

fn git_guardrail_fields(git: &la_core::engineering::GitPolicy, i18n: I18n) -> Vec<FormField> {
    let opts: Vec<String> = decision_options().into_iter().map(str::to_owned).collect();
    vec![
        FormField::toggle(
            "worktree_enabled",
            i18n.pick("Worktree Enabled", "启用 Worktree"),
            git.worktree.enabled,
        )
        .on_tab(1),
        FormField::text(
            "worktree_pattern",
            i18n.pick("Worktree Pattern", "Worktree 路径模板"),
            &git.worktree.pattern,
            "../{repo}-wt-{task}",
            FieldValidator::Required,
        )
        .on_tab(1),
        FormField::enum_select(
            "block_push_default_branch",
            i18n.pick("Push Main Guard", "直推默认分支护栏"),
            opts.clone(),
            decision_to_idx(git.guardrails.block_push_default_branch),
        )
        .on_tab(1),
        FormField::enum_select(
            "block_force_push",
            i18n.pick("Force Push Guard", "强推 (-f) 护栏"),
            opts.clone(),
            decision_to_idx(git.guardrails.block_force_push),
        )
        .on_tab(1),
        FormField::enum_select(
            "block_destructive_reset",
            i18n.pick("Reset/Clean Guard", "重置/清理护栏"),
            opts.clone(),
            decision_to_idx(git.guardrails.block_destructive_reset),
        )
        .on_tab(1),
        FormField::enum_select(
            "guard_broad_staging",
            i18n.pick("Broad Add Guard", "全量暂存护栏"),
            opts,
            decision_to_idx(git.guardrails.guard_broad_staging),
        )
        .on_tab(1),
    ]
}

pub(super) fn build_protected_paths_form(
    manifest: &ProjectManifest,
    i18n: I18n,
) -> (ModalFormState, FormTarget) {
    let pp = &manifest.engineering.protected_paths;
    let opts: Vec<String> = decision_options().into_iter().map(str::to_owned).collect();
    let entries: Vec<KeyValueEntry> = pp
        .entries
        .iter()
        .map(|e| KeyValueEntry {
            key: e.glob.clone(),
            value: e
                .decision
                .map_or_else(String::new, |d| d.as_wire_str().to_owned()),
            extra: Some(e.owners.join(", ")),
        })
        .collect();

    let form = ModalFormState::new(
        i18n.pick(
            "Configure [engineering.protected_paths]",
            "配置受保护路径与 CODEOWNERS",
        ),
        vec![
            FormField::enum_select(
                "default_decision",
                i18n.pick("Default Decision", "默认拦截策略"),
                opts,
                decision_to_idx(pp.default_decision),
            ),
            FormField::toggle(
                "codeowners_sync",
                i18n.pick("Sync CODEOWNERS", "同步 CODEOWNERS"),
                pp.codeowners_sync,
            ),
            FormField::kv_list(
                "entries",
                i18n.pick("Protected Globs", "受保护路径条目"),
                KeyValueListState::new(
                    entries,
                    "Glob (.github/**)",
                    "Decision (blank=default)",
                    Some("Owners (@org/team, csv)".to_owned()),
                    false,
                ),
            ),
        ],
    );
    (
        form,
        FormTarget::EngineeringDomain(GovernanceDomain::ProtectedPaths),
    )
}

pub(super) fn build_github_form(
    manifest: &ProjectManifest,
    i18n: I18n,
) -> (ModalFormState, FormTarget) {
    let gh = &manifest.engineering.github;
    let mut fields = github_sync_fields(gh, i18n);
    fields.extend(github_ruleset_fields(gh, i18n));

    let form = ModalFormState::new(
        i18n.pick(
            "Configure [engineering.github]",
            "配置 GitHub 同步与 Rulesets",
        ),
        fields,
    )
    .with_tabs(vec![
        FormTab::general(i18n.pick("Sync & Presets", "同步与预设")),
        FormTab::general(i18n.pick("Expected Rulesets", "预期 Rulesets 审计")),
    ]);
    (
        form,
        FormTarget::EngineeringDomain(GovernanceDomain::GitHub),
    )
}

fn github_sync_fields(gh: &la_core::engineering::GitHubPolicy, i18n: I18n) -> Vec<FormField> {
    let ci_install_idx = match gh.sync.ci_install {
        CiInstallMode::ReleaseBinary => 0,
        CiInstallMode::CargoWorkspace => 1,
    };
    let pr_mode_idx = match gh.sync.pr_template_mode {
        PrTemplateSyncMode::WholeFile => 0,
        PrTemplateSyncMode::ManagedBlock => 1,
    };
    let skills: Vec<String> = gh.enable_skills.iter().map(ToString::to_string).collect();
    let scaffolds: Vec<String> = gh
        .scaffolds
        .iter()
        .map(|s| scaffold_to_str(*s).to_owned())
        .collect();

    vec![
        FormField::toggle(
            "enabled",
            i18n.pick("GitHub Enabled", "启用 GitHub 治理"),
            gh.enabled,
        ),
        FormField::text(
            "repo",
            i18n.pick("Repository Slug", "仓库标识"),
            gh.repo.as_deref().unwrap_or(""),
            "owner/repo (blank = auto)",
            FieldValidator::None,
        ),
        FormField::toggle(
            "ci_workflow",
            i18n.pick("CI Drift Workflow", "生成 CI 漂移工作流"),
            gh.sync.ci_workflow,
        ),
        FormField::enum_select(
            "ci_install",
            i18n.pick("CI Install Mode", "CI 运行模式"),
            vec!["release-binary".into(), "cargo-workspace".into()],
            ci_install_idx,
        ),
        FormField::toggle(
            "pr_template",
            i18n.pick("Sync PR Template", "同步 PR 模板"),
            gh.sync.pr_template,
        ),
        FormField::enum_select(
            "pr_template_mode",
            i18n.pick("PR Template Mode", "PR 模板所有权"),
            vec!["whole-file".into(), "managed-block".into()],
            pr_mode_idx,
        ),
        FormField::toggle(
            "lint_workflows",
            i18n.pick("Lint Workflows", "审计 Actions 安全"),
            gh.sync.lint_workflows,
        ),
        FormField::toggle(
            "enable_mcp_preset",
            i18n.pick("GitHub MCP Preset", "启用 GitHub MCP"),
            gh.enable_mcp_preset,
        ),
        FormField::string_list(
            "enable_skills",
            i18n.pick("Built-in Skills", "内置协作技能"),
            skills,
            "pr-author | ci-remediation",
            FieldValidator::KebabName,
        ),
        FormField::string_list(
            "scaffolds",
            i18n.pick("Scaffolds", "治理脚手架"),
            scaffolds,
            "pr-title | security-zizmor | dependabot",
            FieldValidator::Required,
        ),
    ]
}

fn github_ruleset_fields(gh: &la_core::engineering::GitHubPolicy, i18n: I18n) -> Vec<FormField> {
    let rs = &gh.expected_rulesets;
    vec![
        FormField::toggle(
            "protect_default_branch",
            i18n.pick("Protect Main", "保护默认分支"),
            rs.protect_default_branch,
        )
        .on_tab(1),
        FormField::toggle(
            "require_linear_history",
            i18n.pick("Linear History", "要求线性历史"),
            rs.require_linear_history,
        )
        .on_tab(1),
        FormField::toggle(
            "require_pr_before_merge",
            i18n.pick("Require PR", "合并前必须走 PR"),
            rs.require_pr_before_merge,
        )
        .on_tab(1),
        FormField::string_list(
            "required_checks",
            i18n.pick("Required Checks", "必过 CI 检查"),
            rs.required_checks.clone(),
            "Clippy",
            FieldValidator::Required,
        )
        .on_tab(1),
        FormField::text(
            "release_tag_pattern",
            i18n.pick("Tag Rule Pattern", "发布标签保护"),
            rs.release_tags.pattern.as_deref().unwrap_or(""),
            "v*",
            FieldValidator::None,
        )
        .on_tab(1),
        FormField::toggle(
            "release_immutable",
            i18n.pick("Immutable Release", "不可变发布校验"),
            rs.release_tags.immutable_releases,
        )
        .on_tab(1),
    ]
}

pub(super) fn apply_git(
    form: &ModalFormState,
    policy: &mut EngineeringPolicy,
) -> Result<String, String> {
    let default_branch =
        opt_text(form, "default_branch").ok_or_else(|| "Default branch is required".to_owned())?;
    let worktree_pattern = opt_text(form, "worktree_pattern")
        .ok_or_else(|| "Worktree pattern is required".to_owned())?;

    let git = &mut policy.git;
    git.default_branch = default_branch;
    git.branching_model = match form.enum_value("branching_model").unwrap_or("trunk-based") {
        "github-flow" => BranchingModel::GithubFlow,
        "gitflow" => BranchingModel::Gitflow,
        _ => BranchingModel::TrunkBased,
    };
    git.merge_strategy = match form.enum_value("merge_strategy").unwrap_or("squash-only") {
        "merge" => MergeStrategy::Merge,
        "rebase" => MergeStrategy::Rebase,
        _ => MergeStrategy::SquashOnly,
    };
    git.conventional_commits = match form
        .enum_value("conventional_commits")
        .unwrap_or("pr-title")
    {
        "every-commit" => ConventionalCommitMode::EveryCommit,
        "off" => ConventionalCommitMode::Off,
        _ => ConventionalCommitMode::PrTitle,
    };
    git.stacked_prs = form.bool_value("stacked_prs").unwrap_or(true);
    git.ai_disclosure = match form.enum_value("ai_disclosure").unwrap_or("pr-checkbox") {
        "co-authored-by" => AiDisclosureMode::CoAuthoredBy,
        "none" => AiDisclosureMode::None,
        _ => AiDisclosureMode::PrCheckbox,
    };
    git.recommend_ssh_signing = form.bool_value("recommend_ssh_signing").unwrap_or(true);
    git.local_git_hooks = form.bool_value("local_git_hooks").unwrap_or(false);
    git.branch_prefixes = form
        .string_list_values("branch_prefixes")
        .map_or_default(<[String]>::to_vec);
    git.worktree.enabled = form.bool_value("worktree_enabled").unwrap_or(true);
    git.worktree.pattern = worktree_pattern;
    git.guardrails.block_push_default_branch = parse_decision(
        form.enum_value("block_push_default_branch")
            .unwrap_or("deny"),
    )?;
    git.guardrails.block_force_push =
        parse_decision(form.enum_value("block_force_push").unwrap_or("deny"))?;
    git.guardrails.block_destructive_reset =
        parse_decision(form.enum_value("block_destructive_reset").unwrap_or("deny"))?;
    git.guardrails.guard_broad_staging =
        parse_decision(form.enum_value("guard_broad_staging").unwrap_or("ask"))?;

    Ok("Updated [engineering.git] policy".to_owned())
}

pub(super) fn apply_protected_paths(
    form: &ModalFormState,
    policy: &mut EngineeringPolicy,
) -> Result<String, String> {
    let default_decision =
        parse_decision(form.enum_value("default_decision").unwrap_or("force_ask"))?;
    let mut entries = Vec::new();
    for item in form.kv_entries("entries").unwrap_or(&[]) {
        let glob = item.key.trim().to_owned();
        if glob.is_empty() {
            return Err("Protected path glob cannot be empty".to_owned());
        }
        let decision = if item.value.trim().is_empty() {
            None
        } else {
            Some(parse_decision(&item.value)?)
        };
        let owners = item.extra.as_deref().map_or_default(split_csv);
        entries.push(ProtectedPathEntry {
            glob,
            owners,
            decision,
        });
    }
    policy.protected_paths.default_decision = default_decision;
    policy.protected_paths.codeowners_sync = form.bool_value("codeowners_sync").unwrap_or(false);
    policy.protected_paths.entries = entries;
    Ok("Updated [engineering.protected_paths] policy".to_owned())
}

pub(super) fn apply_github(
    form: &ModalFormState,
    policy: &mut EngineeringPolicy,
) -> Result<String, String> {
    let mut enable_skills = Vec::new();
    for raw in form.string_list_values("enable_skills").unwrap_or(&[]) {
        enable_skills.push(KebabName::parse(raw).map_err(|err| err.to_string())?);
    }
    let mut scaffolds = Vec::new();
    for raw in form.string_list_values("scaffolds").unwrap_or(&[]) {
        scaffolds.push(parse_scaffold(raw)?);
    }

    let gh = &mut policy.github;
    gh.enabled = form.bool_value("enabled").unwrap_or(false);
    gh.repo = opt_text(form, "repo");
    gh.sync.ci_workflow = form.bool_value("ci_workflow").unwrap_or(false);
    gh.sync.ci_install = match form.enum_value("ci_install").unwrap_or("release-binary") {
        "cargo-workspace" => CiInstallMode::CargoWorkspace,
        _ => CiInstallMode::ReleaseBinary,
    };
    gh.sync.pr_template = form.bool_value("pr_template").unwrap_or(false);
    gh.sync.pr_template_mode = match form.enum_value("pr_template_mode").unwrap_or("whole-file") {
        "managed-block" => PrTemplateSyncMode::ManagedBlock,
        _ => PrTemplateSyncMode::WholeFile,
    };
    gh.sync.lint_workflows = form.bool_value("lint_workflows").unwrap_or(false);
    gh.enable_mcp_preset = form.bool_value("enable_mcp_preset").unwrap_or(false);
    gh.enable_skills = enable_skills;
    gh.scaffolds = scaffolds;

    let rs = &mut gh.expected_rulesets;
    rs.protect_default_branch = form.bool_value("protect_default_branch").unwrap_or(true);
    rs.require_linear_history = form.bool_value("require_linear_history").unwrap_or(true);
    rs.require_pr_before_merge = form.bool_value("require_pr_before_merge").unwrap_or(true);
    rs.required_checks = form
        .string_list_values("required_checks")
        .map_or_default(<[String]>::to_vec);
    rs.release_tags.pattern = opt_text(form, "release_tag_pattern");
    rs.release_tags.immutable_releases = form.bool_value("release_immutable").unwrap_or(false);

    Ok("Updated [engineering.github] policy".to_owned())
}

const fn scaffold_to_str(scaffold: GitHubScaffold) -> &'static str {
    match scaffold {
        GitHubScaffold::PrTitle => "pr-title",
        GitHubScaffold::SecurityZizmor => "security-zizmor",
        GitHubScaffold::Dependabot => "dependabot",
        GitHubScaffold::IssueTemplates => "issue-templates",
        GitHubScaffold::Contributing => "contributing",
        GitHubScaffold::SecurityPolicy => "security-policy",
    }
}

fn parse_scaffold(raw: &str) -> Result<GitHubScaffold, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "pr-title" => Ok(GitHubScaffold::PrTitle),
        "security-zizmor" => Ok(GitHubScaffold::SecurityZizmor),
        "dependabot" => Ok(GitHubScaffold::Dependabot),
        "issue-templates" => Ok(GitHubScaffold::IssueTemplates),
        "contributing" => Ok(GitHubScaffold::Contributing),
        "security-policy" => Ok(GitHubScaffold::SecurityPolicy),
        other => Err(format!(
            "Unknown scaffold `{other}` (allowed: pr-title, security-zizmor, dependabot, issue-templates, contributing, security-policy)"
        )),
    }
}
