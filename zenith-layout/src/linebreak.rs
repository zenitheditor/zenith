//! Line break opportunities inside a space-free word (UAX #14).
//!
//! `unicode-linebreak` finds the opportunities. Zenith keeps only the ones
//! next to an ideograph or a Japanese small kana, so CJK text wraps between
//! characters. Latin, Hangul, and other space-separated scripts break only at
//! spaces. Kinsoku follows from the UAX #14 classes: no break before closing
//! punctuation (`。，」`) and no break after opening punctuation (`「（`).

use unicode_linebreak::{BreakClass, BreakOpportunity, break_property, linebreaks};

/// Whether a char takes part in ideographic line breaking.
fn is_ideographic(ch: char) -> bool {
    matches!(
        break_property(u32::from(ch)),
        BreakClass::Ideographic | BreakClass::ConditionalJapaneseStarter
    )
}

/// The byte offsets inside `word` where a line can break.
///
/// Each offset `b` satisfies `0 < b < word.len()` and lies on a char boundary.
/// A break is kept only when the char before or after it is ideographic.
/// `word` holds no spaces: the caller already split on whitespace.
#[must_use]
pub fn ideographic_breaks(word: &str) -> Vec<usize> {
    if !word.chars().any(is_ideographic) {
        return Vec::new();
    }
    linebreaks(word)
        .filter(|&(offset, kind)| {
            kind == BreakOpportunity::Allowed && offset > 0 && offset < word.len()
        })
        .filter(|&(offset, _)| {
            let before = word.get(..offset).and_then(|s| s.chars().next_back());
            let after = word.get(offset..).and_then(|s| s.chars().next());
            before.is_some_and(is_ideographic) || after.is_some_and(is_ideographic)
        })
        .map(|(offset, _)| offset)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Split `word` at its break offsets.
    fn pieces(word: &str) -> Vec<&str> {
        let mut out = Vec::new();
        let mut start = 0;
        for b in ideographic_breaks(word) {
            out.push(&word[start..b]);
            start = b;
        }
        out.push(&word[start..]);
        out
    }

    #[test]
    fn latin_words_never_break() {
        assert!(ideographic_breaks("well-known").is_empty());
        assert!(ideographic_breaks("a/b").is_empty());
        assert!(ideographic_breaks("Zenith").is_empty());
    }

    #[test]
    fn chinese_breaks_between_ideographs() {
        assert_eq!(pieces("中文字"), vec!["中", "文", "字"]);
    }

    #[test]
    fn closing_punctuation_never_starts_a_piece() {
        assert_eq!(
            pieces("语言。汉字，好"),
            vec!["语", "言。", "汉", "字，", "好"]
        );
        assert_eq!(pieces("「但是」标点"), vec!["「但", "是」", "标", "点"]);
    }

    #[test]
    fn small_kana_stays_with_the_previous_char() {
        // The small kana ょ and っ never start a piece.
        let p = pieces("ちょっと");
        assert!(
            p.iter()
                .all(|s| !s.starts_with('ょ') && !s.starts_with('っ')),
            "{p:?}"
        );
    }

    #[test]
    fn hangul_breaks_only_at_spaces() {
        assert!(ideographic_breaks("한국어는").is_empty());
    }

    #[test]
    fn latin_next_to_ideographs_can_break() {
        assert_eq!(pieces("和English混排"), vec!["和", "English", "混", "排"]);
    }

    #[test]
    fn emoji_zwj_sequence_stays_whole() {
        assert!(ideographic_breaks("👨\u{200D}👩\u{200D}👧").is_empty());
    }
}
