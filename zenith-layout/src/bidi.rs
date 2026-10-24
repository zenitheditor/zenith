//! Bidirectional text (UAX #9) over Zenith-owned types.
//!
//! `unicode-bidi` resolves embedding levels. Callers see only `u8` levels and
//! [`TextDirection`]. An even level runs left to right. An odd level runs
//! right to left.

use unicode_bidi::{BidiClass, BidiInfo, Direction, Level, bidi_class, get_base_direction_full};

use crate::engine::TextDirection;

/// The direction of the first strong character in `text` (UAX #9 rules P2 and P3).
///
/// Returns `None` when `text` has no strong character.
#[must_use]
pub fn detect_direction(text: &str) -> Option<TextDirection> {
    match get_base_direction_full(text) {
        Direction::Ltr => Some(TextDirection::Ltr),
        Direction::Rtl => Some(TextDirection::Rtl),
        Direction::Mixed => None,
    }
}

/// The paragraph level of `base`: 0 for LTR, 1 for RTL.
#[must_use]
pub const fn base_level(base: TextDirection) -> u8 {
    match base {
        TextDirection::Ltr => 0,
        TextDirection::Rtl => 1,
    }
}

/// The direction a run at `level` shapes in.
#[must_use]
pub const fn level_direction(level: u8) -> TextDirection {
    if level.is_multiple_of(2) {
        TextDirection::Ltr
    } else {
        TextDirection::Rtl
    }
}

/// Whether `text` can resolve to a level other than the base level.
///
/// LTR text without RTL characters or explicit bidi controls stays at level 0.
/// The check skips the full algorithm for that common case.
fn needs_resolution(text: &str, base: TextDirection) -> bool {
    match base {
        TextDirection::Rtl => true,
        TextDirection::Ltr => text.chars().any(|ch| {
            matches!(
                bidi_class(ch),
                BidiClass::R
                    | BidiClass::AL
                    | BidiClass::AN
                    | BidiClass::RLE
                    | BidiClass::RLO
                    | BidiClass::RLI
                    | BidiClass::LRE
                    | BidiClass::LRO
                    | BidiClass::LRI
                    | BidiClass::FSI
                    | BidiClass::PDF
                    | BidiClass::PDI
            )
        }),
    }
}

/// The resolved embedding level of every byte of `text`, with `base` as the
/// paragraph direction.
///
/// The result has `text.len()` entries. All bytes of one char share a level.
/// Line-level rule L1 is not applied: callers do not shape whitespace.
#[must_use]
pub fn bidi_levels(text: &str, base: TextDirection) -> Vec<u8> {
    let base_lvl = base_level(base);
    if !needs_resolution(text, base) {
        return vec![base_lvl; text.len()];
    }
    let para_level = match base {
        TextDirection::Ltr => Level::ltr(),
        TextDirection::Rtl => Level::rtl(),
    };
    let info = BidiInfo::new(text, Some(para_level));
    info.levels.iter().map(Level::number).collect()
}

/// The byte ranges of `text` that share one level, in logical order.
///
/// `levels` holds one level per byte of `text`. A range never splits a char.
/// A missing level reads as `fallback`.
#[must_use]
pub fn level_runs(text: &str, levels: &[u8], fallback: u8) -> Vec<(usize, usize, u8)> {
    let mut runs: Vec<(usize, usize, u8)> = Vec::new();
    for (start, ch) in text.char_indices() {
        let level = levels.get(start).copied().unwrap_or(fallback);
        let end = start + ch.len_utf8();
        match runs.last_mut() {
            Some((_, last_end, last_level)) if *last_level == level => *last_end = end,
            _ => runs.push((start, end, level)),
        }
    }
    runs
}

/// The visual order of items with the given levels (UAX #9 rule L2).
///
/// Index `i` of the result is the logical index of the item drawn `i`-th from
/// the left. From the highest level down to the lowest odd level, every
/// maximal sequence at that level or higher reverses.
#[must_use]
pub fn visual_order(levels: &[u8]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..levels.len()).collect();
    let (Some(&max), Some(&min)) = (levels.iter().max(), levels.iter().min()) else {
        return order;
    };
    let lowest_odd = min | 1;
    let mut level = max;
    while level >= lowest_odd {
        let mut i = 0;
        while i < order.len() {
            let at_or_above = |idx: usize| {
                order
                    .get(idx)
                    .and_then(|&item| levels.get(item))
                    .is_some_and(|&l| l >= level)
            };
            if !at_or_above(i) {
                i += 1;
                continue;
            }
            let start = i;
            while at_or_above(i) {
                i += 1;
            }
            if let Some(slice) = order.get_mut(start..i) {
                slice.reverse();
            }
        }
        level -= 1;
    }
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_first_strong_direction() {
        assert_eq!(detect_direction("Hello"), Some(TextDirection::Ltr));
        assert_eq!(detect_direction("123 שלום"), Some(TextDirection::Rtl));
        assert_eq!(detect_direction("مرحبا world"), Some(TextDirection::Rtl));
        assert_eq!(detect_direction("2025!"), None);
        assert_eq!(detect_direction("中文"), Some(TextDirection::Ltr));
    }

    #[test]
    fn latin_text_skips_resolution() {
        assert_eq!(
            bidi_levels("Hello, world 42!", TextDirection::Ltr),
            vec![0; 16]
        );
    }

    #[test]
    fn hebrew_in_ltr_is_level_one() {
        let text = "a שלום b";
        let levels = bidi_levels(text, TextDirection::Ltr);
        let runs = level_runs(text, &levels, 0);
        assert_eq!(
            runs,
            vec![(0, 2, 0), (2, 10, 1), (10, 12, 0)],
            "the Hebrew word alone is RTL"
        );
    }

    #[test]
    fn numbers_in_rtl_are_level_two() {
        let text = "السعر 25.50 دولار";
        let levels = bidi_levels(text, TextDirection::Rtl);
        let start = text.find('2').expect("digit");
        let end = start + "25.50".len();
        assert!(levels[start..end].iter().all(|&l| l == 2), "{levels:?}");
        assert!(levels[..start - 1].iter().all(|&l| l == 1));
    }

    #[test]
    fn neutral_between_rtl_words_takes_rtl() {
        // The comma sits between two Hebrew words, so it resolves RTL.
        let text = "abc שלום, עולם def";
        let levels = bidi_levels(text, TextDirection::Ltr);
        let comma = text.find(',').expect("comma");
        assert_eq!(levels[comma], 1);
    }

    #[test]
    fn visual_order_reverses_odd_runs() {
        assert_eq!(visual_order(&[0, 0, 0]), vec![0, 1, 2]);
        assert_eq!(visual_order(&[1, 1, 1]), vec![2, 1, 0]);
        assert_eq!(visual_order(&[0, 1, 1, 0]), vec![0, 2, 1, 3]);
        // RTL paragraph with an LTR (level 2) pair inside.
        assert_eq!(visual_order(&[1, 2, 2, 1]), vec![3, 1, 2, 0]);
        assert_eq!(visual_order(&[]), Vec::<usize>::new());
    }

    #[test]
    fn level_direction_follows_parity() {
        assert_eq!(level_direction(0), TextDirection::Ltr);
        assert_eq!(level_direction(1), TextDirection::Rtl);
        assert_eq!(level_direction(2), TextDirection::Ltr);
    }
}
