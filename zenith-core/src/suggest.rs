//! Shared "did you mean?" helpers: edit distance, nearest-name suggestion,
//! and a bounded candidate list for diagnostics.
//!
//! Used by property-name checks (`node.unknown_property`), enum-value checks
//! (`node.invalid_value`), and token-reference checks
//! (`token.unknown_reference`, `token.raw_visual_literal`).

/// Compute the Levenshtein distance between `a` and `b`, returning `Some(dist)`
/// if the distance is ≤ `max`, or `None` if it exceeds `max`.
///
/// Works on Unicode scalar values (via `chars()`). Uses a single-row DP with
/// no unchecked indexing and allocates at most `b.chars().count() + 1` values.
pub(crate) fn edit_distance_within(a: &str, b: &str, max: usize) -> Option<usize> {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let la = a_chars.len();
    let lb = b_chars.len();

    // Fast path: length difference alone exceeds the budget.
    if la.abs_diff(lb) > max {
        return None;
    }

    // `row[j]` = edit distance between a[0..i] and b[0..j] after processing
    // i characters of `a`.
    let mut row: Vec<usize> = (0..=lb).collect();

    for (i, &ca) in a_chars.iter().enumerate() {
        let i = i + 1;
        // `prev` = dist(a[0..i-1], b[0..j-1]); `left` = row[j - 1] in this row.
        let mut prev = i - 1;
        let mut left = i;
        let mut row_min = i;
        for (cell, &cb) in row.iter_mut().skip(1).zip(&b_chars) {
            let old = *cell;
            let cost = usize::from(ca != cb);
            let value = (prev + cost)
                .min(old + 1) // deletion from a
                .min(left + 1); // insertion into a
            *cell = value;
            prev = old;
            left = value;
            row_min = row_min.min(value);
        }
        if let Some(first) = row.first_mut() {
            *first = i;
        }
        // Early exit: no later row can drop back to ≤ max.
        if row_min > max {
            return None;
        }
    }

    let dist = *row.last()?;
    if dist <= max { Some(dist) } else { None }
}

/// Number of leading dot-separated segments shared by `a` and `b`.
fn common_dotted_prefix(a: &str, b: &str) -> usize {
    a.split('.')
        .zip(b.split('.'))
        .take_while(|(x, y)| x == y)
        .count()
}

/// Last dot-separated segment of `s` (the whole string when it has no dot).
fn last_segment(s: &str) -> &str {
    s.rsplit('.').next().unwrap_or(s)
}

/// Find the closest candidate to `name`, skipping candidates equal to `name`.
///
/// Rule 1: the candidate with the smallest edit distance ≤ `max_distance`
/// wins; ties go to the lexicographically smallest candidate.
///
/// Rule 2 (dotted ids, only when rule 1 finds nothing): among candidates that
/// share at least one full leading dot segment with `name` and whose last
/// segment is within `max(2, last_segment_len / 3)` edits of `name`'s last
/// segment, the longest shared prefix wins, then the smaller last-segment
/// distance, then the lexicographically smallest candidate.
///
/// Returns `None` when neither rule matches.
pub(crate) fn find_suggestion<'a>(
    name: &str,
    candidates: impl IntoIterator<Item = &'a str>,
    max_distance: usize,
) -> Option<&'a str> {
    let candidates: Vec<&'a str> = candidates.into_iter().filter(|c| *c != name).collect();

    let mut best: Option<(&str, usize)> = None;
    for &candidate in &candidates {
        if let Some(dist) = edit_distance_within(name, candidate, max_distance) {
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
    if let Some((c, _)) = best {
        return Some(c);
    }

    // Dotted-segment fallback (rule 2).
    let name_last = last_segment(name);
    let budget = 2usize.max(name_last.chars().count() / 3);
    let mut best: Option<(&str, usize, usize)> = None; // (candidate, prefix, dist)
    for &candidate in &candidates {
        let prefix = common_dotted_prefix(name, candidate);
        if prefix == 0 {
            continue;
        }
        let Some(dist) = edit_distance_within(name_last, last_segment(candidate), budget) else {
            continue;
        };
        let replace = match best {
            None => true,
            Some((prev, prev_prefix, prev_dist)) => {
                prefix > prev_prefix
                    || (prefix == prev_prefix
                        && (dist < prev_dist || (dist == prev_dist && candidate < prev)))
            }
        };
        if replace {
            best = Some((candidate, prefix, dist));
        }
    }
    best.map(|(c, _, _)| c)
}

/// Message for a known property whose enum value is not allowed.
///
/// `subject` names the owner (for example `polygon 'p1'`). The message names
/// the property, the bad value, a did-you-mean when `value` is within edit
/// distance ≤ 2 of an allowed value, and every allowed value.
pub(crate) fn invalid_value_message(
    subject: &str,
    prop: &str,
    value: &str,
    allowed: &[&str],
) -> String {
    let list = allowed.join(", ");
    match find_suggestion(value, allowed.iter().copied(), 2) {
        Some(s) => format!(
            "{subject}: invalid {prop} '{value}' — did you mean '{s}'? Allowed values: {list}"
        ),
        None => format!("{subject}: invalid {prop} '{value}' — allowed values: {list}"),
    }
}

/// Message for an unknown property name on `kind`.
///
/// `subject` names the owner (for example `asset 'a1'`). The message suggests
/// the closest entry of `known` within edit distance ≤ 2, otherwise it points
/// at `zenith schema`.
pub(crate) fn unknown_property_message(
    subject: &str,
    kind: &str,
    name: &str,
    known: &[&str],
) -> String {
    match find_suggestion(name, known.iter().copied(), 2) {
        Some(s) => format!("{subject}: unknown property '{name}' — did you mean '{s}'?"),
        None => format!(
            "{subject}: unknown property '{name}' \
             — remove it or run `zenith schema` to list the {kind} properties"
        ),
    }
}

/// Maximum number of names listed by [`format_candidate_list`].
const MAX_LISTED: usize = 8;

/// Format a bounded candidate list for a diagnostic.
///
/// `label` names the kind of candidate (for example `"color tokens"`).
/// Returns `declared <label>: a, b, … (+N more)` with the first 8 names in
/// sorted order, or `no <label> declared` when `names` is empty.
pub(crate) fn format_candidate_list<'a>(
    label: &str,
    names: impl IntoIterator<Item = &'a str>,
) -> String {
    let mut sorted: Vec<&str> = names.into_iter().collect();
    sorted.sort_unstable();
    sorted.dedup();
    if sorted.is_empty() {
        return format!("no {label} declared");
    }
    let total = sorted.len();
    let shown = sorted
        .iter()
        .take(MAX_LISTED)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    if total > MAX_LISTED {
        format!("declared {label}: {shown} (+{} more)", total - MAX_LISTED)
    } else {
        format!("declared {label}: {shown}")
    }
}

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
        assert_eq!(edit_distance_within("gall", "fill", 2), Some(2));
    }

    #[test]
    fn edit_distance_exceeds_max_returns_none() {
        assert_eq!(edit_distance_within("quantum_flux", "fill", 2), None);
    }

    #[test]
    fn edit_distance_empty_a() {
        assert_eq!(edit_distance_within("", "fill", 2), None);
    }

    #[test]
    fn edit_distance_empty_b() {
        assert_eq!(edit_distance_within("fill", "", 2), None);
    }

    #[test]
    fn find_suggestion_near_miss_fill() {
        let known = ["fill", "stroke", "x", "y", "w", "h"];
        assert_eq!(find_suggestion("fil", known, 2), Some("fill"));
    }

    #[test]
    fn find_suggestion_far_miss_returns_none() {
        let known = ["fill", "stroke", "x", "y", "w", "h"];
        assert_eq!(find_suggestion("quantum_flux", known, 2), None);
    }

    #[test]
    fn find_suggestion_skips_exact_match() {
        assert_eq!(find_suggestion("fill", ["fill"], 2), None);
    }

    #[test]
    fn find_suggestion_tie_break_lexicographic() {
        // "ab" is 1 edit from both "a" and "ac"; "a" is lex-smaller.
        assert_eq!(find_suggestion("ab", ["ac", "a"], 1), Some("a"));
    }

    #[test]
    fn dotted_fallback_prefers_longest_shared_prefix() {
        let known = ["color.base.content", "color.brand.contentx", "size.base"];
        // Full distance is too large; shares `color.base` and last segments
        // `contents` / `content` are 1 edit apart.
        assert_eq!(
            find_suggestion("color.base.contents.x", known, 0),
            None,
            "segment count differs only in the tail; no last-segment match"
        );
        assert_eq!(
            find_suggestion("color.base.cntent", known, 0),
            Some("color.base.content")
        );
    }

    #[test]
    fn dotted_fallback_requires_shared_segment() {
        assert_eq!(find_suggestion("a.content", ["b.content"], 0), None);
    }

    #[test]
    fn candidate_list_empty() {
        assert_eq!(
            format_candidate_list("color tokens", []),
            "no color tokens declared"
        );
    }

    #[test]
    fn candidate_list_truncates_with_more_count() {
        let names: Vec<String> = (0..11).map(|i| format!("t{i:02}")).collect();
        let out = format_candidate_list("color tokens", names.iter().map(String::as_str));
        assert_eq!(
            out,
            "declared color tokens: t00, t01, t02, t03, t04, t05, t06, t07 (+3 more)"
        );
    }
}
