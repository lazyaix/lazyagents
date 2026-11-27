//! Pure-Rust shadowed modal form engine (`ModalFormState`, `FormField`, `FormTab`).
//!
//! Provides structured editing, real-time validation, and forward-compatible target tabs
//! for PR 3 (8 Antigravity resources), PR 4 (7 Engineering domains), and M4-A targets.

mod fields;

use la_core::TargetKind;

pub use self::fields::{
    FieldKind, FieldValidator, KeyValueEntry, KeyValueListState, StringListState, TextInputState,
};

/// Semantic keyboard/paste action routed into an active [`ModalFormState`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormAction {
    /// Focus next field within the active tab.
    NextField,
    /// Focus previous field within the active tab.
    PrevField,
    /// Switch to the next form tab (e.g., `General` → `Antigravity` / future M4 targets).
    NextTab,
    /// Switch to the previous form tab.
    PrevTab,
    /// Insert a typed character into the active text input or list draft.
    InsertChar(char),
    /// Insert a bracketed-paste string into the active text input or list draft.
    InsertStr(String),
    /// Delete grapheme cluster before cursor.
    Backspace,
    /// Delete grapheme cluster at cursor (or remove selected list item when not typing).
    Delete,
    /// Move cursor left, cycle enum backward, or select previous list item.
    MoveLeft,
    /// Move cursor right, cycle enum forward, or select next list item.
    MoveRight,
    /// Move cursor to start of active text input.
    MoveHome,
    /// Move cursor to end of active text input.
    MoveEnd,
    /// Toggle boolean, cycle enum, start/commit list draft, or advance field.
    Activate,
    /// Explicitly start adding an item in a `StringList` or `KeyValueList`.
    AddItem,
    /// Remove the highlighted item from a `StringList` or `KeyValueList`.
    RemoveItem,
    /// Reorder the highlighted item in a `StringList` (`up = true` moves earlier).
    ReorderItem {
        /// Move toward index `0` when `true`.
        up: bool,
    },
    /// Validate all fields and submit the modal form (`Ctrl+S`).
    Submit,
    /// Cancel active inline draft (if any), otherwise close the modal form (`Esc`).
    Cancel,
}

/// Result returned by [`ModalFormState::apply`] instructing `AppState` how to proceed.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FormOutcome {
    /// Keep the modal open (state mutated or validation error shown).
    Continue,
    /// All fields validated cleanly; caller may persist changes and close modal.
    Submit,
    /// User dismissed the modal without saving.
    Cancel,
}

/// One tab inside a [`ModalFormState`], supporting optional multi-target badges for M4-A.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormTab {
    /// Tab display title (e.g., `"General"`, `"Antigravity"`, `"Claude Code"`).
    pub label: String,
    /// Optional target badge associated with target-specific extension tabs.
    pub target_badge: Option<TargetKind>,
}

/// One labeled structured field inside a [`ModalFormState`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormField {
    /// Stable programmatic field key (e.g., `"id"`, `"globs"`, `"url"`, `"env"`).
    pub key: String,
    /// Localized user-facing field label.
    pub label: String,
    /// Optional concise helper hint rendered beneath or beside the field.
    pub hint: Option<String>,
    /// Zero-based tab index owning this field.
    pub tab: usize,
    /// Atomic interactive control state.
    pub kind: FieldKind,
    /// Current live validation error (if any).
    pub error: Option<String>,
}

/// Complete state of an open shadowed structured form modal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModalFormState {
    /// Modal header title (e.g., `"Add Conditional Rule"`).
    pub title: String,
    /// Optional subtitle or resource provenance badge.
    pub subtitle: Option<String>,
    /// Ordered tabs (defaults to a single `"General"` tab).
    pub tabs: Vec<FormTab>,
    /// Currently selected tab index.
    pub active_tab: usize,
    /// Ordered fields across all tabs.
    pub fields: Vec<FormField>,
    /// Index into `self.fields` of the currently focused field.
    pub focused_field: usize,
    /// Tracks whether any field has been edited since opening.
    pub dirty: bool,
    /// Summary validation error banner displayed at the bottom when `Submit` fails.
    pub submit_error: Option<String>,
}

impl FormTab {
    /// Creates a plain tab with no target badge.
    #[must_use]
    pub fn general(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            target_badge: None,
        }
    }

    /// Creates a target-specific extension tab carrying a [`TargetKind`] badge.
    #[must_use]
    pub fn for_target(label: impl Into<String>, target: TargetKind) -> Self {
        Self {
            label: label.into(),
            target_badge: Some(target),
        }
    }
}

impl FormField {
    /// Creates a single-line text input field on `tab = 0`.
    #[must_use]
    pub fn text(
        key: impl Into<String>,
        label: impl Into<String>,
        initial: impl Into<String>,
        placeholder: impl Into<String>,
        validator: FieldValidator,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            hint: None,
            tab: 0,
            kind: FieldKind::Text(TextInputState::new(initial, placeholder, validator)),
            error: None,
        }
    }

    /// Creates a boolean toggle field on `tab = 0`.
    #[must_use]
    pub fn toggle(key: impl Into<String>, label: impl Into<String>, initial: bool) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            hint: None,
            tab: 0,
            kind: FieldKind::Toggle(initial),
            error: None,
        }
    }

    /// Creates a cyclic enum selector field on `tab = 0`.
    #[must_use]
    pub fn enum_select(
        key: impl Into<String>,
        label: impl Into<String>,
        options: Vec<String>,
        selected: usize,
    ) -> Self {
        let clamped = if options.is_empty() {
            0
        } else {
            selected.min(options.len().saturating_sub(1))
        };
        Self {
            key: key.into(),
            label: label.into(),
            hint: None,
            tab: 0,
            kind: FieldKind::Enum {
                selected: clamped,
                options,
            },
            error: None,
        }
    }

    /// Creates an ordered string list field on `tab = 0`.
    #[must_use]
    pub fn string_list(
        key: impl Into<String>,
        label: impl Into<String>,
        items: Vec<String>,
        placeholder: impl Into<String>,
        validator: FieldValidator,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            hint: None,
            tab: 0,
            kind: FieldKind::StringList(StringListState::new(items, placeholder, validator)),
            error: None,
        }
    }

    /// Creates a structured key/value (or 3-column) list field on `tab = 0`.
    #[must_use]
    pub fn kv_list(
        key: impl Into<String>,
        label: impl Into<String>,
        state: KeyValueListState,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            hint: None,
            tab: 0,
            kind: FieldKind::KeyValueList(state),
            error: None,
        }
    }

    /// Attaches a helper hint to this field.
    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Assigns this field to a specific zero-based tab index.
    #[must_use]
    pub const fn on_tab(mut self, tab: usize) -> Self {
        self.tab = tab;
        self
    }

    /// Validates this field and stores any error message in `self.error`.
    pub fn validate(&mut self) -> bool {
        self.error = match &self.kind {
            FieldKind::Text(input) => input.validate(),
            FieldKind::Toggle(_) | FieldKind::Enum { .. } => None,
            FieldKind::StringList(list) => list
                .items
                .iter()
                .find_map(|item| list.draft.validator.validate(item)),
            FieldKind::KeyValueList(kv) => kv.validate_entries(),
        };
        self.error.is_none()
    }
}

impl ModalFormState {
    /// Creates a modal form with a single `"General"` tab and `fields`.
    #[must_use]
    pub fn new(title: impl Into<String>, fields: Vec<FormField>) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            tabs: vec![FormTab::general("General")],
            active_tab: 0,
            fields,
            focused_field: 0,
            dirty: false,
            submit_error: None,
        }
    }

    /// Attaches an optional subtitle to the modal header.
    #[must_use]
    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Replaces the form's tab definitions (for multi-tab / target-extension forms).
    #[must_use]
    pub fn with_tabs(mut self, tabs: Vec<FormTab>) -> Self {
        if !tabs.is_empty() {
            self.tabs = tabs;
            self.active_tab = 0;
            self.focus_first_field_in_tab();
        }
        self
    }

    /// Returns field indices belonging to `self.active_tab`.
    #[must_use]
    pub fn visible_field_indices(&self) -> Vec<usize> {
        self.fields
            .iter()
            .enumerate()
            .filter_map(|(idx, f)| (f.tab == self.active_tab).then_some(idx))
            .collect()
    }

    /// Returns `true` when the currently focused field actively captures raw character typing.
    #[must_use]
    pub fn is_text_capturing(&self) -> bool {
        self.fields
            .get(self.focused_field)
            .is_some_and(|field| match &field.kind {
                FieldKind::Text(_) => true,
                FieldKind::StringList(list) => list.editing_draft,
                FieldKind::KeyValueList(kv) => kv.editing_col.is_some(),
                FieldKind::Toggle(_) | FieldKind::Enum { .. } => false,
            })
    }

    /// Validates all fields across every tab, focusing the first invalid field on failure.
    pub fn validate_all(&mut self) -> bool {
        let mut first_bad: Option<(usize, usize, String)> = None;
        for (idx, field) in self.fields.iter_mut().enumerate() {
            if !field.validate() && first_bad.is_none() {
                first_bad = Some((
                    field.tab,
                    idx,
                    field
                        .error
                        .clone()
                        .unwrap_or_else(|| "Validation failed".to_owned()),
                ));
            }
        }
        if let Some((tab, idx, msg)) = first_bad {
            self.active_tab = tab;
            self.focused_field = idx;
            self.submit_error = Some(msg);
            false
        } else {
            self.submit_error = None;
            true
        }
    }

    /// Applies one [`FormAction`] and returns whether to continue, submit, or cancel.
    pub fn apply(&mut self, action: FormAction) -> FormOutcome {
        match action {
            FormAction::Submit => {
                if self.validate_all() {
                    FormOutcome::Submit
                } else {
                    FormOutcome::Continue
                }
            }
            FormAction::Cancel => {
                if self.cancel_active_draft() {
                    FormOutcome::Continue
                } else {
                    FormOutcome::Cancel
                }
            }
            FormAction::NextField => {
                self.step_focus(true);
                FormOutcome::Continue
            }
            FormAction::PrevField => {
                self.step_focus(false);
                FormOutcome::Continue
            }
            FormAction::NextTab => {
                self.step_tab(true);
                FormOutcome::Continue
            }
            FormAction::PrevTab => {
                self.step_tab(false);
                FormOutcome::Continue
            }
            other => {
                self.apply_to_focused_field(other);
                FormOutcome::Continue
            }
        }
    }

    /// Reads the trimmed string value of a `Text` field by `key`.
    #[must_use]
    pub fn text_value(&self, key: &str) -> Option<&str> {
        self.fields.iter().find(|f| f.key == key).and_then(|f| {
            if let FieldKind::Text(t) = &f.kind {
                Some(t.value.trim())
            } else {
                None
            }
        })
    }

    /// Reads the boolean value of a `Toggle` field by `key`.
    #[must_use]
    pub fn bool_value(&self, key: &str) -> Option<bool> {
        self.fields.iter().find(|f| f.key == key).and_then(|f| {
            if let FieldKind::Toggle(v) = f.kind {
                Some(v)
            } else {
                None
            }
        })
    }

    /// Reads the selected option label of an `Enum` field by `key`.
    #[must_use]
    pub fn enum_value(&self, key: &str) -> Option<&str> {
        self.fields.iter().find(|f| f.key == key).and_then(|f| {
            if let FieldKind::Enum { selected, options } = &f.kind {
                options.get(*selected).map(String::as_str)
            } else {
                None
            }
        })
    }

    /// Reads the items slice of a `StringList` field by `key`.
    #[must_use]
    pub fn string_list_values(&self, key: &str) -> Option<&[String]> {
        self.fields.iter().find(|f| f.key == key).and_then(|f| {
            if let FieldKind::StringList(list) = &f.kind {
                Some(list.items.as_slice())
            } else {
                None
            }
        })
    }

    /// Reads the structured entries slice of a `KeyValueList` field by `key`.
    #[must_use]
    pub fn kv_entries(&self, key: &str) -> Option<&[KeyValueEntry]> {
        self.fields.iter().find(|f| f.key == key).and_then(|f| {
            if let FieldKind::KeyValueList(kv) = &f.kind {
                Some(kv.entries.as_slice())
            } else {
                None
            }
        })
    }

    fn cancel_active_draft(&mut self) -> bool {
        let Some(field) = self.fields.get_mut(self.focused_field) else {
            return false;
        };
        match &mut field.kind {
            FieldKind::StringList(list) if list.editing_draft => {
                list.editing_draft = false;
                list.draft.clear();
                field.error = None;
                true
            }
            FieldKind::KeyValueList(kv) if kv.editing_col.is_some() => {
                kv.cancel_draft();
                field.error = None;
                true
            }
            _ => false,
        }
    }

    fn step_focus(&mut self, forward: bool) {
        let visible = self.visible_field_indices();
        if visible.is_empty() {
            return;
        }
        let pos = visible
            .iter()
            .position(|&idx| idx == self.focused_field)
            .unwrap_or(0);
        let next_pos = if forward {
            pos.saturating_add(1)
                .checked_rem(visible.len())
                .unwrap_or(0)
        } else if pos == 0 {
            visible.len().saturating_sub(1)
        } else {
            pos.saturating_sub(1)
        };
        if let Some(&idx) = visible.get(next_pos) {
            self.focused_field = idx;
        }
    }

    fn step_tab(&mut self, forward: bool) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.active_tab = if forward {
            self.active_tab
                .saturating_add(1)
                .checked_rem(self.tabs.len())
                .unwrap_or(0)
        } else if self.active_tab == 0 {
            self.tabs.len().saturating_sub(1)
        } else {
            self.active_tab.saturating_sub(1)
        };
        self.focus_first_field_in_tab();
    }

    fn focus_first_field_in_tab(&mut self) {
        if let Some(first) = self.visible_field_indices().first().copied() {
            self.focused_field = first;
        }
    }

    fn apply_to_focused_field(&mut self, action: FormAction) {
        let Some(field) = self.fields.get_mut(self.focused_field) else {
            return;
        };
        match &mut field.kind {
            FieldKind::Text(input) => {
                apply_text_action(input, action);
                field.error = input.validate();
                self.dirty = true;
            }
            FieldKind::Toggle(value) => {
                if matches!(
                    action,
                    FormAction::Activate | FormAction::MoveLeft | FormAction::MoveRight
                ) {
                    *value = !*value;
                    self.dirty = true;
                }
            }
            FieldKind::Enum { selected, options } if !options.is_empty() => match action {
                FormAction::Activate | FormAction::MoveRight => {
                    *selected = selected
                        .saturating_add(1)
                        .checked_rem(options.len())
                        .unwrap_or(0);
                    self.dirty = true;
                }
                FormAction::MoveLeft => {
                    *selected = if *selected == 0 {
                        options.len().saturating_sub(1)
                    } else {
                        selected.saturating_sub(1)
                    };
                    self.dirty = true;
                }
                _ => {}
            },
            FieldKind::StringList(list) => {
                apply_string_list_action(list, action, &mut field.error, &mut self.dirty);
            }
            FieldKind::KeyValueList(kv) => {
                apply_kv_list_action(kv, action, &mut field.error, &mut self.dirty);
            }
            FieldKind::Enum { .. } => {}
        }
    }
}

fn apply_text_action(input: &mut TextInputState, action: FormAction) {
    match action {
        FormAction::InsertChar(ch) => input.insert_char(ch),
        FormAction::InsertStr(s) => input.insert_str(&s),
        FormAction::Backspace => input.backspace(),
        FormAction::Delete => input.delete(),
        FormAction::MoveLeft => input.move_left(),
        FormAction::MoveRight => input.move_right(),
        FormAction::MoveHome => input.move_home(),
        FormAction::MoveEnd => input.move_end(),
        _ => {}
    }
}

fn apply_string_list_action(
    list: &mut StringListState,
    action: FormAction,
    error: &mut Option<String>,
    dirty: &mut bool,
) {
    if list.editing_draft {
        match action {
            FormAction::Activate => {
                *error = list.commit_draft();
                *dirty = true;
            }
            other => apply_text_action(&mut list.draft, other),
        }
        return;
    }

    match action {
        FormAction::Activate | FormAction::AddItem => {
            list.editing_draft = true;
            *error = None;
        }
        FormAction::RemoveItem | FormAction::Delete => {
            list.remove_selected();
            *dirty = true;
        }
        FormAction::MoveLeft => {
            list.selected = list.selected.saturating_sub(1);
        }
        FormAction::MoveRight => {
            if !list.items.is_empty() {
                list.selected = list
                    .selected
                    .saturating_add(1)
                    .min(list.items.len().saturating_sub(1));
            }
        }
        FormAction::ReorderItem { up } => {
            list.reorder_selected(up);
            *dirty = true;
        }
        _ => {}
    }
}

fn apply_kv_list_action(
    kv: &mut KeyValueListState,
    action: FormAction,
    error: &mut Option<String>,
    dirty: &mut bool,
) {
    if kv.editing_col.is_some() {
        match action {
            FormAction::Activate => {
                *error = kv.advance_or_commit();
                *dirty = true;
            }
            other => {
                if let Some(draft) = kv.active_draft_mut() {
                    apply_text_action(draft, other);
                }
            }
        }
        return;
    }

    match action {
        FormAction::Activate | FormAction::AddItem => {
            kv.editing_col = Some(0);
            *error = None;
        }
        FormAction::RemoveItem | FormAction::Delete => {
            kv.remove_selected();
            *dirty = true;
        }
        FormAction::MoveLeft => {
            kv.selected = kv.selected.saturating_sub(1);
        }
        FormAction::MoveRight if !kv.entries.is_empty() => {
            kv.selected = kv
                .selected
                .saturating_add(1)
                .min(kv.entries.len().saturating_sub(1));
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_input_handles_cjk_emoji_graphemes_and_validators() {
        let mut input = TextInputState::new("规则🚀", "placeholder", FieldValidator::Required);
        assert_eq!(input.grapheme_len(), 3);
        input.move_left();
        input.insert_char('X');
        assert_eq!(input.value, "规则X🚀");
        input.backspace();
        assert_eq!(input.value, "规则🚀");
        assert_eq!(input.validate(), None);

        let secret = TextInputState::new(
            format!("ghs_{}.{}", "A".repeat(25), "B".repeat(25)),
            "",
            FieldValidator::NoPlaintextSecret,
        );
        assert!(secret.validate().is_some_and(|e| e.contains("LA001")));

        let bad_id = TextInputState::new("Bad ID!", "", FieldValidator::ResourceId);
        assert!(bad_id.validate().is_some());
        let good_kebab = TextInputState::new("release-prep", "", FieldValidator::KebabName);
        assert_eq!(good_kebab.validate(), None);
    }

    #[test]
    fn modal_form_supports_tabs_string_list_kv_list_and_validation() {
        let mut form = ModalFormState::new(
            "Configure Rule",
            vec![
                FormField::text(
                    "id",
                    "Rule ID",
                    "",
                    "rust-safety",
                    FieldValidator::ResourceId,
                ),
                FormField::toggle("enabled", "Enabled", true),
                FormField::enum_select(
                    "trigger",
                    "Trigger",
                    vec!["always_on".into(), "glob".into(), "manual".into()],
                    0,
                ),
                FormField::string_list(
                    "globs",
                    "Globs",
                    vec!["crates/**/*.rs".into()],
                    "src/**/*.ts",
                    FieldValidator::Required,
                ),
                FormField::kv_list(
                    "headers",
                    "Headers",
                    KeyValueListState::new(Vec::new(), "Header", "Value", None, true),
                )
                .on_tab(1),
            ],
        )
        .with_tabs(vec![
            FormTab::general("General"),
            FormTab::for_target("Antigravity", TargetKind::Antigravity),
        ]);

        // Empty required ID blocks submit.
        assert_eq!(form.apply(FormAction::Submit), FormOutcome::Continue);
        assert!(form.submit_error.is_some());

        // Type valid ID, toggle enabled, cycle trigger, add glob item.
        form.apply(FormAction::InsertStr("rust-core".to_owned()));
        form.apply(FormAction::NextField);
        form.apply(FormAction::Activate);
        assert_eq!(form.bool_value("enabled"), Some(false));

        form.apply(FormAction::NextField);
        form.apply(FormAction::MoveRight);
        assert_eq!(form.enum_value("trigger"), Some("glob"));

        form.apply(FormAction::NextField);
        form.apply(FormAction::AddItem);
        form.apply(FormAction::InsertStr("docs/**/*.md".to_owned()));
        form.apply(FormAction::Activate);
        assert_eq!(form.string_list_values("globs").unwrap().len(), 2);

        // Switch to Antigravity tab and add a safe header entry.
        form.apply(FormAction::NextTab);
        assert_eq!(form.active_tab, 1);
        form.apply(FormAction::AddItem);
        form.apply(FormAction::InsertStr("Authorization".to_owned()));
        form.apply(FormAction::Activate);
        form.apply(FormAction::InsertStr("Bearer ${TOKEN}".to_owned()));
        form.apply(FormAction::Activate);
        assert_eq!(form.kv_entries("headers").unwrap().len(), 1);

        assert_eq!(form.apply(FormAction::Submit), FormOutcome::Submit);
    }
}
