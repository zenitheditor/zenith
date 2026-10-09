//! Remove source children: whole lines, attached comments, and one
//! redundant blank line.

use kdl::KdlNode;

use super::super::error::PatchError;
use super::super::text::{
    Edit, attach_start, is_blank, layout_error, line_before, line_from, line_start, slice,
    starts_line,
};
use super::differ::Differ;
use super::nodes::{block_braces, closing_line_end, entries_end, node_range};

impl Differ<'_> {
    /// Source range of `u` as whole lines: from its first attached comment
    /// line ([`attach_start`]) to past the `\n` that ends the node, which
    /// takes the node's trailing same-line comment along.
    pub(super) fn node_lines_range(&self, u: &KdlNode) -> Result<(usize, usize), PatchError> {
        let src = self.texts.src;
        let (s, e) = node_range(u);
        if !starts_line(src, s) {
            return Err(layout_error(format!(
                "node `{}` shares its line with another node",
                u.name().value()
            )));
        }
        let end = closing_line_end(src, e)?;
        Ok((attach_start(src, line_start(src, s)), end))
    }

    /// Remove the source nodes `gone` from one sibling list.
    ///
    /// Neighbouring ranges, and ranges with only blank lines between them,
    /// merge into one. Each merged range then drops one adjacent blank line
    /// when the removal would leave two blank lines together, or a blank
    /// line right inside the block's `{` or `}`. When `emptied` is set and
    /// the block holds nothing else, the block collapses to the canonical
    /// form of the after `parent`: `{}`, or no block.
    pub(super) fn remove_nodes(
        &mut self,
        parent: Option<(&KdlNode, &KdlNode)>,
        gone: &[&KdlNode],
        emptied: bool,
    ) -> Result<(), PatchError> {
        if gone.is_empty() {
            return Ok(());
        }
        let src = self.texts.src;
        let mut ranges = gone
            .iter()
            .map(|u| self.node_lines_range(u))
            .collect::<Result<Vec<_>, _>>()?;
        ranges.sort_unstable();
        let merged = merge(src, &ranges);
        if emptied
            && let Some((u, a)) = parent
            && let Some(edit) = collapse(src, u, a, &merged)?
        {
            self.removed.push((edit.start, edit.end));
            self.edits.push(edit);
            return Ok(());
        }
        for range in merged {
            let (s, e) = tidy(src, range);
            self.removed.push((s, e));
            self.edits.push(Edit::replace(s, e, ""));
        }
        Ok(())
    }
}

/// Merge sorted line ranges that touch, overlap, or have only blank lines
/// between them.
fn merge(src: &str, ranges: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::with_capacity(ranges.len());
    for &(s, e) in ranges {
        if let Some(last) = out.last_mut()
            && (s <= last.1 || src.get(last.1..s).is_some_and(|gap| gap.trim().is_empty()))
        {
            last.1 = last.1.max(e);
            continue;
        }
        out.push((s, e));
    }
    out
}

/// Widen a removed line range by one blank line when the removal would
/// leave a doubled blank line or a blank line next to a brace line.
fn tidy(src: &str, (s, e): (usize, usize)) -> (usize, usize) {
    let prev = line_before(src, s);
    let next = line_from(src, e);
    let prev_blank = prev.is_some_and(|(_, t)| is_blank(t));
    let prev_open = prev.is_none_or(|(_, t)| t.trim_end().ends_with('{'));
    let next_blank = next.is_some_and(|(_, t)| is_blank(t));
    let next_close = next.is_none_or(|(_, t)| t.trim_start().starts_with('}'));
    match (prev, next) {
        (_, Some((end, _))) if next_blank && (prev_blank || prev_open) => (s, end),
        (Some((start, _)), _) if prev_blank && next_close => (start, e),
        _ => (s, e),
    }
}

/// The edit that empties `u`'s child block when the removed `ranges` are
/// all it holds besides whitespace. `None` when other text stays.
fn collapse(
    src: &str,
    u: &KdlNode,
    a: &KdlNode,
    ranges: &[(usize, usize)],
) -> Result<Option<Edit>, PatchError> {
    let Ok((open, close)) = block_braces(src, u) else {
        return Ok(None);
    };
    let mut cursor = open + 1;
    for &(s, e) in ranges {
        if s < cursor || e > close || !is_blank(slice(src, cursor, s)?) {
            return Ok(None);
        }
        cursor = e;
    }
    if !is_blank(slice(src, cursor, close)?) {
        return Ok(None);
    }
    let text = if a.children().is_some() { " {}" } else { "" };
    Ok(Some(Edit::replace(entries_end(u), close + 1, text)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_joins_ranges_across_blank_lines() {
        let src = "a\nb\n\nc\nd\n";
        assert_eq!(merge(src, &[(2, 4), (5, 7)]), vec![(2, 7)]);
        assert_eq!(merge(src, &[(0, 2), (5, 7)]), vec![(0, 2), (5, 7)]);
    }

    #[test]
    fn tidy_drops_a_doubled_blank_line() {
        let src = "a\n\nx\n\nb\n";
        assert_eq!(tidy(src, (3, 5)), (3, 6));
    }

    #[test]
    fn tidy_drops_a_blank_line_before_the_closer() {
        let src = "a {\n  b\n\n  x\n}\n";
        let x = src.find("  x").expect("x");
        assert_eq!(tidy(src, (x, x + 4)), (x - 1, x + 4));
    }

    #[test]
    fn tidy_drops_a_blank_line_after_the_opener() {
        let src = "a {\n  x\n\n  b\n}\n";
        assert_eq!(tidy(src, (4, 8)), (4, 9));
    }

    #[test]
    fn tidy_keeps_single_separators() {
        let src = "a\n\nx\nb\n";
        assert_eq!(tidy(src, (3, 5)), (3, 5));
    }
}
