//! Complete built-in English (`en`) and Simplified Chinese (`zh`) UI string catalog.
//!
//! Widgets MUST obtain every visible label, heading, status caption, and key hint through
//! [`I18n`] rather than hardcoding natural-language literals.

use crate::config::UiLang;

/// Bilingual UI text lookup facade parameterized by [`UiLang`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct I18n {
    lang: UiLang,
}

impl I18n {
    /// Creates an [`I18n`] catalog for `lang`.
    #[must_use]
    pub const fn new(lang: UiLang) -> Self {
        Self { lang }
    }

    /// Returns the active [`UiLang`].
    #[must_use]
    pub const fn lang(self) -> UiLang {
        self.lang
    }

    /// Selects between an English and Simplified Chinese string literal.
    #[must_use]
    pub const fn pick(self, en: &'static str, zh: &'static str) -> &'static str {
        match self.lang {
            UiLang::En => en,
            UiLang::Zh => zh,
        }
    }

    /// Studio application subtitle shown in the HUD header and splash screen.
    #[must_use]
    pub const fn studio_subtitle(self) -> &'static str {
        self.pick(
            "CYBER CONFIG STUDIO // ANTIGRAVITY GOVERNANCE",
            "赛博配置工作室 // ANTIGRAVITY 工程治理",
        )
    }

    /// Splash skip prompt.
    #[must_use]
    pub const fn splash_hint(self) -> &'static str {
        self.pick(
            "PRESS ANY KEY TO ENTER COMMAND DECK",
            "按任意键立即进入指挥舱",
        )
    }

    /// Tiny terminal fallback headline.
    #[must_use]
    pub const fn terminal_too_small_title(self) -> &'static str {
        self.pick("Terminal Too Small", "终端窗口过小")
    }

    /// Tiny terminal fallback guidance.
    #[must_use]
    pub const fn terminal_too_small_hint(self) -> &'static str {
        self.pick("Resize to >= 60x16", "请调整至至少 60x16")
    }

    /// Navigation rail group: Overview.
    #[must_use]
    pub const fn nav_group_overview(self) -> &'static str {
        self.pick("OVERVIEW", "全局总览")
    }

    /// Navigation rail group: Engineering Governance.
    #[must_use]
    pub const fn nav_group_governance(self) -> &'static str {
        self.pick("GOVERNANCE", "工程治理")
    }

    /// Navigation rail group: 8 Antigravity Assets.
    #[must_use]
    pub const fn nav_group_resources(self) -> &'static str {
        self.pick("ANTIGRAVITY", "ANTIGRAVITY 资产")
    }

    /// Navigation rail group: Output & Sync.
    #[must_use]
    pub const fn nav_group_output(self) -> &'static str {
        self.pick("DEPLOY", "编译同步")
    }

    /// Nav item: HUD Dashboard & Budget Radar.
    #[must_use]
    pub const fn nav_dashboard(self) -> &'static str {
        self.pick("HUD Dashboard", "赛博仪表盘")
    }

    /// Nav item: Engineering Policy.
    #[must_use]
    pub const fn nav_engineering(self) -> &'static str {
        self.pick("Engineering", "工程规范")
    }

    /// Nav item: Live Read-Only GitHub Audit.
    #[must_use]
    pub const fn nav_github_audit(self) -> &'static str {
        self.pick("GitHub Audit", "GitHub 审计")
    }

    /// Nav item: Dynamic `AGENTS.md` Instructions.
    #[must_use]
    pub const fn nav_instructions(self) -> &'static str {
        self.pick("AGENTS.md", "AGENTS.md 指令")
    }

    /// Nav item: Conditional Rules.
    #[must_use]
    pub const fn nav_rules(self) -> &'static str {
        self.pick("Rules", "条件规则")
    }

    /// Nav item: Skills.
    #[must_use]
    pub const fn nav_skills(self) -> &'static str {
        self.pick("Skills", "技能包")
    }

    /// Nav item: Custom Subagents.
    #[must_use]
    pub const fn nav_agents(self) -> &'static str {
        self.pick("Subagents", "子代理")
    }

    /// Nav item: Lifecycle Hooks & Playground.
    #[must_use]
    pub const fn nav_hooks(self) -> &'static str {
        self.pick("Hooks & Lab", "钩子与沙盒")
    }

    /// Nav item: MCP Servers.
    #[must_use]
    pub const fn nav_mcp(self) -> &'static str {
        self.pick("MCP Servers", "MCP 服务")
    }

    /// Nav item: Index Manifests.
    #[must_use]
    pub const fn nav_manifests(self) -> &'static str {
        self.pick("Manifests", "索引清单")
    }

    /// Nav item: Project Plugins.
    #[must_use]
    pub const fn nav_plugins(self) -> &'static str {
        self.pick("Plugins", "插件包")
    }

    /// Nav item: Diff & Sync Center.
    #[must_use]
    pub const fn nav_sync_diff(self) -> &'static str {
        self.pick("Sync & Diff", "同步与差异")
    }

    /// Inspector tab: Compiled Preview.
    #[must_use]
    pub const fn inspector_preview(self) -> &'static str {
        self.pick("Preview", "编译预览")
    }

    /// Inspector tab: Unified / Side-by-Side Diff.
    #[must_use]
    pub const fn inspector_diff(self) -> &'static str {
        self.pick("Diff", "差异对比")
    }

    /// Inspector tab: Lint Diagnostics (`LA001`–`LA017`).
    #[must_use]
    pub const fn inspector_diagnostics(self) -> &'static str {
        self.pick("Lint", "合规诊断")
    }

    /// Inspector tab: Context Budget Telemetry.
    #[must_use]
    pub const fn inspector_budget(self) -> &'static str {
        self.pick("Budget", "预算遥测")
    }

    /// Dashboard panel title: Context Budget Radar.
    #[must_use]
    pub const fn budget_radar_title(self) -> &'static str {
        self.pick("CONTEXT BUDGET RADAR", "上下文预算雷达")
    }

    /// Dashboard panel title: Compliance & Health Score.
    #[must_use]
    pub const fn health_score_title(self) -> &'static str {
        self.pick("GOVERNANCE HEALTH", "治理健康度")
    }

    /// Dashboard panel title: Scope Provenance.
    #[must_use]
    pub const fn scope_provenance_title(self) -> &'static str {
        self.pick("ACTIVE SCOPES", "生效作用域")
    }

    /// Hook Playground title.
    #[must_use]
    pub const fn hook_playground_title(self) -> &'static str {
        self.pick("HOOK POLICY PLAYGROUND", "钩子政策实时沙盒")
    }

    /// Command Palette modal title.
    #[must_use]
    pub const fn command_palette_title(self) -> &'static str {
        self.pick("COMMAND PALETTE", "命令面板")
    }

    /// Command Palette input placeholder.
    #[must_use]
    pub const fn command_palette_placeholder(self) -> &'static str {
        self.pick(
            "Type a command, view, or action...",
            "输入命令、视图或操作关键词...",
        )
    }

    /// Guided Init Wizard title.
    #[must_use]
    pub const fn init_wizard_title(self) -> &'static str {
        self.pick("FIRST-RUN STUDIO INITIALIZATION", "首次启动配置向导")
    }

    /// Conflict Guard modal title.
    #[must_use]
    pub const fn conflict_guard_title(self) -> &'static str {
        self.pick("EXTERNAL CONFLICT GUARD", "外部修改冲突守卫")
    }

    /// Keybindings cheat sheet modal title.
    #[must_use]
    pub const fn help_modal_title(self) -> &'static str {
        self.pick("COMMAND DECK KEYBINDINGS", "指挥舱快捷键总览")
    }

    /// Bottom live tracing / logs drawer title.
    #[must_use]
    pub const fn logs_drawer_title(self) -> &'static str {
        self.pick("LIVE TELEMETRY & REDACTED LOGS", "实时遥测与脱敏日志")
    }

    /// Status label: Clean / Zero Drift.
    #[must_use]
    pub const fn status_synced(self) -> &'static str {
        self.pick("SYNCED", "已同步")
    }

    /// Status label: Drift Detected.
    #[must_use]
    pub const fn status_drifted(self) -> &'static str {
        self.pick("DRIFT", "存在漂移")
    }

    /// Status label: External Conflict.
    #[must_use]
    pub const fn status_conflict(self) -> &'static str {
        self.pick("CONFLICT", "外部冲突")
    }

    /// Footer shortcut legend for main studio views.
    #[must_use]
    pub const fn footer_hints(self) -> &'static str {
        self.pick(
            "Ctrl+K Palette  Tab Pane  s Sync  e $EDITOR  t Theme  p Shader  L EN/ZH  ~ Logs  ? Help  q Quit",
            "Ctrl+K 命令面板  Tab 切窗  s 同步  e 外部编辑  t 主题  p 特效  L 中/英  ~ 日志  ? 帮助  q 退出",
        )
    }

    /// Editor mode indicator: Insert.
    #[must_use]
    pub const fn editor_mode_insert(self) -> &'static str {
        self.pick("INSERT", "插入")
    }

    /// Editor mode indicator: Normal (Vim).
    #[must_use]
    pub const fn editor_mode_normal(self) -> &'static str {
        self.pick("NORMAL", "普通")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_and_chinese_catalogs_return_non_empty_distinct_copy() {
        let en = I18n::new(UiLang::En);
        let zh = I18n::new(UiLang::Zh);

        assert_eq!(en.lang(), UiLang::En);
        assert_eq!(zh.lang(), UiLang::Zh);
        assert_ne!(en.nav_dashboard(), zh.nav_dashboard());
        assert_ne!(en.budget_radar_title(), zh.budget_radar_title());
        assert_ne!(en.footer_hints(), zh.footer_hints());
        assert_ne!(en.hook_playground_title(), "");
        assert_ne!(zh.hook_playground_title(), "");
    }
}
