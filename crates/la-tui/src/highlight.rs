//! Pure-Rust syntax highlighting (`syntect` with `default-fancy`), Markdown rendering
//! (`pulldown-cmark`), word-level diff styling (`similar`), and bounded layout caching.
//!
//! All emitted colors pass through [`Theme::adapt_rgb`] so previews degrade losslessly across
//! `TrueColor -> Ansi256 -> Ansi16 -> NoColor`.

use core::fmt;
use std::collections::{HashMap, VecDeque};
use std::ffi::OsStr;
use std::hash::{Hash, Hasher};
use std::path::Path;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Parser as MdParser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use similar::{ChangeTag, TextDiff};
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, ThemeSet};
use syntect::parsing::SyntaxSet;

use crate::config::ColorProfile;
use crate::text::wrap_graphemes;
use crate::theme::{ResolvedTheme, Theme};

/// Maximum cached rendered documents kept in [`PreviewCache`] to bound memory deterministically.
const MAX_CACHE_ENTRIES: usize = 32;

/// Source document format for live preview rendering.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PreviewSyntax {
    /// Rich Markdown (`AGENTS.md`, conditional rules, `SKILL.md`, subagents).
    Markdown,
    /// TOML manifest (`.lazyagents/lazyagents.toml`, `local.toml`).
    Toml,
    /// JSON (`hooks.json`, `mcp_config.json`, `plugin.json`, `lock.json`).
    Json,
    /// YAML (`.github/workflows/*.yml`, `dependabot.yml`).
    Yaml,
    /// Plain text fallback.
    Plain,
}

/// One row in a side-by-side diff view with left (disk) and right (compiled) styled lines.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SideBySideRow {
    /// Optional 1-based line number in the old (disk) file.
    pub old_line_no: Option<usize>,
    /// Styled spans for the old (disk) side.
    pub old_line: Line<'static>,
    /// Optional 1-based line number in the new (compiled) file.
    pub new_line_no: Option<usize>,
    /// Styled spans for the new (compiled) side.
    pub new_line: Line<'static>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct CacheKey {
    content_hash: u64,
    syntax: PreviewSyntax,
    width: u16,
    theme_variant: ResolvedTheme,
    color_profile: ColorProfile,
}

/// Pure-Rust Markdown, syntax-highlighting, and diff renderer with bounded render caching.
pub struct PreviewRenderer {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    cache: HashMap<CacheKey, Vec<Line<'static>>>,
    order: VecDeque<CacheKey>,
}

impl fmt::Debug for PreviewRenderer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreviewRenderer")
            .field("cached_entries", &self.cache.len())
            .finish_non_exhaustive()
    }
}

impl Default for PreviewRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl PreviewSyntax {
    /// Infers [`PreviewSyntax`] from a workspace-relative file path extension.
    #[must_use]
    pub fn from_path(path: &str) -> Self {
        let Some(ext) = Path::new(path).extension().and_then(OsStr::to_str) else {
            return Self::Plain;
        };
        if ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown") {
            Self::Markdown
        } else if ext.eq_ignore_ascii_case("toml") {
            Self::Toml
        } else if ext.eq_ignore_ascii_case("json") {
            Self::Json
        } else if ext.eq_ignore_ascii_case("yml") || ext.eq_ignore_ascii_case("yaml") {
            Self::Yaml
        } else {
            Self::Plain
        }
    }

    const fn syntect_token(self) -> &'static str {
        match self {
            Self::Markdown => "md",
            Self::Toml => "toml",
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Plain => "txt",
        }
    }
}

impl PreviewRenderer {
    /// Initializes pure-Rust `syntect` syntax and theme sets (no C `onig` dependency).
    #[must_use]
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            cache: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    /// Renders `content` into theme-adapted [`Line`]s, hitting the cache on identical
    /// `(content_hash, syntax, width, theme)` tuples.
    pub fn render_cached(
        &mut self,
        content: &str,
        syntax: PreviewSyntax,
        width: u16,
        theme: &Theme,
    ) -> &[Line<'static>] {
        let key = CacheKey {
            content_hash: hash_str(content),
            syntax,
            width,
            theme_variant: theme.variant,
            color_profile: theme.color_profile,
        };

        if !self.cache.contains_key(&key) {
            let rendered = match syntax {
                PreviewSyntax::Markdown => self.render_markdown(content, width, theme),
                PreviewSyntax::Toml | PreviewSyntax::Json | PreviewSyntax::Yaml => {
                    self.highlight_code(content, syntax.syntect_token(), theme)
                }
                PreviewSyntax::Plain => render_plain_wrapped(content, width, theme),
            };
            self.insert_cache(key, rendered);
        }

        self.cache.get(&key).map_or(&[], Vec::as_slice)
    }

    /// Renders a highlighted code block using pure-Rust `syntect` (`default-fancy`).
    #[must_use]
    pub fn highlight_code(&self, code: &str, lang_hint: &str, theme: &Theme) -> Vec<Line<'static>> {
        let syntax = self
            .syntax_set
            .find_syntax_by_token(lang_hint)
            .or_else(|| self.syntax_set.find_syntax_by_extension(lang_hint))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let syn_theme = self
            .theme_set
            .themes
            .get(theme.syntax_theme)
            .or_else(|| self.theme_set.themes.values().next());

        let Some(syn_theme) = syn_theme else {
            return render_plain_lines(code, theme);
        };

        let mut highlighter = HighlightLines::new(syntax, syn_theme);
        let mut lines = Vec::new();
        for raw_line in code.split('\n') {
            let line_with_nl = format!("{raw_line}\n");
            match highlighter.highlight_line(&line_with_nl, &self.syntax_set) {
                Ok(ranges) => {
                    let mut spans = Vec::with_capacity(ranges.len());
                    for (style, text) in ranges {
                        let cleaned = text.trim_end_matches(['\r', '\n']);
                        if cleaned.is_empty() {
                            continue;
                        }
                        spans.push(Span::styled(
                            cleaned.to_owned(),
                            adapt_syntect_style(style, theme),
                        ));
                    }
                    lines.push(Line::from(spans));
                }
                Err(_) => {
                    lines.push(Line::from(Span::styled(
                        raw_line.to_owned(),
                        Style::default().fg(theme.text_primary),
                    )));
                }
            }
        }
        lines
    }

    /// Renders Markdown with headings, lists, blockquotes, inline code, and syntax-highlighted
    /// fenced code blocks wrapped safely at grapheme boundaries.
    #[must_use]
    pub fn render_markdown(&self, markdown: &str, width: u16, theme: &Theme) -> Vec<Line<'static>> {
        let wrap_w = usize::from(width.max(16));
        let mut lines: Vec<Line<'static>> = Vec::new();
        let mut current_spans: Vec<Span<'static>> = Vec::new();
        let mut style_stack: Vec<Style> = vec![Style::default().fg(theme.text_primary)];
        let mut in_code_block: Option<(String, String)> = None;
        let mut in_list_item = false;

        for event in MdParser::new(markdown) {
            if let Some((ref lang, ref mut code_buf)) = in_code_block {
                match event {
                    Event::Text(text) => code_buf.push_str(&text),
                    Event::End(TagEnd::CodeBlock) => {
                        let lang_owned = lang.clone();
                        let code_owned = core::mem::take(code_buf);
                        in_code_block = None;
                        self.push_fenced_code_block(&lang_owned, &code_owned, theme, &mut lines);
                    }
                    _ => {}
                }
                continue;
            }

            if let Event::Start(Tag::CodeBlock(kind)) = event {
                flush_spans(&mut current_spans, &mut lines);
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => info
                        .split_whitespace()
                        .next()
                        .unwrap_or_default()
                        .to_owned(),
                    CodeBlockKind::Indented => String::new(),
                };
                in_code_block = Some((lang, String::new()));
                continue;
            }

            apply_markdown_event(
                event,
                wrap_w,
                theme,
                &mut style_stack,
                &mut in_list_item,
                &mut current_spans,
                &mut lines,
            );
        }
        flush_spans(&mut current_spans, &mut lines);
        lines
    }

    fn push_fenced_code_block(
        &self,
        lang: &str,
        code: &str,
        theme: &Theme,
        lines: &mut Vec<Line<'static>>,
    ) {
        let header_label = if lang.is_empty() { "code" } else { lang };
        lines.push(Line::from(Span::styled(
            format!("┌─ {header_label} "),
            Style::default().fg(theme.border_muted),
        )));
        for code_line in self.highlight_code(code.trim_end_matches('\n'), lang, theme) {
            let mut prefixed = vec![Span::styled(
                format!("{} ", theme.glyphs.sep),
                Style::default().fg(theme.border_muted),
            )];
            prefixed.extend(code_line.spans);
            lines.push(Line::from(prefixed));
        }
        lines.push(Line::from(Span::styled(
            "└─",
            Style::default().fg(theme.border_muted),
        )));
        lines.push(Line::default());
    }

    fn insert_cache(&mut self, key: CacheKey, lines: Vec<Line<'static>>) {
        if self.cache.len() >= MAX_CACHE_ENTRIES
            && let Some(oldest) = self.order.pop_front()
        {
            self.cache.remove(&oldest);
        }
        self.order.push_back(key);
        self.cache.insert(key, lines);
    }
}

/// Renders a unified diff string (`---`/`+++`/`@@`/`+`/`-`) with semantic [`Theme`] styling.
#[must_use]
pub fn render_unified_diff_styled(unified_diff: &str, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for raw in unified_diff.lines() {
        let style = if raw.starts_with("@@") {
            Style::default()
                .fg(theme.diff_hunk)
                .add_modifier(Modifier::BOLD)
        } else if raw.starts_with("+++") || raw.starts_with("---") {
            Style::default()
                .fg(theme.text_secondary)
                .add_modifier(Modifier::BOLD)
        } else if raw.starts_with('+') {
            diff_add_style(theme, false)
        } else if raw.starts_with('-') {
            diff_del_style(theme, false)
        } else {
            Style::default().fg(theme.text_secondary)
        };
        lines.push(Line::from(Span::styled(raw.to_owned(), style)));
    }
    lines
}

/// Computes a word-level highlighted side-by-side diff comparing `before` (disk) vs `after` (compiled).
#[must_use]
pub fn render_side_by_side_diff(before: &str, after: &str, theme: &Theme) -> Vec<SideBySideRow> {
    let diff = TextDiff::from_lines(before, after);
    let mut rows = Vec::new();
    let mut old_no = 1usize;
    let mut new_no = 1usize;

    for op in diff.ops() {
        match *op {
            similar::DiffOp::Equal { old_index, len, .. } => {
                for offset in 0..len {
                    let text = diff
                        .old_slice(old_index.saturating_add(offset))
                        .unwrap_or_default()
                        .trim_end_matches(['\r', '\n']);
                    let line = Line::from(Span::styled(
                        text.to_owned(),
                        Style::default().fg(theme.text_secondary),
                    ));
                    rows.push(SideBySideRow {
                        old_line_no: Some(old_no),
                        old_line: line.clone(),
                        new_line_no: Some(new_no),
                        new_line: line,
                    });
                    old_no = old_no.saturating_add(1);
                    new_no = new_no.saturating_add(1);
                }
            }
            similar::DiffOp::Delete {
                old_index, old_len, ..
            } => {
                for offset in 0..old_len {
                    let text = diff
                        .old_slice(old_index.saturating_add(offset))
                        .unwrap_or_default()
                        .trim_end_matches(['\r', '\n']);
                    rows.push(SideBySideRow {
                        old_line_no: Some(old_no),
                        old_line: Line::from(Span::styled(
                            text.to_owned(),
                            diff_del_style(theme, false),
                        )),
                        new_line_no: None,
                        new_line: Line::default(),
                    });
                    old_no = old_no.saturating_add(1);
                }
            }
            similar::DiffOp::Insert {
                new_index, new_len, ..
            } => {
                for offset in 0..new_len {
                    let text = diff
                        .new_slice(new_index.saturating_add(offset))
                        .unwrap_or_default()
                        .trim_end_matches(['\r', '\n']);
                    rows.push(SideBySideRow {
                        old_line_no: None,
                        old_line: Line::default(),
                        new_line_no: Some(new_no),
                        new_line: Line::from(Span::styled(
                            text.to_owned(),
                            diff_add_style(theme, false),
                        )),
                    });
                    new_no = new_no.saturating_add(1);
                }
            }
            similar::DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                push_replace_rows(
                    &diff,
                    old_index,
                    old_len,
                    new_index,
                    new_len,
                    &mut old_no,
                    &mut new_no,
                    theme,
                    &mut rows,
                );
            }
        }
    }

    rows
}

fn apply_markdown_event(
    event: Event<'_>,
    wrap_w: usize,
    theme: &Theme,
    style_stack: &mut Vec<Style>,
    in_list_item: &mut bool,
    current_spans: &mut Vec<Span<'static>>,
    lines: &mut Vec<Line<'static>>,
) {
    match event {
        Event::Start(Tag::Heading { level, .. }) => {
            flush_spans(current_spans, lines);
            let style = heading_style(level, theme);
            let prefix = match level {
                HeadingLevel::H1 => "# ",
                HeadingLevel::H2 => "## ",
                _ => "### ",
            };
            current_spans.push(Span::styled(prefix, style));
            style_stack.push(style);
        }
        Event::End(TagEnd::Heading(_)) => {
            style_stack.pop();
            flush_spans(current_spans, lines);
            lines.push(Line::default());
        }
        Event::Start(Tag::Strong) => {
            style_stack.push(current_style(style_stack, theme).add_modifier(Modifier::BOLD));
        }
        Event::Start(Tag::Emphasis) => {
            style_stack.push(current_style(style_stack, theme).add_modifier(Modifier::ITALIC));
        }
        Event::Start(Tag::BlockQuote(_)) => {
            flush_spans(current_spans, lines);
            current_spans.push(Span::styled(
                format!("{} ", theme.glyphs.sep),
                Style::default().fg(theme.accent_secondary),
            ));
            style_stack.push(
                Style::default()
                    .fg(theme.text_secondary)
                    .add_modifier(Modifier::ITALIC),
            );
        }
        Event::End(TagEnd::Strong | TagEnd::Emphasis | TagEnd::BlockQuote(_)) => {
            style_stack.pop();
        }
        Event::Start(Tag::Item) => {
            flush_spans(current_spans, lines);
            *in_list_item = true;
            current_spans.push(Span::styled(
                format!(" {} ", theme.glyphs.cursor),
                Style::default().fg(theme.accent_primary),
            ));
        }
        Event::End(TagEnd::Item) => {
            *in_list_item = false;
            flush_spans(current_spans, lines);
        }
        Event::End(TagEnd::Paragraph) => {
            flush_spans(current_spans, lines);
            if !*in_list_item {
                lines.push(Line::default());
            }
        }
        Event::Code(code) => {
            let code_style = if theme.color_profile == ColorProfile::NoColor {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(theme.accent_glow)
                    .bg(theme.bg_surface_elevated)
            };
            current_spans.push(Span::styled(format!("`{code}`"), code_style));
        }
        Event::Text(text) => {
            let style = current_style(style_stack, theme);
            for (idx, piece) in wrap_graphemes(&text, wrap_w).into_iter().enumerate() {
                if idx > 0 {
                    flush_spans(current_spans, lines);
                }
                current_spans.push(Span::styled(piece, style));
            }
        }
        Event::SoftBreak => {
            current_spans.push(Span::styled(" ", current_style(style_stack, theme)));
        }
        Event::HardBreak => flush_spans(current_spans, lines),
        Event::Rule => {
            flush_spans(current_spans, lines);
            lines.push(Line::from(Span::styled(
                "─".repeat(wrap_w.min(48)),
                Style::default().fg(theme.border_muted),
            )));
        }
        _ => {}
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "internal side-by-side diff replace-op helper keeps caller cleanly under line threshold"
)]
fn push_replace_rows(
    diff: &TextDiff<'_, '_, str>,
    old_index: usize,
    old_len: usize,
    new_index: usize,
    new_len: usize,
    old_no: &mut usize,
    new_no: &mut usize,
    theme: &Theme,
    rows: &mut Vec<SideBySideRow>,
) {
    let paired = old_len.max(new_len);
    for offset in 0..paired {
        let old_txt = if offset < old_len {
            diff.old_slice(old_index.saturating_add(offset))
                .map(|s| s.trim_end_matches(['\r', '\n']))
        } else {
            None
        };
        let new_txt = if offset < new_len {
            diff.new_slice(new_index.saturating_add(offset))
                .map(|s| s.trim_end_matches(['\r', '\n']))
        } else {
            None
        };

        let (old_line, new_line) = match (old_txt, new_txt) {
            (Some(o), Some(n)) => render_inline_word_pair(o, n, theme),
            (Some(o), None) => (
                Line::from(Span::styled(o.to_owned(), diff_del_style(theme, false))),
                Line::default(),
            ),
            (None, Some(n)) => (
                Line::default(),
                Line::from(Span::styled(n.to_owned(), diff_add_style(theme, false))),
            ),
            (None, None) => (Line::default(), Line::default()),
        };

        let cur_old = old_txt.map(|_| {
            let n = *old_no;
            *old_no = old_no.saturating_add(1);
            n
        });
        let cur_new = new_txt.map(|_| {
            let n = *new_no;
            *new_no = new_no.saturating_add(1);
            n
        });

        rows.push(SideBySideRow {
            old_line_no: cur_old,
            old_line,
            new_line_no: cur_new,
            new_line,
        });
    }
}

fn render_inline_word_pair(
    old_line: &str,
    new_line: &str,
    theme: &Theme,
) -> (Line<'static>, Line<'static>) {
    let word_diff = TextDiff::from_words(old_line, new_line);
    let mut old_spans = Vec::new();
    let mut new_spans = Vec::new();

    for change in word_diff.iter_all_changes() {
        let value = change.value().to_owned();
        match change.tag() {
            ChangeTag::Equal => {
                old_spans.push(Span::styled(value.clone(), diff_del_style(theme, false)));
                new_spans.push(Span::styled(value, diff_add_style(theme, false)));
            }
            ChangeTag::Delete => {
                old_spans.push(Span::styled(value, diff_del_style(theme, true)));
            }
            ChangeTag::Insert => {
                new_spans.push(Span::styled(value, diff_add_style(theme, true)));
            }
        }
    }
    (Line::from(old_spans), Line::from(new_spans))
}

fn diff_add_style(theme: &Theme, emphasized: bool) -> Style {
    if theme.color_profile == ColorProfile::NoColor {
        let modifier = if emphasized {
            Modifier::BOLD | Modifier::UNDERLINED
        } else {
            Modifier::BOLD
        };
        return Style::default().add_modifier(modifier);
    }
    let mut style = Style::default().fg(theme.diff_add_fg).bg(theme.diff_add_bg);
    if emphasized {
        style = style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
    }
    style
}

fn diff_del_style(theme: &Theme, emphasized: bool) -> Style {
    if theme.color_profile == ColorProfile::NoColor {
        let modifier = if emphasized {
            Modifier::DIM | Modifier::CROSSED_OUT
        } else {
            Modifier::DIM
        };
        return Style::default().add_modifier(modifier);
    }
    let mut style = Style::default().fg(theme.diff_del_fg).bg(theme.diff_del_bg);
    if emphasized {
        style = style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
    }
    style
}

fn adapt_syntect_style(syn: syntect::highlighting::Style, theme: &Theme) -> Style {
    let mut style =
        Style::default().fg(theme.adapt_rgb(syn.foreground.r, syn.foreground.g, syn.foreground.b));
    if syn.font_style.contains(FontStyle::BOLD) {
        style = style.add_modifier(Modifier::BOLD);
    }
    if syn.font_style.contains(FontStyle::ITALIC) {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if syn.font_style.contains(FontStyle::UNDERLINE) {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    style
}

fn heading_style(level: HeadingLevel, theme: &Theme) -> Style {
    if theme.color_profile == ColorProfile::NoColor {
        return Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
    }
    let fg = match level {
        HeadingLevel::H1 => theme.accent_primary,
        HeadingLevel::H2 => theme.accent_secondary,
        _ => theme.accent_glow,
    };
    Style::default().fg(fg).add_modifier(Modifier::BOLD)
}

fn current_style(stack: &[Style], theme: &Theme) -> Style {
    stack
        .last()
        .copied()
        .unwrap_or_else(|| Style::default().fg(theme.text_primary))
}

fn flush_spans(spans: &mut Vec<Span<'static>>, lines: &mut Vec<Line<'static>>) {
    if !spans.is_empty() {
        lines.push(Line::from(core::mem::take(spans)));
    }
}

fn render_plain_wrapped(content: &str, width: u16, theme: &Theme) -> Vec<Line<'static>> {
    wrap_graphemes(content, usize::from(width.max(16)))
        .into_iter()
        .map(|line| Line::from(Span::styled(line, Style::default().fg(theme.text_primary))))
        .collect()
}

fn render_plain_lines(content: &str, theme: &Theme) -> Vec<Line<'static>> {
    content
        .split('\n')
        .map(|line| {
            Line::from(Span::styled(
                line.to_owned(),
                Style::default().fg(theme.text_primary),
            ))
        })
        .collect()
}

fn hash_str(text: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use ratatui::style::Color;

    use crate::config::GlyphSet;

    use super::*;

    #[test]
    fn highlights_markdown_toml_and_caches_by_width_and_theme() {
        let mut renderer = PreviewRenderer::new();
        let theme = Theme::build(
            ResolvedTheme::CyberNeon,
            ColorProfile::TrueColor,
            GlyphSet::NerdFont,
        );

        let md = "# 赛博规范\n\n- 禁止直推 `main`\n\n```toml\nschema_version = 1\n```\n";
        let first_len = renderer
            .render_cached(md, PreviewSyntax::Markdown, 60, &theme)
            .len();
        assert!(first_len >= 5);

        // Cache hit returns identical rendered slice.
        let cached_len = renderer
            .render_cached(md, PreviewSyntax::Markdown, 60, &theme)
            .len();
        assert_eq!(cached_len, first_len);

        let no_color = Theme::build(
            ResolvedTheme::CyberNeon,
            ColorProfile::NoColor,
            GlyphSet::Ascii,
        );
        let toml_lines = renderer.highlight_code(
            "schema_version = 1\n[project]\nname = \"demo\"\n",
            "toml",
            &no_color,
        );
        for line in toml_lines {
            for span in line.spans {
                assert!(matches!(span.style.fg, None | Some(Color::Reset)));
            }
        }
    }

    #[test]
    fn side_by_side_diff_highlights_inline_word_edits() {
        let theme = Theme::build(
            ResolvedTheme::CyberNeon,
            ColorProfile::TrueColor,
            GlyphSet::Unicode,
        );
        let rows = render_side_by_side_diff(
            "default_branch = \"master\"\n",
            "default_branch = \"main\"\n",
            &theme,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].old_line_no, Some(1));
        assert_eq!(rows[0].new_line_no, Some(1));
        assert!(rows[0].new_line.spans.len() >= 2);
    }
}
