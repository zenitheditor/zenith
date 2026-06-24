//! "Did you mean?" suggestion helpers for `node.unknown_property` warnings.
//!
//! Exposes [`check_unknown_props`] — a single shared helper that replaces the
//! duplicated `for prop_name in …unknown_props.keys()` loops across all
//! per-kind check files. When a typo is close (Levenshtein distance ≤ 2) to a
//! known property name the diagnostic message includes a "did you mean?" hint;
//! otherwise the original version-relative wording is emitted verbatim so
//! forward-compat documents are not affected.

use std::collections::BTreeMap;

use crate::ast::Span;
use crate::ast::node::UnknownProperty;
use crate::diagnostics::Diagnostic;
use crate::parse::transform::known_props_for_kind;

// ---------------------------------------------------------------------------
// Levenshtein distance (bounded)
// ---------------------------------------------------------------------------

/// Compute the Levenshtein edit distance between `a` and `b`.
///
/// Returns `Some(distance)` if the distance is ≤ `max`, otherwise `None`.
/// Uses a single-row DP approach; iterates over `chars()` so multi-byte
/// codepoints are counted as one edit unit. No unchecked indexing.
fn edit_distance_within(a: &str, b: &str, max: usize) -> Option<usize> {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    // Fast path: if length difference alone exceeds max, skip the DP.
    if a_len.abs_diff(b_len) > max {
        return None;
    }

    // Single-row DP. `row[j]` = edit distance between a[0..i] and b[0..j].
    let mut row: Vec<usize> = (0..=b_len).collect();

    for i in 1..=a_len {
        let mut prev = i - 1; // row[j-1] from the previous outer iteration
        row[0] = i;
        for j in 1..=b_len {
            let old = row[j];
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            row[j] = (prev + cost).min(row[j] + 1).min(row[j - 1] + 1);
            prev = old;
        }
    }

    let dist = row[b_len];
    if dist <= max { Some(dist) } else { None }
}

// ---------------------------------------------------------------------------
// Suggestion finder
// ---------------------------------------------------------------------------

/// Find the lexicographically-smallest known property within edit distance ≤ 2
/// of `prop_name`. Returns `None` when no near-miss exists or when the only
/// candidate is an exact match (which would not be in `unknown_props` anyway).
fn find_suggestion<'a>(prop_name: &str, known: &[&'a str]) -> Option<&'a str> {
    let mut best: Option<(usize, &str)> = None; // (distance, name) — lex-sorted within distance

    for &candidate in known {
        // Skip exact matches (should never appear in unknown_props, but be safe).
        if candidate == prop_name {
            continue;
        }
        if let Some(dist) = edit_distance_within(prop_name, candidate, 2) {
            let is_better = match best {
                None => true,
                Some((best_dist, best_name)) => {
                    dist < best_dist || (dist == best_dist && candidate < best_name)
                }
            };
            if is_better {
                best = Some((dist, candidate));
            }
        }
    }

    best.map(|(_, name)| name)
}

// ---------------------------------------------------------------------------
// Public shared helper
// ---------------------------------------------------------------------------

/// Emit a `node.unknown_property` warning for every entry in `unknown`.
///
/// When a near-miss (edit distance ≤ 2) to a known property for `kind` exists,
/// the warning message includes a "did you mean?" hint. Otherwise the original
/// version-relative wording is emitted unchanged, so forward-compat documents
/// (using properties from a later schema version) are not misled.
///
/// The diagnostic code is always `"node.unknown_property"` and the severity is
/// always Warning — no new code or severity level is introduced.
pub(super) fn check_unknown_props(
    kind: &str,
    id: &str,
    unknown: &BTreeMap<String, UnknownProperty>,
    span: Option<Span>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let known = known_props_for_kind(kind);
    for prop_name in unknown.keys() {
        let msg = match find_suggestion(prop_name, known) {
            Some(suggestion) => format!(
                "{kind} '{id}': unknown property '{prop_name}' \
                 — did you mean '{suggestion}'? (or a newer-schema property)"
            ),
            None => format!(
                "{kind} '{id}': unknown property '{prop_name}' \
                 (version-relative; may be valid in a later schema version)"
            ),
        };
        diagnostics.push(Diagnostic::warning(
            "node.unknown_property",
            msg,
            span,
            Some(id.to_owned()),
        ));
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // edit_distance_within

    #[test]
    fn distance_identical_strings_is_zero() {
        assert_eq!(edit_distance_within("fill", "fill", 2), Some(0));
    }

    #[test]
    fn distance_one_deletion() {
        // "fil" → "fill": insert one 'l' = distance 1
        assert_eq!(edit_distance_within("fil", "fill", 2), Some(1));
    }

    #[test]
    fn distance_two_substitutions() {
        // "widht" → "width": the transposed 'h'/'t' cost two substitutions
        // under plain Levenshtein.
        assert_eq!(edit_distance_within("widht", "width", 2), Some(2));
    }

    #[test]
    fn distance_exceeds_max_returns_none() {
        // "quantum_flux" vs "fill": distance >> 2
        assert_eq!(edit_distance_within("quantum_flux", "fill", 2), None);
    }

    #[test]
    fn distance_length_diff_fast_path() {
        // Length diff > 2 → None without running DP
        assert_eq!(edit_distance_within("x", "opacity", 2), None);
    }

    // find_suggestion

    #[test]
    fn suggestion_typo_fil_finds_fill() {
        let known = &["fill", "stroke", "opacity", "visible"];
        assert_eq!(find_suggestion("fil", known), Some("fill"));
    }

    #[test]
    fn suggestion_far_miss_returns_none() {
        let known = &["fill", "stroke", "opacity", "visible"];
        assert_eq!(find_suggestion("quantum_flux", known), None);
    }

    #[test]
    fn suggestion_exact_match_skipped() {
        // If someone somehow has an exact name in known, it should NOT be suggested.
        let known = &["fill", "stroke"];
        assert_eq!(find_suggestion("fill", known), None);
    }

    #[test]
    fn suggestion_tie_breaks_lexicographically() {
        // "ab" is distance 1 from both "abc" and "abd"; "abc" < "abd" lexicographically.
        let known = &["abd", "abc"];
        assert_eq!(find_suggestion("ab", known), Some("abc"));
    }
}
