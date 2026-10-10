//! The comments an edit dropped, each with its line.

use std::collections::BTreeMap;

use super::lex::{Comment, comments};
use crate::session::TextDelta;

/// The comments of `before` that the edit `delta` dropped, in source
/// order: every `//`, `/* … */`, and `/-` comment of `before` that `after`
/// (`before` with `delta` applied) no longer holds.
///
/// The texts are compared as multisets of comment text, so a comment the
/// edit moves (a reordered node carries its comments) is not dropped. When
/// a comment text occurs more often in `before` than in `after`, the
/// copies inside the replaced range count as dropped first.
///
/// Each entry is `line N: <comment>`, `N` its 1-based line in `before`.
/// A comment that shares its line with other text adds that line:
/// `line N: <comment> (in: <line>)`.
pub(crate) fn removed_comments(before: &str, delta: &TextDelta) -> Vec<String> {
    let Ok(after) = delta.apply(before) else {
        return Vec::new();
    };
    let found = comments(before);
    if found.is_empty() {
        return Vec::new();
    }
    let mut missing: BTreeMap<&str, usize> = BTreeMap::new();
    for c in &found {
        if let Some(t) = before.get(c.start..c.end) {
            *missing.entry(t).or_insert(0) += 1;
        }
    }
    for c in comments(&after) {
        if let Some(n) = after.get(c.start..c.end).and_then(|t| missing.get_mut(t)) {
            *n = n.saturating_sub(1);
        }
    }
    if missing.values().all(|n| *n == 0) {
        return Vec::new();
    }
    let touched = |c: &Comment| c.start < delta.end.max(delta.start + 1) && c.end > delta.start;
    let mut dropped: Vec<&Comment> = Vec::new();
    for pass_touched in [true, false] {
        for c in found.iter().filter(|c| touched(c) == pass_touched) {
            if let Some(n) = before.get(c.start..c.end).and_then(|t| missing.get_mut(t))
                && *n > 0
            {
                *n -= 1;
                dropped.push(c);
            }
        }
    }
    dropped.sort_by_key(|c| c.start);
    dropped.into_iter().map(|c| describe(before, c)).collect()
}

/// `line N: <comment>`, with the line when other text shares it.
fn describe(text: &str, c: &Comment) -> String {
    let body = text.get(c.start..c.end).unwrap_or_default();
    let head = text.get(..c.start).unwrap_or_default();
    let line_no = head.bytes().filter(|&b| b == b'\n').count() + 1;
    let line_start = head.rfind('\n').map_or(0, |i| i + 1);
    let line_end = text
        .get(c.start..)
        .and_then(|rest| rest.find('\n'))
        .map_or(text.len(), |i| c.start + i);
    let line = text.get(line_start..line_end).unwrap_or_default().trim();
    let first = body.lines().next().unwrap_or_default().trim();
    if line == first {
        format!("line {line_no}: {body}")
    } else {
        format!("line {line_no}: {body} (in: {line})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_only_comments_that_left() {
        let before = "a\n  // keep\n  // gone\n  rect r\n";
        let after = "a\n  // keep\n";
        let delta = TextDelta::between(before, after).expect("differs");
        assert_eq!(
            removed_comments(before, &delta),
            vec!["line 3: // gone".to_owned()]
        );
        let moved = TextDelta {
            start: 0,
            end: before.len(),
            insert: "// gone\n// keep\n".to_owned(),
        };
        assert!(removed_comments(before, &moved).is_empty());
    }

    #[test]
    fn widens_a_delta_that_starts_inside_a_comment() {
        let before = "  // A thin rule.
  line l
  // A row.
  frame f
";
        let after = "  // A row.
  frame f
";
        let delta = TextDelta::between(before, after).expect("differs");
        assert!(delta.start > 0, "the minimal delta starts inside the line");
        assert_eq!(
            removed_comments(before, &delta),
            vec!["line 1: // A thin rule.".to_owned()]
        );
    }

    #[test]
    fn reports_inline_block_and_slashdash_comments_with_their_line() {
        let before = "a\nrect x= /* unit */ (px)10 // note\n/-rect id=\"old\"\nb\n";
        let after = "a\nb\n";
        let delta = TextDelta::between(before, after).expect("differs");
        assert_eq!(
            removed_comments(before, &delta),
            vec![
                "line 2: /* unit */ (in: rect x= /* unit */ (px)10 // note)".to_owned(),
                "line 2: // note (in: rect x= /* unit */ (px)10 // note)".to_owned(),
                "line 3: /-rect id=\"old\"".to_owned(),
            ]
        );
    }

    #[test]
    fn a_duplicate_text_drops_the_copy_in_the_replaced_range() {
        let before = "// same\nx\n// same\ny\n";
        let after = "// same\nx\n";
        let delta = TextDelta::between(before, after).expect("differs");
        assert_eq!(
            removed_comments(before, &delta),
            vec!["line 3: // same".to_owned()]
        );
    }
}
