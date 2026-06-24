//! "Did you mean?" suggestion helper for unknown-property diagnostics.
//!
//! [`check_unknown_props`] is the single shared helper that replaces the ~15
//! duplicated `unknown_props.keys()` loops scattered across the per-kind
//! `check_*` files. It computes an edit-distance suggestion from the node
//! kind's known-props list and emits one `node.unknown_property` Warning per
//! unknown entry with the appropriate message.

use std::collections::BTreeMap;

use crate::ast::Span;
use crate::ast::node::UnknownProperty;
use crate::diagnostics::Diagnostic;
use crate::parse::transform::known_props_for_kind;

// ---------------------------------------------------------------------------
// Edit-distance helper
// ---------------------------------------------------------------------------

/// Compute the Levenshtein distance between `a` and `b`, returning `Some(dist)`
/// if the distance is ≤ `max`, or `None` if it exceeds `max`.
///
/// Works on Unicode scalar values (via `chars()`). Uses a single-row DP with
/// no unchecked indexing and allocates at most `b.chars().count() + 1` values.
pub(super) fn edit_distance_within(a: &str, b: &str, max: usize) -> Option<usize> {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let la = a_chars.len();
    let lb = b_chars.len();

    // Fast path: length difference alone exceeds the budget.
    if la.abs_diff(lb) > max {
        return None;
    }

    // `row[j]` = edit distance between a[0..i] and b[0..j] at the start of
    // each outer iteration (i.e., after processing i characters of `a`).
    let mut row: Vec<usize> = (0..=lb).collect();

    for i in 1..=la {
        let mut prev = row[0]; // = dist(a[0..i-1], b[0..0])
        row[0] = i;
        let mut row_min = row[0];
        for j in 1..=lb {
            let old = row[j];
            let cost = usize::from(a_chars[i - 1] != b_chars[j - 1]);
            row[j] = (prev + cost)
                .min(old + 1) // deletion from a
                .min(row[j - 1] + 1); // insertion into a
            prev = old;
            if row[j] < row_min {
                row_min = row[j];
            }
        }
        // Early exit: if the minimum in this row already exceeds `max`, no
        // column in any subsequent row can be ≤ max.
        if row_min > max {
            return None;
        }
    }

    let dist = row[lb];
    if dist <= max { Some(dist) } else { None }
}

// ---------------------------------------------------------------------------
// Shared unknown-property check
// ---------------------------------------------------------------------------

/// Emit one `node.unknown_property` Warning for every entry in `unknown`.
///
/// For each unknown property name, the known-props list for `kind` is queried
/// via [`known_props_for_kind`] and the closest match within edit distance ≤ 2
/// is found. Ties are broken by lexicographic order: among equal-distance
/// candidates the lex-smallest name wins (see [`find_suggestion`]).
///
/// - Near-miss (distance ≤ 2): message contains `— did you mean '<suggestion>'?`
/// - No near-miss: the existing version-relative message is used verbatim.
///
/// Code is always `"node.unknown_property"`, severity always Warning.
pub(super) fn check_unknown_props(
    kind: &str,
    id: &str,
    unknown: &BTreeMap<String, UnknownProperty>,
    span: Option<Span>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let known = known_props_for_kind(kind);

    for prop_name in unknown.keys() {
        // Find the closest known prop within edit distance ≤ 2.
        // Deterministic tie-break: lexicographically smallest candidate wins
        // because we iterate `known` in slice order and only replace when
        // strictly less distance OR (same distance AND lex-smaller name).
        let suggestion = find_suggestion(prop_name, known);

        let message = match suggestion {
            Some(s) => format!(
                "{kind} '{id}': unknown property '{prop_name}' \
                 — did you mean '{s}'? \
                 (or a newer-schema property)"
            ),
            None => format!(
                "{kind} '{id}': unknown property '{prop_name}' (version-relative; \
                 may be valid in a later schema version)"
            ),
        };

        diagnostics.push(Diagnostic::warning(
            "node.unknown_property",
            message,
            span,
            Some(id.to_owned()),
        ));
    }
}

/// Find the lexicographically-smallest known prop within edit distance ≤ 2 of
/// `prop_name`, skipping any candidate that is identical to `prop_name` (exact
/// matches would have been collected as known props at parse time).
///
/// Returns `None` when no candidate is within the budget.
fn find_suggestion<'a>(prop_name: &str, known: &[&'a str]) -> Option<&'a str> {
    let mut best: Option<(&str, usize)> = None; // (candidate, distance)

    for &candidate in known {
        if candidate == prop_name {
            continue;
        }
        if let Some(dist) = edit_distance_within(prop_name, candidate, 2) {
            let replace = match best {
                None => true,
                Some((prev, prev_dist)) => {
                    dist < prev_dist || (dist == prev_dist && candidate < prev)
                }
            };
            if replace {
                best = Some((candidate, dist));
            }
        }
    }

    best.map(|(c, _)| c)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_distance_identical_strings() {
        assert_eq!(edit_distance_within("fill", "fill", 2), Some(0));
    }

    #[test]
    fn edit_distance_one_substitution() {
        // "fil" → "fill" is 1 insertion
        assert_eq!(edit_distance_within("fil", "fill", 2), Some(1));
    }

    #[test]
    fn edit_distance_two_substitutions() {
        // "gall" → "fill" substitutes g→f and a→i (the two trailing l's match)
        // for an edit distance of 2.
        assert_eq!(edit_distance_within("gall", "fill", 2), Some(2));
    }

    #[test]
    fn edit_distance_exceeds_max_returns_none() {
        assert_eq!(edit_distance_within("quantum_flux", "fill", 2), None);
    }

    #[test]
    fn edit_distance_empty_a() {
        // empty → "fill" = 4 insertions, exceeds 2
        assert_eq!(edit_distance_within("", "fill", 2), None);
    }

    #[test]
    fn edit_distance_empty_b() {
        // "fill" → empty = 4 deletions, exceeds 2
        assert_eq!(edit_distance_within("fill", "", 2), None);
    }

    #[test]
    fn find_suggestion_near_miss_fill() {
        let known: &[&str] = &["fill", "stroke", "x", "y", "w", "h"];
        // "fil" is 1 edit from "fill"
        assert_eq!(find_suggestion("fil", known), Some("fill"));
    }

    #[test]
    fn find_suggestion_far_miss_returns_none() {
        let known: &[&str] = &["fill", "stroke", "x", "y", "w", "h"];
        assert_eq!(find_suggestion("quantum_flux", known), None);
    }

    #[test]
    fn find_suggestion_skips_exact_match() {
        // If "fill" is in unknown props but also in known (shouldn't normally
        // happen, but the guard must not suggest the exact same name).
        let known: &[&str] = &["fill"];
        assert_eq!(find_suggestion("fill", known), None);
    }

    #[test]
    fn find_suggestion_tie_break_lexicographic() {
        // "ab" has distance 1 from "a" (insert b) and "ac" (substitute c→b).
        // Among equal-distance candidates, the lex-smallest wins: "a" < "ac".
        let known: &[&str] = &["ac", "a"];
        let result = find_suggestion("ab", known);
        // Both "a" and "ac" are at distance 1. "a" < "ac" lexicographically.
        assert_eq!(result, Some("a"));
    }
}
