//! The comment lines an edit dropped.

use crate::session::TextDelta;

/// The `//` comment lines the edit `delta` removed from `before`, trimmed,
/// in source order. A comment line the insert still holds is not counted.
///
/// Removing a node also removes the comment lines directly above it, so
/// the page can tell the user which comments went with it.
///
/// The delta is minimal, so it can start or end inside a line (two
/// comments that share a prefix). Both sides widen to whole lines first.
pub(crate) fn removed_comments(before: &str, delta: &TextDelta) -> Vec<String> {
    let start = before
        .get(..delta.start)
        .map_or(delta.start, |head| head.rfind('\n').map_or(0, |i| i + 1));
    let end = before.get(delta.end..).map_or(delta.end, |tail| {
        tail.find('\n').map_or(before.len(), |i| delta.end + i)
    });
    let (Some(removed), Some(lead), Some(trail)) = (
        before.get(start..end),
        before.get(start..delta.start),
        before.get(delta.end..end),
    ) else {
        return Vec::new();
    };
    let inserted = format!("{lead}{}{trail}", delta.insert);
    let mut kept: Vec<&str> = comment_lines(&inserted).collect();
    let mut out = Vec::new();
    for line in comment_lines(removed) {
        if let Some(i) = kept.iter().position(|k| *k == line) {
            kept.swap_remove(i);
        } else {
            out.push(line.to_owned());
        }
    }
    out
}

fn comment_lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .map(str::trim)
        .filter(|line| line.starts_with("//"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_only_comments_that_left() {
        let before = "a\n  // keep\n  // gone\n  rect r\n";
        let after = "a\n  // keep\n";
        let delta = TextDelta::between(before, after).expect("differs");
        assert_eq!(removed_comments(before, &delta), vec!["// gone".to_owned()]);
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
            vec!["// A thin rule.".to_owned()]
        );
    }
}
