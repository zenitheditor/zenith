//! Name ranking: edit distance, nearest-name suggestion, and the token-id
//! ranking rules used by hints and by `zenith fix`.

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
pub(crate) fn common_dotted_prefix(a: &str, b: &str) -> usize {
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

/// Truncated-segment match: drop trailing dot segments of `name` one at a
/// time and return the first (longest) prefix that is a candidate.
///
/// `color.primary.500` matches `color.primary`. A name without a dot never
/// matches. The result is unique by construction.
pub(crate) fn truncated_match<'a>(
    name: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    let candidates: Vec<&'a str> = candidates.into_iter().collect();
    let mut prefix = name;
    while let Some((head, _)) = prefix.rsplit_once('.') {
        if let Some(&hit) = candidates.iter().find(|c| **c == head) {
            return Some(hit);
        }
        prefix = head;
    }
    None
}

/// Rank a suggestion for an unknown token id.
///
/// Rule 1: [`truncated_match`]. Rule 2: [`find_suggestion`] at edit distance
/// ≤ 2, which already ends with its dotted-segment fallback.
pub(crate) fn find_token_suggestion<'a>(
    name: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    let candidates: Vec<&'a str> = candidates.into_iter().collect();
    truncated_match(name, candidates.iter().copied())
        .or_else(|| find_suggestion(name, candidates.iter().copied(), 2))
}

/// The single candidate at the smallest edit distance ≤ `max` from `name`.
///
/// Returns `None` when no candidate is within `max`, or when two or more
/// candidates share the smallest distance. Candidates equal to `name` are
/// skipped.
pub(crate) fn unique_nearest<'a>(
    name: &str,
    candidates: impl IntoIterator<Item = &'a str>,
    max: usize,
) -> Option<&'a str> {
    let mut best: Option<(&'a str, usize)> = None;
    let mut tied = false;
    for candidate in candidates {
        if candidate == name {
            continue;
        }
        let Some(dist) = edit_distance_within(name, candidate, max) else {
            continue;
        };
        match best {
            Some((prev, prev_dist)) if dist == prev_dist => {
                if prev != candidate {
                    tied = true;
                }
            }
            Some((_, prev_dist)) if dist > prev_dist => {}
            _ => {
                best = Some((candidate, dist));
                tied = false;
            }
        }
    }
    if tied { None } else { best.map(|(c, _)| c) }
}

/// The token id `zenith fix` swaps in for an unknown token reference.
///
/// Rule 1: [`truncated_match`]. Rule 2: [`unique_nearest`] at edit distance
/// ≤ 2. A tie under rule 2 returns `None`.
pub(crate) fn token_fix_target<'a>(
    name: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    let candidates: Vec<&'a str> = candidates.into_iter().collect();
    truncated_match(name, candidates.iter().copied())
        .or_else(|| unique_nearest(name, candidates.iter().copied(), 2))
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
    fn truncated_match_drops_trailing_segments() {
        let known = ["color.base.100", "color.primary", "color.primary.content"];
        assert_eq!(
            truncated_match("color.primary.500", known),
            Some("color.primary")
        );
        assert_eq!(
            truncated_match("color.primary.500.x", known),
            Some("color.primary")
        );
    }

    #[test]
    fn truncated_match_prefers_longest_prefix() {
        let known = ["color", "color.primary"];
        assert_eq!(
            truncated_match("color.primary.500", known),
            Some("color.primary")
        );
    }

    #[test]
    fn truncated_match_misses_without_declared_prefix() {
        let known = ["color.base.100", "color.base.200"];
        assert_eq!(truncated_match("color.base.900", known), None);
        assert_eq!(truncated_match("nodot", ["nodot"]), None);
    }

    #[test]
    fn token_suggestion_truncation_beats_edit_distance() {
        // Edit distance alone picks `color.base.100` (2 edits); the declared
        // prefix `color.primary` wins first.
        let known = ["color.base.100", "color.primary", "color.primary.content"];
        assert_eq!(
            find_token_suggestion("color.primary.500", known),
            Some("color.primary")
        );
    }

    #[test]
    fn token_suggestion_falls_back_to_edit_distance() {
        let known = ["color.base.100", "color.base.200"];
        assert_eq!(
            find_token_suggestion("color.base.900", known),
            Some("color.base.100"),
            "tie goes to the lexicographically smallest id"
        );
    }

    #[test]
    fn token_suggestion_falls_back_to_dotted_rule() {
        let known = ["color.base.content-primary"];
        assert_eq!(
            find_token_suggestion("color.base.contnt-primry", known),
            Some("color.base.content-primary")
        );
    }

    #[test]
    fn unique_nearest_rejects_ties() {
        assert_eq!(unique_nearest("ab", ["ac", "ad"], 2), None);
        assert_eq!(unique_nearest("ab", ["ac", "xyz"], 2), Some("ac"));
    }

    #[test]
    fn unique_nearest_prefers_strictly_closer() {
        assert_eq!(unique_nearest("abcd", ["abce", "abzz"], 2), Some("abce"));
        assert_eq!(unique_nearest("abcd", ["abzz", "abce"], 2), Some("abce"));
    }

    #[test]
    fn unique_nearest_ignores_duplicate_candidates() {
        assert_eq!(unique_nearest("ab", ["ac", "ac"], 2), Some("ac"));
    }

    #[test]
    fn token_fix_target_rules() {
        let known = ["color.base.100", "color.base.200", "color.primary"];
        assert_eq!(
            token_fix_target("color.primary.500", known),
            Some("color.primary")
        );
        assert_eq!(token_fix_target("color.base.900", known), None);
        assert_eq!(
            token_fix_target("color.primry", known),
            Some("color.primary")
        );
    }
}
