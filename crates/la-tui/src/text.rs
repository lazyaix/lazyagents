//! Grapheme-cluster Unicode/CJK/Emoji width, truncation, wrapping, and zero-dependency
//! permissive fuzzy matching utilities for `la-tui`.

use unicode_segmentation::UnicodeSegmentation as _;
use unicode_width::UnicodeWidthStr;

/// Scored fuzzy-match result carrying matched character indices for highlight rendering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FuzzyMatch {
    /// Higher score indicates a tighter / higher-priority match.
    pub score: i32,
    /// Character indices inside the candidate string that matched the query.
    pub matched_indices: Vec<usize>,
}

/// Computes the exact terminal cell display width of `text` using Unicode grapheme clusters.
#[must_use]
pub fn display_width(text: &str) -> usize {
    text.graphemes(true)
        .map(grapheme_width)
        .fold(0usize, usize::saturating_add)
}

/// Computes the terminal display width of a single extended grapheme cluster.
#[must_use]
pub fn grapheme_width(grapheme: &str) -> usize {
    if grapheme == "\t" {
        return 4;
    }
    if grapheme.chars().all(char::is_control) {
        return 0;
    }
    UnicodeWidthStr::width(grapheme)
}

/// Truncates `text` so its terminal display width never exceeds `max_width`, appending `ellipsis`
/// when truncation occurs while strictly preserving grapheme-cluster boundaries.
#[must_use]
pub fn truncate_with_ellipsis(text: &str, max_width: usize, ellipsis: &str) -> String {
    if max_width == 0 {
        return String::new();
    }
    if display_width(text) <= max_width {
        return text.to_owned();
    }

    let ellipsis_w = display_width(ellipsis);
    if ellipsis_w >= max_width {
        return take_prefix_by_width(ellipsis, max_width);
    }

    let budget = max_width.saturating_sub(ellipsis_w);
    let mut out = take_prefix_by_width(text, budget);
    out.push_str(ellipsis);
    out
}

/// Wraps `text` into display lines whose cell widths never exceed `max_width` (unless a single
/// wide grapheme exceeds `max_width` on a 1-column viewport).
#[must_use]
pub fn wrap_graphemes(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    for raw_line in text.split('\n') {
        if raw_line.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        let mut current_w = 0usize;

        for grapheme in raw_line.graphemes(true) {
            let gw = grapheme_width(grapheme);
            if current_w.saturating_add(gw) > max_width && !current.is_empty() {
                lines.push(current);
                current = String::new();
                current_w = 0;
            }
            current.push_str(grapheme);
            current_w = current_w.saturating_add(gw);
        }
        lines.push(current);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Pure-Rust, Unicode-safe subsequence fuzzy matcher for the `Ctrl+K` Command Palette and pickers.
///
/// Rewards consecutive runs, word-boundary starts (after space, `/`, `-`, `_`, `:`), prefix
/// matches, and shorter candidates. Returns `None` if any query character is missing in order.
#[must_use]
pub fn fuzzy_match(query: &str, candidate: &str) -> Option<FuzzyMatch> {
    let query_chars: Vec<char> = query
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    if query_chars.is_empty() {
        return Some(FuzzyMatch {
            score: 0,
            matched_indices: Vec::new(),
        });
    }

    let cand_chars: Vec<char> = candidate.chars().collect();
    let cand_lower: Vec<char> = cand_chars
        .iter()
        .map(|ch| ch.to_lowercase().next().unwrap_or(*ch))
        .collect();

    let mut matched_indices = Vec::with_capacity(query_chars.len());
    let mut score: i32 = 0;
    let mut q_idx = 0usize;
    let mut prev_match: Option<usize> = None;

    for (c_idx, &ch) in cand_lower.iter().enumerate() {
        let Some(&target) = query_chars.get(q_idx) else {
            break;
        };
        if ch == target {
            matched_indices.push(c_idx);
            score = score.saturating_add(10);

            if c_idx == 0 {
                score = score.saturating_add(18);
            } else if let Some(&prev_char) = cand_chars.get(c_idx.saturating_sub(1))
                && is_word_boundary(prev_char, cand_chars.get(c_idx).copied())
            {
                score = score.saturating_add(14);
            }

            if let Some(prev) = prev_match {
                if c_idx == prev.saturating_add(1) {
                    score = score.saturating_add(12);
                } else {
                    let gap = i32::try_from(c_idx.saturating_sub(prev).saturating_sub(1))
                        .unwrap_or(i32::MAX);
                    score = score.saturating_sub(gap.min(10));
                }
            }

            prev_match = Some(c_idx);
            q_idx = q_idx.saturating_add(1);
        }
    }

    if q_idx < query_chars.len() {
        return None;
    }

    let len_penalty = i32::try_from(cand_chars.len().saturating_sub(query_chars.len()))
        .unwrap_or(100)
        .min(40);
    score = score.saturating_sub(len_penalty / 2);

    Some(FuzzyMatch {
        score,
        matched_indices,
    })
}

fn take_prefix_by_width(text: &str, max_width: usize) -> String {
    let mut out = String::new();
    let mut used = 0usize;
    for grapheme in text.graphemes(true) {
        let gw = grapheme_width(grapheme);
        if used.saturating_add(gw) > max_width {
            break;
        }
        out.push_str(grapheme);
        used = used.saturating_add(gw);
    }
    out
}

fn is_word_boundary(prev: char, current: Option<char>) -> bool {
    matches!(prev, ' ' | '-' | '_' | '/' | ':' | '.')
        || (prev.is_lowercase() && current.is_some_and(char::is_uppercase))
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn handles_cjk_emoji_and_truncation_at_grapheme_boundaries() {
        let sample = "赛博配置🚀Studio";
        assert_eq!(display_width(sample), 8 + 2 + 6);

        let truncated = truncate_with_ellipsis(sample, 9, "…");
        assert!(display_width(&truncated) <= 9);
        assert!(truncated.ends_with('…'));

        let wrapped = wrap_graphemes("中文测试ABCD", 6);
        assert!(wrapped.iter().all(|line| display_width(line) <= 6));
        assert_eq!(wrapped.concat(), "中文测试ABCD");
    }

    #[test]
    fn fuzzy_matcher_ranks_boundary_and_consecutive_matches_higher() {
        let strong = fuzzy_match("ghau", "GitHub Audit").unwrap();
        let weak = fuzzy_match("ghau", "Generate Hook And Update").unwrap();
        assert!(strong.score > weak.score);

        let zh = fuzzy_match("审计", "GitHub 治理审计面板").unwrap();
        assert_eq!(zh.matched_indices.len(), 2);
        assert!(fuzzy_match("xyz", "GitHub Audit").is_none());
    }

    proptest! {
        #[test]
        fn truncated_string_never_exceeds_requested_width(
            input in ".*",
            max_width in 0usize..80,
        ) {
            let truncated = truncate_with_ellipsis(&input, max_width, "…");
            prop_assert!(display_width(&truncated) <= max_width);
        }

        #[test]
        fn wrap_graphemes_preserves_all_non_newline_content(
            input in "[a-zA-Z0-9 中文字🚀\n]{0,60}",
            max_width in 2usize..32,
        ) {
            let wrapped = wrap_graphemes(&input, max_width);
            prop_assert!(!wrapped.is_empty());
            for line in &wrapped {
                prop_assert!(display_width(line) <= max_width);
            }
            let expected: String = input.chars().filter(|&c| c != '\n').collect();
            prop_assert_eq!(wrapped.concat(), expected);
        }
    }
}
