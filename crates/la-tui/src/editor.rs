//! Pure native grapheme-cluster multiline editor (`TextEditor`) for `la-tui`.
//!
//! Built directly on `unicode-segmentation` + `unicode-width` with zero third-party textarea
//! coupling: 100% CJK/Emoji safe cursor motion, bounded Undo/Redo history, optional Vim
//! Normal/Insert modal editing, and native [`Theme`] token rendering.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};
use unicode_segmentation::UnicodeSegmentation as _;

use crate::config::ColorProfile;
use crate::text::{display_width, grapheme_width};
use crate::theme::Theme;

/// Maximum snapshots kept in the Undo/Redo history stacks.
const MAX_UNDO_STEPS: usize = 128;

/// Active editing mode for [`TextEditor`].
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum EditorMode {
    /// Direct text insertion (standard multiline editor behavior).
    #[default]
    Insert,
    /// Optional Vim-style Normal mode (`h/j/k/l`, `0/$`, `w/b`, `x`, `dd`, `i/a/o`, `u`).
    Normal,
}

/// High-level semantic editing command dispatched to [`TextEditor::apply`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EditorAction {
    /// Insert a single character at the cursor.
    InsertChar(char),
    /// Insert a multi-character pasted string (Bracketed Paste safe).
    InsertStr(String),
    /// Split the current line at the cursor (`Enter` or Kitty `Shift+Enter`).
    Newline,
    /// Delete the grapheme cluster immediately before the cursor.
    Backspace,
    /// Delete the grapheme cluster under the cursor.
    Delete,
    /// Delete the entire current line (`dd` in Vim mode).
    DeleteLine,
    /// Move cursor left by one grapheme cluster.
    MoveLeft,
    /// Move cursor right by one grapheme cluster.
    MoveRight,
    /// Move cursor up one line while preserving visual display column.
    MoveUp,
    /// Move cursor down one line while preserving visual display column.
    MoveDown,
    /// Jump to column 0 of the current line.
    MoveHome,
    /// Jump to the end of the current line.
    MoveEnd,
    /// Jump to the first line.
    MoveTop,
    /// Jump to the last line.
    MoveBottom,
    /// Undo the previous text mutation.
    Undo,
    /// Redo the last undone text mutation.
    Redo,
    /// Switch between [`EditorMode::Insert`] and [`EditorMode::Normal`].
    SetMode(EditorMode),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EditorSnapshot {
    lines: Vec<String>,
    row: usize,
    col: usize,
}

/// Grapheme-aware multiline text buffer with Undo/Redo, optional Vim mode, and viewport scrolling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextEditor {
    lines: Vec<String>,
    /// 0-based line index.
    row: usize,
    /// 0-based grapheme index within `lines[row]`.
    col: usize,
    /// Sticky visual column preserved across vertical `MoveUp` / `MoveDown`.
    preferred_display_col: usize,
    /// Vertical scroll offset in lines.
    scroll_row: usize,
    /// Horizontal scroll offset in terminal cells.
    scroll_col: usize,
    /// Active editing mode (`Insert` vs optional Vim `Normal`).
    mode: EditorMode,
    /// Whether Vim modal editing is enabled.
    vim_enabled: bool,
    /// Dirty flag set whenever content diverges from the baseline loaded text.
    dirty: bool,
    undo_stack: Vec<EditorSnapshot>,
    redo_stack: Vec<EditorSnapshot>,
}

impl Default for TextEditor {
    fn default() -> Self {
        Self::from_text("")
    }
}

impl TextEditor {
    /// Creates a new [`TextEditor`] initialized with `text` (CRLF normalized to LF).
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        let normalized = text.replace("\r\n", "\n");
        let lines: Vec<String> = normalized.split('\n').map(str::to_owned).collect();
        Self {
            lines: if lines.is_empty() {
                vec![String::new()]
            } else {
                lines
            },
            row: 0,
            col: 0,
            preferred_display_col: 0,
            scroll_row: 0,
            scroll_col: 0,
            mode: EditorMode::Insert,
            vim_enabled: false,
            dirty: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Serializes the current buffer back into a single LF-delimited string.
    #[must_use]
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// Returns the number of lines currently in the buffer (always >= 1).
    #[must_use]
    pub const fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Returns the 0-based `(row, grapheme_col)` cursor position.
    #[must_use]
    pub const fn cursor(&self) -> (usize, usize) {
        (self.row, self.col)
    }

    /// Returns the active [`EditorMode`].
    #[must_use]
    pub const fn mode(&self) -> EditorMode {
        self.mode
    }

    /// Enables or disables optional Vim modal editing.
    pub const fn set_vim_enabled(&mut self, enabled: bool) {
        self.vim_enabled = enabled;
        if !enabled {
            self.mode = EditorMode::Insert;
        }
    }

    /// Returns `true` when Vim modal editing is active.
    #[must_use]
    pub const fn vim_enabled(&self) -> bool {
        self.vim_enabled
    }

    /// Returns `true` when the buffer has unsaved edits.
    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clears the unsaved-changes flag (e.g., immediately after atomic save).
    pub const fn mark_clean(&mut self) {
        self.dirty = false;
    }

    /// Applies one [`EditorAction`] and maintains cursor + Undo/Redo invariants.
    pub fn apply(&mut self, action: EditorAction) {
        match action {
            EditorAction::InsertChar(ch) => {
                if ch == '\n' {
                    self.insert_newline();
                } else {
                    self.push_undo();
                    let mut buf = [0u8; 4];
                    let s = ch.encode_utf8(&mut buf);
                    self.insert_str_inline(s);
                }
            }
            EditorAction::InsertStr(text) => {
                if text.is_empty() {
                    return;
                }
                self.push_undo();
                let normalized = text.replace("\r\n", "\n");
                for (idx, part) in normalized.split('\n').enumerate() {
                    if idx > 0 {
                        self.split_current_line();
                    }
                    self.insert_str_inline(part);
                }
            }
            EditorAction::Newline => self.insert_newline(),
            EditorAction::Backspace => self.backspace(),
            EditorAction::Delete => self.delete_at_cursor(),
            EditorAction::DeleteLine => self.delete_current_line(),
            EditorAction::MoveLeft => self.move_left(),
            EditorAction::MoveRight => self.move_right(),
            EditorAction::MoveUp => self.move_up(),
            EditorAction::MoveDown => self.move_down(),
            EditorAction::MoveHome => {
                self.col = 0;
                self.preferred_display_col = 0;
            }
            EditorAction::MoveEnd => {
                self.col = self.current_line_grapheme_len();
                self.preferred_display_col = self.cursor_display_col();
            }
            EditorAction::MoveTop => {
                self.row = 0;
                self.snap_col_to_preferred();
            }
            EditorAction::MoveBottom => {
                self.row = self.lines.len().saturating_sub(1);
                self.snap_col_to_preferred();
            }
            EditorAction::Undo => self.undo(),
            EditorAction::Redo => self.redo(),
            EditorAction::SetMode(mode) => {
                self.mode = if self.vim_enabled {
                    mode
                } else {
                    EditorMode::Insert
                };
            }
        }
    }

    /// Renders the editor into `area` using strictly semantic [`Theme`] tokens.
    pub fn render_into(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        block: Option<Block<'_>>,
        theme: &Theme,
        focused: bool,
    ) {
        let inner = block.map_or(area, |b| {
            let inner = b.inner(area);
            b.render(area, buf);
            inner
        });
        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let gutter_width = line_number_gutter_width(self.lines.len());
        let content_width = usize::from(inner.width).saturating_sub(gutter_width).max(1);
        let visible_rows = usize::from(inner.height);
        self.ensure_cursor_visible(visible_rows, content_width);

        let mut rendered_lines = Vec::with_capacity(visible_rows);
        let end_row = self
            .scroll_row
            .saturating_add(visible_rows)
            .min(self.lines.len());

        for row_idx in self.scroll_row..end_row {
            let is_cursor_row = row_idx == self.row;
            let num_str = format!(
                "{:>width$} ",
                row_idx.saturating_add(1),
                width = gutter_width.saturating_sub(1)
            );
            let num_style = if is_cursor_row && focused {
                Style::default()
                    .fg(theme.accent_primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                theme.muted_style()
            };

            let mut spans = vec![Span::styled(num_str, num_style)];
            let line_text = self.lines.get(row_idx).map_or("", String::as_str);
            spans.extend(render_editor_line_spans(
                line_text,
                is_cursor_row && focused,
                self.col,
                self.scroll_col,
                content_width,
                theme,
            ));

            rendered_lines.push(Line::from(spans));
        }

        Paragraph::new(rendered_lines)
            .style(theme.surface_style())
            .render(inner, buf);
    }

    fn insert_newline(&mut self) {
        self.push_undo();
        self.split_current_line();
    }

    fn split_current_line(&mut self) {
        let byte_idx = self.cursor_byte_offset();
        let tail = self
            .lines
            .get_mut(self.row)
            .map_or_else(String::new, |line| line.split_off(byte_idx));
        self.row = self.row.saturating_add(1);
        self.lines.insert(self.row, tail);
        self.col = 0;
        self.preferred_display_col = 0;
        self.dirty = true;
    }

    fn insert_str_inline(&mut self, fragment: &str) {
        if fragment.is_empty() {
            return;
        }
        let byte_idx = self.cursor_byte_offset();
        if let Some(line) = self.lines.get_mut(self.row) {
            line.insert_str(byte_idx, fragment);
        }
        let inserted_graphemes = fragment.graphemes(true).count();
        self.col = self.col.saturating_add(inserted_graphemes);
        self.preferred_display_col = self.cursor_display_col();
        self.dirty = true;
    }

    fn backspace(&mut self) {
        if self.col > 0 {
            self.push_undo();
            let prev_col = self.col.saturating_sub(1);
            let start = self.grapheme_byte_offset(self.row, prev_col);
            let end = self.grapheme_byte_offset(self.row, self.col);
            if let Some(line) = self.lines.get_mut(self.row) {
                line.replace_range(start..end, "");
            }
            self.col = prev_col;
            self.preferred_display_col = self.cursor_display_col();
            self.dirty = true;
        } else if self.row > 0 {
            self.push_undo();
            let removed = self.lines.remove(self.row);
            self.row = self.row.saturating_sub(1);
            let join_col = self.current_line_grapheme_len();
            if let Some(prev_line) = self.lines.get_mut(self.row) {
                prev_line.push_str(&removed);
            }
            self.col = join_col;
            self.preferred_display_col = self.cursor_display_col();
            self.dirty = true;
        }
    }

    fn delete_at_cursor(&mut self) {
        let len = self.current_line_grapheme_len();
        if self.col < len {
            self.push_undo();
            let start = self.grapheme_byte_offset(self.row, self.col);
            let end = self.grapheme_byte_offset(self.row, self.col.saturating_add(1));
            if let Some(line) = self.lines.get_mut(self.row) {
                line.replace_range(start..end, "");
            }
            self.dirty = true;
        } else if self.row.saturating_add(1) < self.lines.len() {
            self.push_undo();
            let next_line = self.lines.remove(self.row.saturating_add(1));
            if let Some(line) = self.lines.get_mut(self.row) {
                line.push_str(&next_line);
            }
            self.dirty = true;
        }
    }

    fn delete_current_line(&mut self) {
        self.push_undo();
        if self.lines.len() == 1 {
            if let Some(only) = self.lines.first_mut() {
                only.clear();
            }
            self.col = 0;
        } else {
            self.lines.remove(self.row);
            if self.row >= self.lines.len() {
                self.row = self.lines.len().saturating_sub(1);
            }
            self.col = self.col.min(self.current_line_grapheme_len());
        }
        self.preferred_display_col = self.cursor_display_col();
        self.dirty = true;
    }

    fn move_left(&mut self) {
        if self.col > 0 {
            self.col = self.col.saturating_sub(1);
        } else if self.row > 0 {
            self.row = self.row.saturating_sub(1);
            self.col = self.current_line_grapheme_len();
        }
        self.preferred_display_col = self.cursor_display_col();
    }

    fn move_right(&mut self) {
        let len = self.current_line_grapheme_len();
        if self.col < len {
            self.col = self.col.saturating_add(1);
        } else if self.row.saturating_add(1) < self.lines.len() {
            self.row = self.row.saturating_add(1);
            self.col = 0;
        }
        self.preferred_display_col = self.cursor_display_col();
    }

    fn move_up(&mut self) {
        if self.row > 0 {
            self.row = self.row.saturating_sub(1);
            self.snap_col_to_preferred();
        }
    }

    fn move_down(&mut self) {
        if self.row.saturating_add(1) < self.lines.len() {
            self.row = self.row.saturating_add(1);
            self.snap_col_to_preferred();
        }
    }

    fn snap_col_to_preferred(&mut self) {
        let line = self.lines.get(self.row).map_or("", String::as_str);
        let mut width_acc = 0usize;
        let mut target_col = 0usize;
        for (idx, g) in line.graphemes(true).enumerate() {
            let gw = grapheme_width(g);
            if width_acc.saturating_add(gw) > self.preferred_display_col {
                break;
            }
            width_acc = width_acc.saturating_add(gw);
            target_col = idx.saturating_add(1);
        }
        self.col = target_col;
    }

    fn push_undo(&mut self) {
        if self.undo_stack.len() >= MAX_UNDO_STEPS {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(EditorSnapshot {
            lines: self.lines.clone(),
            row: self.row,
            col: self.col,
        });
        self.redo_stack.clear();
    }

    fn undo(&mut self) {
        let Some(prev) = self.undo_stack.pop() else {
            return;
        };
        self.redo_stack.push(EditorSnapshot {
            lines: core::mem::replace(&mut self.lines, prev.lines),
            row: self.row,
            col: self.col,
        });
        self.row = prev.row;
        self.col = prev.col;
        self.preferred_display_col = self.cursor_display_col();
        self.dirty = true;
    }

    fn redo(&mut self) {
        let Some(next) = self.redo_stack.pop() else {
            return;
        };
        self.undo_stack.push(EditorSnapshot {
            lines: core::mem::replace(&mut self.lines, next.lines),
            row: self.row,
            col: self.col,
        });
        self.row = next.row;
        self.col = next.col;
        self.preferred_display_col = self.cursor_display_col();
        self.dirty = true;
    }

    fn ensure_cursor_visible(&mut self, visible_rows: usize, content_width: usize) {
        if self.row < self.scroll_row {
            self.scroll_row = self.row;
        } else if self.row >= self.scroll_row.saturating_add(visible_rows) {
            self.scroll_row = self.row.saturating_sub(visible_rows.saturating_sub(1));
        }

        let display_col = self.cursor_display_col();
        if display_col < self.scroll_col {
            self.scroll_col = display_col;
        } else if display_col >= self.scroll_col.saturating_add(content_width) {
            self.scroll_col = display_col.saturating_sub(content_width.saturating_sub(1));
        }
    }

    fn current_line_grapheme_len(&self) -> usize {
        self.lines
            .get(self.row)
            .map_or(0, |line| line.graphemes(true).count())
    }

    fn cursor_byte_offset(&self) -> usize {
        self.grapheme_byte_offset(self.row, self.col)
    }

    fn grapheme_byte_offset(&self, row: usize, grapheme_idx: usize) -> usize {
        let Some(line) = self.lines.get(row) else {
            return 0;
        };
        line.grapheme_indices(true)
            .nth(grapheme_idx)
            .map_or(line.len(), |(byte_idx, _)| byte_idx)
    }

    fn cursor_display_col(&self) -> usize {
        let Some(line) = self.lines.get(self.row) else {
            return 0;
        };
        let byte_end = self.cursor_byte_offset();
        display_width(line.get(..byte_end).unwrap_or_default())
    }
}

fn line_number_gutter_width(total_lines: usize) -> usize {
    let digits = total_lines.max(1).ilog10();
    usize::try_from(digits)
        .unwrap_or(2)
        .saturating_add(3)
        .max(4)
}

fn render_editor_line_spans(
    line_text: &str,
    show_cursor: bool,
    cursor_col: usize,
    scroll_col: usize,
    max_width: usize,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let base_style = if show_cursor && theme.color_profile != ColorProfile::NoColor {
        Style::default()
            .fg(theme.text_primary)
            .bg(theme.bg_selection)
    } else {
        Style::default().fg(theme.text_primary)
    };
    let cursor_style = if theme.color_profile == ColorProfile::NoColor {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
            .fg(theme.text_inverse)
            .bg(theme.accent_primary)
            .add_modifier(Modifier::BOLD)
    };

    let mut spans = Vec::new();
    let mut cell_x = 0usize;
    let mut rendered_w = 0usize;
    let mut cursor_rendered = false;

    for (g_idx, g) in line_text.graphemes(true).enumerate() {
        let gw = grapheme_width(g);
        let next_x = cell_x.saturating_add(gw);
        if next_x <= scroll_col {
            cell_x = next_x;
            continue;
        }
        if rendered_w.saturating_add(gw) > max_width {
            break;
        }
        let is_cur = show_cursor && g_idx == cursor_col;
        if is_cur {
            cursor_rendered = true;
        }
        let style = if is_cur { cursor_style } else { base_style };
        spans.push(Span::styled(g.to_owned(), style));
        cell_x = next_x;
        rendered_w = rendered_w.saturating_add(gw);
    }

    if show_cursor && !cursor_rendered && rendered_w < max_width {
        spans.push(Span::styled(" ", cursor_style));
    }
    spans
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use crate::config::GlyphSet;
    use crate::theme::ResolvedTheme;

    use super::*;

    #[test]
    fn handles_cjk_emoji_editing_undo_redo_and_rendering() {
        let mut editor = TextEditor::from_text("第一行🚀\nSecond");
        editor.apply(EditorAction::MoveRight);
        editor.apply(EditorAction::MoveRight);
        editor.apply(EditorAction::InsertStr("赛博".to_owned()));
        assert_eq!(editor.text(), "第一赛博行🚀\nSecond");
        assert!(editor.is_dirty());

        editor.apply(EditorAction::Undo);
        assert_eq!(editor.text(), "第一行🚀\nSecond");

        editor.apply(EditorAction::Redo);
        assert_eq!(editor.text(), "第一赛博行🚀\nSecond");

        editor.apply(EditorAction::MoveEnd);
        editor.apply(EditorAction::Backspace);
        assert_eq!(editor.text(), "第一赛博行\nSecond");

        let theme = Theme::build(
            ResolvedTheme::CyberNeon,
            ColorProfile::TrueColor,
            GlyphSet::NerdFont,
        );
        let backend = TestBackend::new(30, 6);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                editor.render_into(
                    frame.area(),
                    frame.buffer_mut(),
                    Some(Block::bordered().title("Editor")),
                    &theme,
                    true,
                );
            })
            .unwrap();
    }

    proptest! {
        #[test]
        fn arbitrary_edit_sequences_never_panic_or_violate_cursor_bounds(
            initial in "[a-z中🚀\n]{0,20}",
            ops in prop::collection::vec(0u8..12, 0..40),
        ) {
            let mut ed = TextEditor::from_text(&initial);
            for op in ops {
                match op {
                    0 => ed.apply(EditorAction::InsertChar('x')),
                    1 => ed.apply(EditorAction::InsertChar('中')),
                    2 => ed.apply(EditorAction::Newline),
                    3 => ed.apply(EditorAction::Backspace),
                    4 => ed.apply(EditorAction::Delete),
                    5 => ed.apply(EditorAction::MoveLeft),
                    6 => ed.apply(EditorAction::MoveRight),
                    7 => ed.apply(EditorAction::MoveUp),
                    8 => ed.apply(EditorAction::MoveDown),
                    9 => ed.apply(EditorAction::Undo),
                    10 => ed.apply(EditorAction::Redo),
                    _ => ed.apply(EditorAction::DeleteLine),
                }
                let (row, col) = ed.cursor();
                prop_assert!(row < ed.line_count());
                let line_len = ed.text().split('\n').nth(row).unwrap_or("").graphemes(true).count();
                prop_assert!(col <= line_len);
            }
        }
    }
}
