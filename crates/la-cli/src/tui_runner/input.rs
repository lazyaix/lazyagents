//! Context-aware translation from `crossterm::event::Event` (dual-track Vim + Universal keys,
//! Kitty `Shift+Enter`, Bracketed Paste, Mouse click/scroll, Focus, Resize) into [`la_tui::Action`].

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind};
use la_tui::{Action, AppState, EditorAction, EditorMode, FormAction, ModalKind, NavSection};

/// Translates a terminal [`Event`] into a semantic [`Action`] respecting active modals,
/// native inline editing, Hook Playground typing, and Command Deck navigation.
#[must_use]
pub(crate) fn translate_event(event: Event, state: &AppState) -> Option<Action> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => translate_key(key, state),
        Event::Paste(text) => translate_paste(text, state),
        Event::Mouse(mouse) => match mouse.kind {
            MouseEventKind::Down(crossterm::event::MouseButton::Left) => Some(Action::MouseClick {
                col: mouse.column,
                row: mouse.row,
            }),
            MouseEventKind::ScrollUp => Some(Action::MouseScroll {
                col: mouse.column,
                row: mouse.row,
                up: true,
            }),
            MouseEventKind::ScrollDown => Some(Action::MouseScroll {
                col: mouse.column,
                row: mouse.row,
                up: false,
            }),
            _ => None,
        },
        Event::FocusGained => Some(Action::WindowFocus(true)),
        Event::FocusLost => Some(Action::WindowFocus(false)),
        Event::Resize(w, h) => Some(Action::Resize(w, h)),
        Event::Key(_) => None,
    }
}

fn translate_paste(text: String, state: &AppState) -> Option<Action> {
    if state.modal == Some(ModalKind::FormModal) {
        return Some(Action::FormInput(FormAction::InsertStr(text)));
    }
    if state.inline_edit.is_some() {
        return Some(Action::EditorInput(EditorAction::InsertStr(text)));
    }
    None
}

fn translate_key(key: KeyEvent, state: &AppState) -> Option<Action> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Action::Quit);
    }

    if state.splash_active {
        return match key.code {
            KeyCode::Char('q') => Some(Action::Quit),
            _ => Some(Action::SkipSplash),
        };
    }

    if let Some(modal) = state.modal {
        return translate_modal_key(key, modal, state);
    }

    if let Some(edit) = &state.inline_edit {
        return translate_inline_editor_key(key, edit.editor.mode(), edit.editor.vim_enabled());
    }

    if state.nav == NavSection::Hooks && state.playground.input_focused {
        return translate_playground_input_key(key);
    }

    translate_deck_key(key, state)
}

fn translate_modal_key(key: KeyEvent, modal: ModalKind, state: &AppState) -> Option<Action> {
    match modal {
        ModalKind::CommandPalette => match key.code {
            KeyCode::Esc => Some(Action::CloseModal),
            KeyCode::Enter => Some(Action::PaletteConfirm),
            KeyCode::Up => Some(Action::MoveUp),
            KeyCode::Down => Some(Action::MoveDown),
            KeyCode::Backspace => Some(Action::PaletteBackspace),
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Action::PaletteInput(ch))
            }
            _ => None,
        },
        ModalKind::InitWizard => match key.code {
            KeyCode::Esc => Some(Action::CloseModal),
            KeyCode::Enter | KeyCode::Right => Some(Action::WizardNext),
            KeyCode::Left | KeyCode::Backspace => Some(Action::WizardPrev),
            KeyCode::Up | KeyCode::Char('k') => Some(Action::MoveUp),
            KeyCode::Down | KeyCode::Char('j') => Some(Action::MoveDown),
            KeyCode::Char(' ') => Some(Action::WizardToggleOption),
            _ => None,
        },
        ModalKind::ConflictGuard => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => Some(Action::CloseModal),
            KeyCode::Char('F' | 'f') => Some(Action::RequestSync { force: true }),
            _ => None,
        },
        ModalKind::Help => match key.code {
            KeyCode::Esc | KeyCode::Char('?' | 'q') => Some(Action::CloseModal),
            _ => None,
        },
        ModalKind::ConfirmDelete => match key.code {
            KeyCode::Esc | KeyCode::Char('n' | 'q') => Some(Action::CloseModal),
            KeyCode::Char(' ') => Some(Action::ToggleDeletePurge),
            KeyCode::Enter | KeyCode::Char('y' | 'Y') => Some(Action::ConfirmDelete),
            _ => None,
        },
        ModalKind::CatalogPicker => match key.code {
            KeyCode::Esc => Some(Action::CloseModal),
            KeyCode::Enter => Some(Action::CatalogConfirm),
            KeyCode::Tab => Some(Action::CatalogCycleCategory),
            KeyCode::Up => Some(Action::MoveUp),
            KeyCode::Down => Some(Action::MoveDown),
            KeyCode::Backspace => Some(Action::CatalogBackspace),
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Action::CatalogInput(ch))
            }
            _ => None,
        },
        ModalKind::FormModal => translate_form_modal_key(key, state),
    }
}

fn translate_form_modal_key(key: KeyEvent, state: &AppState) -> Option<Action> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
        return Some(Action::FormInput(FormAction::Submit));
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        return match key.code {
            KeyCode::Right => Some(Action::FormInput(FormAction::NextTab)),
            KeyCode::Left => Some(Action::FormInput(FormAction::PrevTab)),
            KeyCode::Up => Some(Action::FormInput(FormAction::ReorderItem { up: true })),
            KeyCode::Down => Some(Action::FormInput(FormAction::ReorderItem { up: false })),
            _ => None,
        };
    }

    let capturing_text = state
        .active_form
        .as_ref()
        .is_some_and(la_tui::ModalFormState::is_text_capturing);

    match key.code {
        KeyCode::Esc => Some(Action::FormInput(FormAction::Cancel)),
        KeyCode::Tab | KeyCode::Down => Some(Action::FormInput(FormAction::NextField)),
        KeyCode::BackTab | KeyCode::Up => Some(Action::FormInput(FormAction::PrevField)),
        KeyCode::Enter => Some(Action::FormInput(FormAction::Activate)),
        KeyCode::Left => Some(Action::FormInput(FormAction::MoveLeft)),
        KeyCode::Right => Some(Action::FormInput(FormAction::MoveRight)),
        KeyCode::Home => Some(Action::FormInput(FormAction::MoveHome)),
        KeyCode::End => Some(Action::FormInput(FormAction::MoveEnd)),
        KeyCode::Backspace => Some(Action::FormInput(FormAction::Backspace)),
        KeyCode::Delete => Some(Action::FormInput(FormAction::Delete)),
        KeyCode::Char(' ') if !capturing_text => Some(Action::FormInput(FormAction::Activate)),
        KeyCode::Char('a') if !capturing_text => Some(Action::FormInput(FormAction::AddItem)),
        KeyCode::Char('d') if !capturing_text => Some(Action::FormInput(FormAction::RemoveItem)),
        KeyCode::Char('K') if !capturing_text => {
            Some(Action::FormInput(FormAction::ReorderItem { up: true }))
        }
        KeyCode::Char('J') if !capturing_text => {
            Some(Action::FormInput(FormAction::ReorderItem { up: false }))
        }
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::FormInput(FormAction::InsertChar(ch)))
        }
        _ => None,
    }
}

fn translate_inline_editor_key(
    key: KeyEvent,
    mode: EditorMode,
    vim_enabled: bool,
) -> Option<Action> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('s') => Some(Action::SaveInlineEdit),
            KeyCode::Char('v') => Some(Action::ToggleVimMode),
            KeyCode::Char('z') => Some(Action::EditorInput(EditorAction::Undo)),
            KeyCode::Char('y') => Some(Action::EditorInput(EditorAction::Redo)),
            KeyCode::Char('j') => Some(Action::EditorInput(EditorAction::Newline)),
            _ => None,
        };
    }

    if vim_enabled && mode == EditorMode::Normal {
        return match key.code {
            KeyCode::Esc => Some(Action::CancelInlineEdit),
            KeyCode::Char('i') => Some(Action::EditorInput(EditorAction::SetMode(
                EditorMode::Insert,
            ))),
            KeyCode::Char('h') | KeyCode::Left => Some(Action::EditorInput(EditorAction::MoveLeft)),
            KeyCode::Char('l') | KeyCode::Right => {
                Some(Action::EditorInput(EditorAction::MoveRight))
            }
            KeyCode::Char('k') | KeyCode::Up => Some(Action::EditorInput(EditorAction::MoveUp)),
            KeyCode::Char('j') | KeyCode::Down => Some(Action::EditorInput(EditorAction::MoveDown)),
            KeyCode::Char('0') => Some(Action::EditorInput(EditorAction::MoveHome)),
            KeyCode::Char('$') => Some(Action::EditorInput(EditorAction::MoveEnd)),
            KeyCode::Char('x') => Some(Action::EditorInput(EditorAction::Delete)),
            KeyCode::Char('d') => Some(Action::EditorInput(EditorAction::DeleteLine)),
            KeyCode::Char('u') => Some(Action::EditorInput(EditorAction::Undo)),
            _ => None,
        };
    }

    match key.code {
        KeyCode::Esc => {
            if vim_enabled {
                Some(Action::EditorInput(EditorAction::SetMode(
                    EditorMode::Normal,
                )))
            } else {
                Some(Action::CancelInlineEdit)
            }
        }
        KeyCode::Enter => Some(Action::EditorInput(EditorAction::Newline)),
        KeyCode::Backspace => Some(Action::EditorInput(EditorAction::Backspace)),
        KeyCode::Delete => Some(Action::EditorInput(EditorAction::Delete)),
        KeyCode::Left => Some(Action::EditorInput(EditorAction::MoveLeft)),
        KeyCode::Right => Some(Action::EditorInput(EditorAction::MoveRight)),
        KeyCode::Up => Some(Action::EditorInput(EditorAction::MoveUp)),
        KeyCode::Down => Some(Action::EditorInput(EditorAction::MoveDown)),
        KeyCode::Home => Some(Action::EditorInput(EditorAction::MoveHome)),
        KeyCode::End => Some(Action::EditorInput(EditorAction::MoveEnd)),
        KeyCode::Tab => Some(Action::EditorInput(EditorAction::InsertStr(
            "  ".to_owned(),
        ))),
        KeyCode::Char(ch) => Some(Action::EditorInput(EditorAction::InsertChar(ch))),
        _ => None,
    }
}

const fn translate_playground_input_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Esc => Some(Action::PlaygroundToggleFocus),
        KeyCode::Enter => Some(Action::PlaygroundEvaluate),
        KeyCode::Backspace => Some(Action::PlaygroundBackspace),
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::PlaygroundInput(ch))
        }
        _ => None,
    }
}

fn translate_deck_key(key: KeyEvent, state: &AppState) -> Option<Action> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('k') {
        return Some(Action::OpenModal(ModalKind::CommandPalette));
    }

    match key.code {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char('?') => Some(Action::OpenModal(ModalKind::Help)),
        KeyCode::Tab => Some(Action::CycleFocusPane),
        KeyCode::Char('[') => Some(Action::ToggleRail),
        KeyCode::Char(']') => Some(Action::ToggleInspector),
        KeyCode::Char('~') => Some(Action::ToggleLogsDrawer),
        KeyCode::Char('t') => Some(Action::CycleTheme),
        KeyCode::Char('p') => Some(Action::CycleShaderPreset),
        KeyCode::Char('m') => Some(Action::CycleMotion),
        KeyCode::Char('L') => Some(Action::ToggleLang),
        KeyCode::Char('v') => Some(Action::ToggleDiffMode),
        KeyCode::Char('s') => Some(Action::RequestSync { force: false }),
        KeyCode::Char('R') => Some(Action::ReloadWorkspace),
        KeyCode::Char('r') => Some(Action::TriggerGitHubAudit),
        KeyCode::Char('y') => Some(Action::CopySelected),
        KeyCode::Char('e') => Some(Action::OpenExternalEditor),
        KeyCode::Char('i') => Some(Action::StartInlineEdit),
        KeyCode::Char('a') => Some(Action::OpenAddResource),
        KeyCode::Char('A') if state.nav == NavSection::Instructions => {
            Some(Action::OpenAddInstructionPlan)
        }
        KeyCode::Char('E') => Some(Action::OpenEditResource),
        KeyCode::Char('d') => Some(Action::OpenDeleteConfirm),
        KeyCode::Char('C') => Some(Action::OpenCatalogPicker),
        KeyCode::Char('c') => Some(Action::CycleItemProperty),
        KeyCode::Char('J') => Some(Action::ReorderItem { up: false }),
        KeyCode::Char('K') => Some(Action::ReorderItem { up: true }),
        KeyCode::Char('j') | KeyCode::Down => Some(Action::MoveDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::MoveUp),
        KeyCode::Char('h') | KeyCode::Left => Some(Action::MoveLeft),
        KeyCode::Char('l') | KeyCode::Right => Some(Action::MoveRight),
        KeyCode::Char('G') => Some(Action::JumpBottom),
        KeyCode::Char('/') if state.nav == NavSection::Hooks => Some(Action::PlaygroundToggleFocus),
        KeyCode::Char('g') if state.nav == NavSection::Hooks => Some(Action::PlaygroundCycleGuard),
        KeyCode::Char('n') if state.nav == NavSection::Hooks => Some(Action::PlaygroundNextSample),
        KeyCode::Char('g') => Some(Action::JumpTop),
        KeyCode::Enter if state.nav == NavSection::Hooks => Some(Action::PlaygroundEvaluate),
        KeyCode::Char(' ') | KeyCode::Enter => Some(Action::ActivateSelected),
        KeyCode::Char('1') => Some(Action::SelectNav(NavSection::Dashboard)),
        KeyCode::Char('2') => Some(Action::SelectNav(NavSection::Engineering)),
        KeyCode::Char('3') => Some(Action::SelectNav(NavSection::GitHubAudit)),
        KeyCode::Char('4') => Some(Action::SelectNav(NavSection::Instructions)),
        KeyCode::Char('5') => Some(Action::SelectNav(NavSection::Rules)),
        KeyCode::Char('6') => Some(Action::SelectNav(NavSection::Skills)),
        KeyCode::Char('7') => Some(Action::SelectNav(NavSection::Subagents)),
        KeyCode::Char('8') => Some(Action::SelectNav(NavSection::Hooks)),
        KeyCode::Char('9') => Some(Action::SelectNav(NavSection::McpServers)),
        KeyCode::Char('0') => Some(Action::SelectNav(NavSection::SyncDiff)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use la_core::{ProjectFacts, ProjectManifest};
    use la_tui::{TerminalCapabilities, TuiConfig, WorkspaceSnapshot};

    use super::*;

    fn deck_state(nav: NavSection) -> AppState {
        let ws = WorkspaceSnapshot::from_manifest(
            "/repo",
            true,
            ProjectManifest::default(),
            ProjectFacts::default(),
            BTreeMap::new(),
            BTreeMap::new(),
            None,
        );
        let cfg = TuiConfig {
            show_splash: false,
            ..TuiConfig::default()
        };
        let mut state = AppState::new(cfg, TerminalCapabilities::default(), ws);
        state.nav = nav;
        state
    }

    #[test]
    fn deck_shortcuts_route_add_edit_plan_delete_and_catalog_actions() {
        let inst_state = deck_state(NavSection::Instructions);
        assert_eq!(
            translate_event(
                Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)),
                &inst_state,
            ),
            Some(Action::OpenAddResource)
        );
        assert_eq!(
            translate_event(
                Event::Key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT)),
                &inst_state,
            ),
            Some(Action::OpenAddInstructionPlan)
        );
        assert_eq!(
            translate_event(
                Event::Key(KeyEvent::new(KeyCode::Char('E'), KeyModifiers::SHIFT)),
                &inst_state,
            ),
            Some(Action::OpenEditResource)
        );

        let rules_state = deck_state(NavSection::Rules);
        assert_eq!(
            translate_event(
                Event::Key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT)),
                &rules_state,
            ),
            None
        );
        assert_eq!(
            translate_event(
                Event::Key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE)),
                &rules_state,
            ),
            Some(Action::OpenDeleteConfirm)
        );
        assert_eq!(
            translate_event(
                Event::Key(KeyEvent::new(KeyCode::Char('C'), KeyModifiers::SHIFT)),
                &rules_state,
            ),
            Some(Action::OpenCatalogPicker)
        );
    }
}
