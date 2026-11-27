//! Editing, sync, visual preferences, modal/playground, and async event handlers
//! extracted from [`crate::state::AppState`].

use la_core::manifest::GitOutputMode;

use crate::action::{Action, StudioCommand};
use crate::editor::TextEditor;
use crate::form::{FormAction, FormOutcome, ModalFormState};
use crate::media::PluginLogoPreview;
use crate::protocol::BackgroundTone;
use crate::state::catalog_modal::ConfirmDeleteState;
use crate::state::{
    AppState, FocusPane, FormTarget, InlineEditState, ModalKind, NavSection, SPLASH_DURATION_MS,
    ToastLevel, governance, resource_forms, resources,
};
use crate::theme::{ResolvedTheme, Theme};

impl AppState {
    pub(super) fn update_during_splash(&mut self, action: Action) -> Vec<StudioCommand> {
        match action {
            Action::Tick(dt) => {
                let dt_ms = u64::try_from(dt.as_millis()).unwrap_or(16);
                self.splash_elapsed_ms = self.splash_elapsed_ms.saturating_add(dt_ms);
                if self.splash_elapsed_ms >= SPLASH_DURATION_MS {
                    self.splash_active = false;
                }
            }
            Action::Quit => {
                self.should_quit = true;
                return vec![StudioCommand::Quit];
            }
            Action::LogReceived(entry) => self.logs.push(entry),
            Action::Resize(_, _) | Action::WindowFocus(_) => {}
            _ => self.splash_active = false,
        }
        Vec::new()
    }

    pub(super) fn update_async_event(&mut self, action: Action) -> Vec<StudioCommand> {
        match action {
            Action::WorkspaceLoaded(ws) => {
                self.workspace = *ws;
                if self.workspace.initialized && self.modal == Some(ModalKind::InitWizard) {
                    self.modal = None;
                }
                if self.selected_output_idx >= self.workspace.compiled.files.len() {
                    self.selected_output_idx = 0;
                }
                self.clamp_selection_indices();
                self.push_toast(
                    ToastLevel::Info,
                    self.i18n()
                        .pick("Workspace reloaded", "工作区配置已重新加载")
                        .to_owned(),
                );
                Vec::new()
            }
            Action::PluginLogoDecoded { plugin_name, image } => {
                if let Ok(preview) =
                    PluginLogoPreview::from_rgba(&image, self.capabilities.graphics)
                {
                    self.plugin_logos.insert(plugin_name, preview);
                }
                Vec::new()
            }
            Action::GitHubAuditLoaded(report) => {
                self.github_audit_loading = false;
                self.github_audit_error = None;
                self.github_audit = Some(report);
                self.push_toast(
                    ToastLevel::Success,
                    self.i18n()
                        .pick("GitHub audit updated", "GitHub 治理审计已刷新")
                        .to_owned(),
                );
                Vec::new()
            }
            Action::GitHubAuditFailed(err) => {
                self.github_audit_loading = false;
                self.github_audit_error = Some(err.clone());
                self.push_toast(ToastLevel::Error, err);
                Vec::new()
            }
            Action::SyncCompleted {
                created,
                updated,
                deleted,
            } => {
                self.modal = None;
                self.push_toast(
                    ToastLevel::Success,
                    format!("Sync complete: +{created} ~{updated} -{deleted}"),
                );
                vec![StudioCommand::ReloadWorkspace]
            }
            Action::SyncFailed(err) => {
                self.push_toast(ToastLevel::Error, err);
                Vec::new()
            }
            Action::LogReceived(entry) => {
                self.logs.push(entry);
                Vec::new()
            }
            Action::Notify { level, message } => {
                self.push_toast(level, message);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    pub(super) fn handle_activate(&mut self) -> Vec<StudioCommand> {
        if self.focus == FocusPane::Rail {
            self.focus = FocusPane::Workspace;
            return Vec::new();
        }
        if self.nav == NavSection::Engineering
            && governance::toggle_governance_row(
                self.governance_domain,
                self.governance_row,
                &mut self.workspace.manifest.engineering,
            )
        {
            self.workspace.recompute();
            self.push_toast(
                ToastLevel::Success,
                self.i18n()
                    .pick("Governance policy updated", "工程治理策略已更新")
                    .to_owned(),
            );
            return vec![StudioCommand::SaveManifest(Box::new(
                self.workspace.manifest.clone(),
            ))];
        }
        if resources::toggle_resource_enabled(
            self.nav,
            self.list_index,
            &mut self.workspace.manifest,
        ) {
            self.workspace.recompute();
            self.push_toast(
                ToastLevel::Success,
                self.i18n()
                    .pick("Resource toggled & recompiled", "资源状态已切换并重编译")
                    .to_owned(),
            );
            return vec![StudioCommand::SaveManifest(Box::new(
                self.workspace.manifest.clone(),
            ))];
        }
        Vec::new()
    }

    pub(super) fn handle_cycle_property(&mut self) -> Vec<StudioCommand> {
        if let Some(note) = resources::cycle_resource_property(
            self.nav,
            self.list_index,
            &mut self.workspace.manifest,
        ) {
            self.workspace.recompute();
            self.push_toast(ToastLevel::Success, note.to_owned());
            return vec![StudioCommand::SaveManifest(Box::new(
                self.workspace.manifest.clone(),
            ))];
        }
        Vec::new()
    }

    pub(super) fn handle_reorder_section(&mut self, up: bool) -> Vec<StudioCommand> {
        if self.nav == NavSection::Instructions
            && let Some(new_idx) = resources::reorder_instruction_section(
                self.list_index,
                self.section_index,
                up,
                &mut self.workspace.manifest,
            )
        {
            self.section_index = new_idx;
            self.workspace.recompute();
            self.push_toast(
                ToastLevel::Info,
                self.i18n()
                    .pick("Section order updated", "AGENTS.md 区块顺序已更新")
                    .to_owned(),
            );
            return vec![StudioCommand::SaveManifest(Box::new(
                self.workspace.manifest.clone(),
            ))];
        }
        Vec::new()
    }

    pub(super) fn update_editor_action(&mut self, action: Action) -> Vec<StudioCommand> {
        match action {
            Action::StartInlineEdit => {
                self.start_inline_edit();
                Vec::new()
            }
            Action::SaveInlineEdit => self.save_inline_edit(),
            Action::CancelInlineEdit => {
                self.inline_edit = None;
                Vec::new()
            }
            Action::ToggleVimMode => {
                if let Some(ref mut edit) = self.inline_edit {
                    let next = !edit.editor.vim_enabled();
                    edit.editor.set_vim_enabled(next);
                }
                Vec::new()
            }
            Action::OpenExternalEditor => self.handle_open_external_editor(),
            Action::EditorInput(ed_action) => {
                if let Some(ref mut edit) = self.inline_edit {
                    edit.editor.apply(ed_action);
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn start_inline_edit(&mut self) {
        let Some((rel_path, title)) = resources::editable_source_path(
            self.nav,
            self.list_index,
            self.section_index,
            &self.workspace.manifest,
        ) else {
            self.push_toast(
                ToastLevel::Info,
                self.i18n()
                    .pick(
                        "Select a File section, Rule, Skill, or Subagent to edit inline (or press e).",
                        "请选择文件区块、规则、技能或子代理进行内置编辑（或按 e 外部编辑）。",
                    )
                    .to_owned(),
            );
            return;
        };
        let initial = self
            .workspace
            .source_files
            .get(&rel_path)
            .cloned()
            .unwrap_or_default();
        self.inline_edit = Some(InlineEditState {
            rel_path,
            title,
            editor: TextEditor::from_text(&initial),
        });
    }

    fn save_inline_edit(&mut self) -> Vec<StudioCommand> {
        let Some(ref mut edit) = self.inline_edit else {
            return Vec::new();
        };
        let content = edit.editor.text();
        let rel_path = edit.rel_path.clone();
        edit.editor.mark_clean();
        self.workspace
            .source_files
            .insert(rel_path.clone(), content.clone());
        self.workspace.recompute();
        self.push_toast(ToastLevel::Success, format!("Saved .lazyagents/{rel_path}"));
        vec![StudioCommand::SaveSourceFile { rel_path, content }]
    }

    pub(super) fn update_modal_or_playground(&mut self, action: &Action) -> Vec<StudioCommand> {
        match *action {
            Action::PaletteInput(ch) => {
                self.palette.query.push(ch);
                self.palette.selected = 0;
                Vec::new()
            }
            Action::PaletteBackspace => {
                self.palette.query.pop();
                self.palette.selected = 0;
                Vec::new()
            }
            Action::PaletteConfirm => {
                let matches = self.palette.filtered(self.i18n());
                let chosen = matches
                    .get(self.palette.selected)
                    .map(|entry| entry.command.action.clone());
                self.modal = None;
                chosen.map_or_else(Vec::new, |act| self.update(act))
            }
            Action::WizardNext => {
                if self.wizard.step < 2 {
                    self.wizard.step = self.wizard.step.saturating_add(1);
                    Vec::new()
                } else {
                    self.update(Action::WizardConfirm)
                }
            }
            Action::WizardPrev => {
                self.wizard.step = self.wizard.step.saturating_sub(1);
                Vec::new()
            }
            Action::WizardToggleOption => {
                if self.wizard.option_index == 0 {
                    self.wizard.git_mode = match self.wizard.git_mode {
                        GitOutputMode::Commit => GitOutputMode::Ignore,
                        GitOutputMode::Ignore => GitOutputMode::Commit,
                    };
                } else {
                    self.wizard.import_existing = !self.wizard.import_existing;
                }
                Vec::new()
            }
            Action::WizardConfirm => {
                self.modal = None;
                vec![StudioCommand::InitializeWorkspace {
                    git_mode: self.wizard.git_mode,
                    import_existing: self.wizard.import_existing,
                }]
            }
            Action::PlaygroundToggleFocus => {
                self.playground.input_focused = !self.playground.input_focused;
                Vec::new()
            }
            Action::PlaygroundInput(ch) => {
                self.playground.input.push(ch);
                Vec::new()
            }
            Action::PlaygroundBackspace => {
                self.playground.input.pop();
                Vec::new()
            }
            Action::PlaygroundCycleGuard => {
                self.playground.cycle_guard();
                self.evaluate_playground_with_flash();
                Vec::new()
            }
            Action::PlaygroundNextSample => {
                self.playground.next_sample();
                self.evaluate_playground_with_flash();
                Vec::new()
            }
            Action::PlaygroundEvaluate => {
                self.evaluate_playground_with_flash();
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    pub(super) fn update_form_or_catalog(&mut self, action: Action) -> Vec<StudioCommand> {
        match action {
            Action::OpenDeleteConfirm => {
                self.open_delete_confirm();
                Vec::new()
            }
            Action::ToggleDeletePurge => {
                if let Some(ref mut confirm) = self.delete_confirm
                    && confirm.backing_path.is_some()
                {
                    confirm.purge_source_file = !confirm.purge_source_file;
                }
                Vec::new()
            }
            Action::ConfirmDelete => self.confirm_delete(),
            Action::OpenCatalogPicker => {
                self.open_catalog_picker();
                Vec::new()
            }
            Action::CatalogInput(ch) => {
                self.catalog.query.push(ch);
                self.catalog.selected = 0;
                Vec::new()
            }
            Action::CatalogBackspace => {
                self.catalog.query.pop();
                self.catalog.selected = 0;
                Vec::new()
            }
            Action::CatalogCycleCategory => {
                self.catalog.category = self.catalog.category.next();
                self.catalog.selected = 0;
                Vec::new()
            }
            Action::CatalogConfirm => self.confirm_catalog_install(),
            Action::OpenAddResource => {
                self.open_add_resource_form();
                Vec::new()
            }
            Action::OpenAddInstructionPlan => {
                let (form, target) = resource_forms::build_add_instruction_plan_form(
                    &self.workspace.manifest,
                    self.i18n(),
                );
                self.open_targeted_form(form, target);
                Vec::new()
            }
            Action::OpenEditResource => {
                self.open_edit_resource_form();
                Vec::new()
            }
            Action::OpenForm(form) => {
                self.open_form_modal(*form);
                Vec::new()
            }
            Action::FormInput(form_action) => self.handle_form_input(form_action),
            _ => Vec::new(),
        }
    }

    fn open_delete_confirm(&mut self) {
        let Some(confirm) = ConfirmDeleteState::from_selection(
            self.nav,
            self.list_index,
            self.section_index,
            &self.workspace.manifest,
        ) else {
            self.push_toast(
                ToastLevel::Info,
                self.i18n()
                    .pick(
                        "Select a removable resource item first.",
                        "请先选择可删除的资源条目。",
                    )
                    .to_owned(),
            );
            return;
        };
        self.delete_confirm = Some(confirm);
        self.modal = Some(ModalKind::ConfirmDelete);
        self.effects
            .trigger_modal_entrance(self.geometry.workspace_area, &self.theme);
    }

    fn confirm_delete(&mut self) -> Vec<StudioCommand> {
        let Some(confirm) = self.delete_confirm.take() else {
            self.modal = None;
            return Vec::new();
        };
        let label = confirm.item_label.clone();
        self.modal = None;
        let cmds = confirm.apply(&mut self.workspace);
        if !cmds.is_empty() {
            self.clamp_selection_indices();
            self.push_toast(ToastLevel::Warning, format!("Deleted `{label}`"));
        }
        cmds
    }

    fn open_catalog_picker(&mut self) {
        self.catalog.reset();
        self.modal = Some(ModalKind::CatalogPicker);
        self.effects
            .trigger_modal_entrance(self.geometry.workspace_area, &self.theme);
    }

    fn confirm_catalog_install(&mut self) -> Vec<StudioCommand> {
        let result = self.catalog.install_selected(&mut self.workspace.manifest);
        if let Some(msg) = result {
            self.workspace.recompute();
            self.modal = None;
            self.push_toast(ToastLevel::Success, msg);
            vec![StudioCommand::SaveManifest(Box::new(
                self.workspace.manifest.clone(),
            ))]
        } else {
            self.push_toast(
                ToastLevel::Info,
                self.i18n()
                    .pick("Item is already installed.", "该预设或能力已安装。")
                    .to_owned(),
            );
            Vec::new()
        }
    }

    fn open_add_resource_form(&mut self) {
        let Some((form, target)) = resource_forms::build_add_form(
            self.nav,
            self.list_index,
            &self.workspace.manifest,
            self.i18n(),
        ) else {
            self.push_toast(
                ToastLevel::Info,
                self.i18n()
                    .pick(
                        "Switch to an Antigravity resource view (4-9, Manifests, Plugins) to add items.",
                        "请切换到 Antigravity 资源视图（4-9、索引清单、插件）以新建资源。",
                    )
                    .to_owned(),
            );
            return;
        };
        self.open_targeted_form(form, target);
    }

    fn open_edit_resource_form(&mut self) {
        let Some((form, target)) = resource_forms::build_edit_form(
            self.nav,
            self.list_index,
            self.section_index,
            &self.workspace,
            self.i18n(),
        ) else {
            self.push_toast(
                ToastLevel::Info,
                self.i18n()
                    .pick(
                        "Select a resource item first (or press `a` to add one).",
                        "请先选择一个资源条目（或按 `a` 新建）。",
                    )
                    .to_owned(),
            );
            return;
        };
        self.open_targeted_form(form, target);
    }

    fn open_targeted_form(&mut self, form: ModalFormState, target: FormTarget) {
        self.active_form = Some(form);
        self.active_form_target = Some(target);
        self.modal = Some(ModalKind::FormModal);
        self.effects
            .trigger_modal_entrance(self.geometry.workspace_area, &self.theme);
    }

    fn open_form_modal(&mut self, form: ModalFormState) {
        self.active_form = Some(form);
        self.active_form_target = None;
        self.modal = Some(ModalKind::FormModal);
        self.effects
            .trigger_modal_entrance(self.geometry.workspace_area, &self.theme);
    }

    fn handle_form_input(&mut self, form_action: FormAction) -> Vec<StudioCommand> {
        let outcome = self.active_form.as_mut().map(|f| f.apply(form_action));
        match outcome {
            Some(FormOutcome::Cancel) => {
                self.active_form = None;
                self.active_form_target = None;
                self.modal = None;
                Vec::new()
            }
            Some(FormOutcome::Submit) => self.commit_active_form(),
            _ => Vec::new(),
        }
    }

    fn commit_active_form(&mut self) -> Vec<StudioCommand> {
        if let Some(target) = self.active_form_target {
            let result = self.active_form.as_ref().map_or_else(
                || Err("No active form".to_owned()),
                |form| resource_forms::apply_form_submit(target, form, &mut self.workspace),
            );
            match result {
                Ok((summary, cmds)) => {
                    self.active_form = None;
                    self.active_form_target = None;
                    self.modal = None;
                    self.clamp_selection_indices();
                    self.push_toast(ToastLevel::Success, summary);
                    cmds
                }
                Err(err_msg) => {
                    if let Some(form) = self.active_form.as_mut() {
                        form.submit_error = Some(err_msg);
                    }
                    Vec::new()
                }
            }
        } else {
            let title = self
                .active_form
                .as_ref()
                .map_or_else(String::new, |f| f.title.clone());
            self.active_form = None;
            self.modal = None;
            self.push_toast(ToastLevel::Success, format!("Validated form: {title}"));
            Vec::new()
        }
    }

    fn handle_open_external_editor(&self) -> Vec<StudioCommand> {
        let rel_path = match self.nav {
            NavSection::Rules => self
                .workspace
                .manifest
                .rules
                .get(self.list_index)
                .map(|r| format!(".lazyagents/{}", r.source)),
            NavSection::Skills => self
                .workspace
                .manifest
                .skills
                .get(self.list_index)
                .map(|s| format!(".lazyagents/{}/SKILL.md", s.source)),
            NavSection::Subagents => self
                .workspace
                .manifest
                .agents
                .get(self.list_index)
                .map(|a| format!(".lazyagents/{}", a.source)),
            _ => Some(".lazyagents/lazyagents.toml".to_owned()),
        };
        rel_path.map_or_default(|path| vec![StudioCommand::OpenExternalEditor { rel_path: path }])
    }

    pub(super) fn handle_request_sync(&mut self, force: bool) -> Vec<StudioCommand> {
        if !force && self.workspace.write_plan.has_conflicts() {
            self.modal = Some(ModalKind::ConflictGuard);
            self.effects
                .trigger_modal_entrance(self.geometry.workspace_area, &self.theme);
            return Vec::new();
        }
        vec![StudioCommand::ApplySync { force }]
    }

    pub(super) fn handle_copy_selected(&mut self) -> Vec<StudioCommand> {
        let cmd = if self.nav == NavSection::GitHubAudit {
            self.github_audit
                .as_ref()
                .and_then(|report| report.checks.get(self.list_index))
                .and_then(|check| check.remediation_cmd.clone())
        } else {
            None
        };
        if let Some(cmd) = cmd {
            self.push_toast(
                ToastLevel::Success,
                self.i18n()
                    .pick(
                        "Copied remediation command via OSC 52",
                        "已通过 OSC 52 复制修复命令",
                    )
                    .to_owned(),
            );
            return vec![StudioCommand::CopyToClipboard(cmd)];
        }
        Vec::new()
    }

    pub(super) fn cycle_theme(&mut self) -> Vec<StudioCommand> {
        self.config.theme = self.config.theme.next();
        let is_light = matches!(self.capabilities.background, BackgroundTone::Light);
        let variant = ResolvedTheme::resolve(self.config.theme, is_light);
        self.theme = Theme::build(variant, self.config.color_profile, self.config.glyphs);
        self.push_toast(
            ToastLevel::Info,
            format!("Theme: {}", variant.display_name()),
        );
        vec![StudioCommand::SavePreferences(self.current_preferences())]
    }

    pub(super) fn cycle_shader_preset(&mut self) -> Vec<StudioCommand> {
        self.config.shader_preset = self.config.shader_preset.next();
        self.effects.set_preset(self.config.shader_preset);
        self.effects
            .trigger_view_transition(self.geometry.workspace_area, &self.theme);
        self.push_toast(
            ToastLevel::Info,
            format!("Shader: {}", self.config.shader_preset),
        );
        vec![StudioCommand::SavePreferences(self.current_preferences())]
    }

    pub(super) fn cycle_motion(&mut self) -> Vec<StudioCommand> {
        self.config.motion = self.config.motion.next();
        self.effects.set_motion(self.config.motion);
        self.push_toast(
            ToastLevel::Info,
            format!("Motion: {}", self.config.motion.as_str()),
        );
        vec![StudioCommand::SavePreferences(self.current_preferences())]
    }

    pub(super) fn toggle_lang(&mut self) -> Vec<StudioCommand> {
        self.config.lang = self.config.lang.toggle();
        let msg = self
            .i18n()
            .pick("Language switched to English", "界面语言已切换为简体中文")
            .to_owned();
        self.push_toast(ToastLevel::Info, msg);
        vec![StudioCommand::SavePreferences(self.current_preferences())]
    }

    fn evaluate_playground_with_flash(&mut self) {
        if let Some(decision) = self.playground.evaluate(&self.workspace.manifest) {
            self.effects
                .trigger_verdict_flash(self.geometry.workspace_area, decision, &self.theme);
        }
    }
}
