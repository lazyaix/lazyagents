//! Atomic grapheme-safe form field primitives (`TextInputState`, `StringListState`,
//! `KeyValueListState`) and real-time domain validators (`LA001`, `ResourceId`, `KebabName`,
//! `Regex`, `HttpsUrl`) for `la-tui` modal forms.

use la_core::{KebabName, ResourceId, scan_secrets};
use regex::Regex;
use unicode_segmentation::UnicodeSegmentation as _;

/// Built-in real-time validation policy attached to a form text field or list item.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum FieldValidator {
    /// No validation constraint.
    #[default]
    None,
    /// Trimmed value must be non-empty.
    Required,
    /// Value must be a valid [`ResourceId`] (`[a-z0-9._-]`).
    ResourceId,
    /// Value must be a strict lowercase kebab-case [`KebabName`].
    KebabName,
    /// Value must be `"*"` or a valid regular expression.
    Regex,
    /// Optional or non-empty value must begin with `"https://"`.
    HttpsUrl,
    /// Value must parse as a positive `u32` (`> 0`).
    PositiveU32,
    /// Rejects plaintext secrets via [`la_core::scan_secrets`] (`LA001`).
    NoPlaintextSecret,
}

/// Grapheme-cluster-safe single-line text input state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TextInputState {
    /// Current UTF-8 value.
    pub value: String,
    /// Cursor position measured in grapheme clusters (`0..=grapheme_len`).
    pub cursor: usize,
    /// Placeholder text displayed when `value` is empty.
    pub placeholder: String,
    /// Real-time validation rule applied on change and submit.
    pub validator: FieldValidator,
}

/// Editable ordered list of strings (globs, args, branch prefixes, owners, patterns).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StringListState {
    /// Ordered string items.
    pub items: Vec<String>,
    /// Currently highlighted item index.
    pub selected: usize,
    /// Draft single-line input for adding a new item.
    pub draft: TextInputState,
    /// Whether keyboard typing is currently captured by `draft`.
    pub editing_draft: bool,
}

/// One structured pair or triple entry inside [`KeyValueListState`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct KeyValueEntry {
    /// Primary key/name column (e.g., env var name, glob, command ID).
    pub key: String,
    /// Secondary value column (e.g., header value, label, decision).
    pub value: String,
    /// Optional third column (e.g., variable description, verify command, rule text).
    pub extra: Option<String>,
}

/// Editable list of key/value pairs (or 3-column tuples) with optional `LA001` secret checking.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct KeyValueListState {
    /// Ordered structured entries.
    pub entries: Vec<KeyValueEntry>,
    /// Highlighted entry index.
    pub selected: usize,
    /// Column header for `key`.
    pub key_label: String,
    /// Column header for `value`.
    pub value_label: String,
    /// Optional column header for `extra` (enables 3-column mode when `Some`).
    pub extra_label: Option<String>,
    /// Active draft column (`Some(0)` = key, `Some(1)` = value, `Some(2)` = extra, `None` = browsing).
    pub editing_col: Option<u8>,
    /// Draft input for `key`.
    pub draft_key: TextInputState,
    /// Draft input for `value`.
    pub draft_value: TextInputState,
    /// Draft input for `extra`.
    pub draft_extra: TextInputState,
    /// When `true`, scans values with [`la_core::scan_secrets`] to prevent plaintext secrets.
    pub check_secrets: bool,
}

/// Typed atomic control variant inside a [`super::FormField`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldKind {
    /// Single-line grapheme-safe text input.
    Text(TextInputState),
    /// Boolean on/off toggle switch.
    Toggle(bool),
    /// Finite cyclic enum picker.
    Enum {
        /// Currently selected option index.
        selected: usize,
        /// Ordered display/canonical option labels.
        options: Vec<String>,
    },
    /// Ordered string list editor.
    StringList(StringListState),
    /// Structured key/value (or 3-column) list editor.
    KeyValueList(KeyValueListState),
}

impl FieldValidator {
    /// Validates `raw` and returns `Some(error_message)` when a constraint is violated.
    #[must_use]
    pub fn validate(self, raw: &str) -> Option<String> {
        let trimmed = raw.trim();
        match self {
            Self::None => None,
            Self::Required => {
                if trimmed.is_empty() {
                    Some("Value is required".to_owned())
                } else {
                    None
                }
            }
            Self::ResourceId => ResourceId::parse(trimmed).err().map(|e| e.to_string()),
            Self::KebabName => KebabName::parse(trimmed).err().map(|e| e.to_string()),
            Self::Regex => {
                if trimmed.is_empty() || trimmed == "*" {
                    None
                } else {
                    Regex::new(trimmed)
                        .err()
                        .map(|e| format!("Invalid regex: {e}"))
                }
            }
            Self::HttpsUrl => {
                if trimmed.is_empty() || trimmed.starts_with("https://") {
                    None
                } else {
                    Some("URL must start with `https://`".to_owned())
                }
            }
            Self::PositiveU32 => match trimmed.parse::<u32>() {
                Ok(n) if n > 0 => None,
                _ => Some("Must be a positive integer (> 0)".to_owned()),
            },
            Self::NoPlaintextSecret => {
                let findings = scan_secrets(raw);
                findings.first().map(|f| {
                    format!(
                        "LA001: plaintext {:?} detected ({}); use `${{VAR}}` placeholder",
                        f.kind, f.preview
                    )
                })
            }
        }
    }
}

impl TextInputState {
    /// Constructs a [`TextInputState`] positioned at the end of `initial`.
    #[must_use]
    pub fn new(
        initial: impl Into<String>,
        placeholder: impl Into<String>,
        validator: FieldValidator,
    ) -> Self {
        let value = initial.into();
        let cursor = value.graphemes(true).count();
        Self {
            value,
            cursor,
            placeholder: placeholder.into(),
            validator,
        }
    }

    /// Returns the grapheme-cluster length of `self.value`.
    #[must_use]
    pub fn grapheme_len(&self) -> usize {
        self.value.graphemes(true).count()
    }

    /// Inserts a single character (ignoring newlines) at the grapheme cursor.
    pub fn insert_char(&mut self, ch: char) {
        if ch == '\n' || ch == '\r' {
            return;
        }
        let byte_idx = grapheme_to_byte_idx(&self.value, self.cursor);
        self.value.insert(byte_idx, ch);
        self.cursor = self.cursor.saturating_add(1).min(self.grapheme_len());
    }

    /// Inserts a pasted single-line string (collapsing line breaks into spaces) at the cursor.
    pub fn insert_str(&mut self, text: &str) {
        let sanitized: String = text
            .chars()
            .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
            .collect();
        if sanitized.is_empty() {
            return;
        }
        let added_graphemes = sanitized.graphemes(true).count();
        let byte_idx = grapheme_to_byte_idx(&self.value, self.cursor);
        self.value.insert_str(byte_idx, &sanitized);
        self.cursor = self
            .cursor
            .saturating_add(added_graphemes)
            .min(self.grapheme_len());
    }

    /// Deletes the grapheme cluster immediately before the cursor.
    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let prev = self.cursor.saturating_sub(1);
        let start = grapheme_to_byte_idx(&self.value, prev);
        let end = grapheme_to_byte_idx(&self.value, self.cursor);
        self.value.replace_range(start..end, "");
        self.cursor = prev;
    }

    /// Deletes the grapheme cluster at the cursor.
    pub fn delete(&mut self) {
        let len = self.grapheme_len();
        if self.cursor >= len {
            return;
        }
        let start = grapheme_to_byte_idx(&self.value, self.cursor);
        let end = grapheme_to_byte_idx(&self.value, self.cursor.saturating_add(1));
        self.value.replace_range(start..end, "");
    }

    /// Moves the cursor one grapheme cluster left.
    pub const fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    /// Moves the cursor one grapheme cluster right.
    pub fn move_right(&mut self) {
        self.cursor = self.cursor.saturating_add(1).min(self.grapheme_len());
    }

    /// Moves the cursor to the start of the input.
    pub const fn move_home(&mut self) {
        self.cursor = 0;
    }

    /// Moves the cursor to the end of the input.
    pub fn move_end(&mut self) {
        self.cursor = self.grapheme_len();
    }

    /// Clears the value and resets the cursor to `0`.
    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
    }

    /// Splits the text into `(before_cursor, at_cursor_or_space, after_cursor)` for rendering.
    #[must_use]
    pub fn split_for_cursor(&self) -> (&str, &str, &str) {
        let len = self.grapheme_len();
        let clamped = self.cursor.min(len);
        let start = grapheme_to_byte_idx(&self.value, clamped);
        let before = self.value.get(..start).unwrap_or("");
        if clamped < len {
            let next = grapheme_to_byte_idx(&self.value, clamped.saturating_add(1));
            let current = self.value.get(start..next).unwrap_or(" ");
            let after = self.value.get(next..).unwrap_or("");
            (before, current, after)
        } else {
            (before, " ", "")
        }
    }

    /// Validates the current text using `self.validator`.
    #[must_use]
    pub fn validate(&self) -> Option<String> {
        self.validator.validate(&self.value)
    }
}

impl StringListState {
    /// Creates a [`StringListState`] initialized with `items` and an item draft placeholder.
    #[must_use]
    pub fn new(
        items: Vec<String>,
        placeholder: impl Into<String>,
        item_validator: FieldValidator,
    ) -> Self {
        Self {
            items,
            selected: 0,
            draft: TextInputState::new("", placeholder, item_validator),
            editing_draft: false,
        }
    }

    /// Commits `self.draft` into `self.items` if valid and non-empty; returns validation error if any.
    pub fn commit_draft(&mut self) -> Option<String> {
        let trimmed = self.draft.value.trim().to_owned();
        if trimmed.is_empty() {
            self.editing_draft = false;
            return None;
        }
        if let Some(err) = self.draft.validator.validate(&trimmed) {
            return Some(err);
        }
        self.items.push(trimmed);
        self.selected = self.items.len().saturating_sub(1);
        self.draft.clear();
        self.editing_draft = false;
        None
    }

    /// Removes the highlighted item, keeping `selected` within bounds.
    pub fn remove_selected(&mut self) {
        if self.selected < self.items.len() {
            self.items.remove(self.selected);
            if self.selected >= self.items.len() {
                self.selected = self.items.len().saturating_sub(1);
            }
        }
    }

    /// Swaps the selected item with its neighbor (`up = true` moves toward index 0).
    pub fn reorder_selected(&mut self, up: bool) {
        if self.items.len() < 2 || self.selected >= self.items.len() {
            return;
        }
        let target = if up {
            let Some(prev) = self.selected.checked_sub(1) else {
                return;
            };
            prev
        } else {
            let next = self.selected.saturating_add(1);
            if next >= self.items.len() {
                return;
            }
            next
        };
        self.items.swap(self.selected, target);
        self.selected = target;
    }
}

impl KeyValueListState {
    /// Constructs a 2-column or 3-column [`KeyValueListState`].
    #[must_use]
    pub fn new(
        entries: Vec<KeyValueEntry>,
        key_label: impl Into<String>,
        value_label: impl Into<String>,
        extra_label: Option<String>,
        check_secrets: bool,
    ) -> Self {
        let k_lbl = key_label.into();
        let v_lbl = value_label.into();
        let e_ph = extra_label.clone().unwrap_or_default();
        let val_validator = if check_secrets {
            FieldValidator::NoPlaintextSecret
        } else {
            FieldValidator::None
        };
        Self {
            entries,
            selected: 0,
            draft_key: TextInputState::new("", k_lbl.clone(), FieldValidator::Required),
            draft_value: TextInputState::new("", v_lbl.clone(), val_validator),
            draft_extra: TextInputState::new("", e_ph, FieldValidator::None),
            key_label: k_lbl,
            value_label: v_lbl,
            extra_label,
            editing_col: None,
            check_secrets,
        }
    }

    /// Returns a mutable reference to the active draft input when in draft mode.
    pub const fn active_draft_mut(&mut self) -> Option<&mut TextInputState> {
        match self.editing_col {
            Some(0) => Some(&mut self.draft_key),
            Some(1) => Some(&mut self.draft_value),
            Some(2) => Some(&mut self.draft_extra),
            _ => None,
        }
    }

    /// Advances draft entry across columns (`0 -> 1 -> [2] -> commit`), returning any validation error.
    pub fn advance_or_commit(&mut self) -> Option<String> {
        match self.editing_col {
            None => {
                self.editing_col = Some(0);
            }
            Some(0) => {
                if let Some(err) = self.draft_key.validate() {
                    return Some(err);
                }
                self.editing_col = Some(1);
            }
            Some(1) => {
                if self.check_secrets
                    && let Some(err) =
                        FieldValidator::NoPlaintextSecret.validate(&self.draft_value.value)
                {
                    return Some(err);
                }
                if self.extra_label.is_some() {
                    self.editing_col = Some(2);
                } else {
                    self.commit_draft();
                }
            }
            Some(_) => {
                if self.check_secrets
                    && let Some(err) =
                        FieldValidator::NoPlaintextSecret.validate(&self.draft_extra.value)
                {
                    return Some(err);
                }
                self.commit_draft();
            }
        }
        None
    }

    /// Cancels an active draft entry and clears draft buffers.
    pub fn cancel_draft(&mut self) {
        self.editing_col = None;
        self.draft_key.clear();
        self.draft_value.clear();
        self.draft_extra.clear();
    }

    /// Removes the highlighted entry, keeping `selected` within bounds.
    pub fn remove_selected(&mut self) {
        if self.selected < self.entries.len() {
            self.entries.remove(self.selected);
            if self.selected >= self.entries.len() {
                self.selected = self.entries.len().saturating_sub(1);
            }
        }
    }

    /// Validates all existing entries (e.g., scanning values for plaintext secrets).
    #[must_use]
    pub fn validate_entries(&self) -> Option<String> {
        for entry in &self.entries {
            if entry.key.trim().is_empty() {
                return Some(format!("{} cannot be empty", self.key_label));
            }
            if self.check_secrets {
                if let Some(err) = FieldValidator::NoPlaintextSecret.validate(&entry.value) {
                    return Some(err);
                }
                if let Some(extra) = &entry.extra
                    && let Some(err) = FieldValidator::NoPlaintextSecret.validate(extra)
                {
                    return Some(err);
                }
            }
        }
        None
    }

    fn commit_draft(&mut self) {
        let key = self.draft_key.value.trim().to_owned();
        let value = self.draft_value.value.trim().to_owned();
        let extra = self
            .extra_label
            .as_ref()
            .map(|_| self.draft_extra.value.trim().to_owned());
        if !key.is_empty() {
            self.entries.push(KeyValueEntry { key, value, extra });
            self.selected = self.entries.len().saturating_sub(1);
        }
        self.cancel_draft();
    }
}

fn grapheme_to_byte_idx(text: &str, grapheme_idx: usize) -> usize {
    text.grapheme_indices(true)
        .nth(grapheme_idx)
        .map_or(text.len(), |(byte_idx, _)| byte_idx)
}
