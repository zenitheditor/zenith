//! Insert new and moved children at their after position.
//!
//! Insertion-position rule: a run of new children goes on new lines right
//! after the source line that ends the preceding staying sibling (past its
//! trailing comment). With no preceding sibling it goes before the next
//! staying sibling's attached comments. With no staying sibling it goes
//! into the parent block. A new child takes its canonical text re-indented
//! to the site. A moved child takes its own source text with its attached
//! comments, patched for its own changes and re-indented.

use kdl::KdlNode;

use super::super::error::PatchError;
use super::super::text::{
    Edit, apply_edits, attach_start, indent_at, is_blank, layout_error, line_start, move_lines,
    reindent, slice, starts_line,
};
use super::differ::{Differ, List, Triple};
use super::key::{Key, find_source, index_error};
use super::nodes::{block_braces, children_of, closing_line_end, entries_end, node_range};

/// One child to insert.
#[derive(Clone, Copy)]
enum Item<'n> {
    /// A canonical after child with no source counterpart in this list.
    New(&'n KdlNode),
    /// A child that moves within its list.
    Moved(Triple<'n>),
}

impl Differ<'_> {
    /// Insert each run of after children that do not stay in place.
    ///
    /// `paired[j]` is the before index of after child `j` in this list.
    /// `a_to_b[j]` keeps only the children that stay in place.
    pub(super) fn insert_runs(
        &mut self,
        list: List<'_>,
        bk: &[(Key, usize)],
        paired: &[Option<usize>],
        a_to_b: &[Option<usize>],
    ) -> Result<(), PatchError> {
        let staying = |j: usize| a_to_b.get(j).copied().flatten();
        let source = |i: usize| -> Result<&KdlNode, PatchError> {
            let (key, occ) = bk.get(i).ok_or_else(|| index_error(i))?;
            find_source(list.un, key, *occ, bk)
        };
        let mut j = 0;
        while j < list.an.len() {
            if staying(j).is_some() {
                j += 1;
                continue;
            }
            let start = j;
            let mut items = Vec::new();
            while j < list.an.len() && staying(j).is_none() {
                let a = list.an.get(j).ok_or_else(|| index_error(j))?;
                items.push(match paired.get(j).copied().flatten() {
                    Some(i) => Item::Moved(Triple {
                        u: source(i)?,
                        b: list.bn.get(i).ok_or_else(|| index_error(i))?,
                        a,
                    }),
                    None => Item::New(a),
                });
                j += 1;
            }
            let prev = start.checked_sub(1).and_then(staying);
            match (prev, staying(j)) {
                (Some(i), _) => self.insert_after(source(i)?, &items)?,
                (None, Some(i)) => self.insert_before(source(i)?, &items)?,
                (None, None) => self.insert_into(list.parent.map(|p| p.0), list.un, &items)?,
            }
        }
        Ok(())
    }

    /// `at`, or the start of the removed range that holds it.
    fn place(&self, at: usize) -> usize {
        self.removed
            .iter()
            .find(|(s, e)| *s < at && at < *e)
            .map_or(at, |(s, _)| *s)
    }

    /// Insert `items` on new lines after source node `u`.
    fn insert_after(&mut self, u: &KdlNode, items: &[Item<'_>]) -> Result<(), PatchError> {
        let src = self.texts.src;
        let (s, e) = node_range(u);
        if !starts_line(src, s) {
            return Err(layout_error(
                "the sibling shares its line with another node",
            ));
        }
        let at = self.place(closing_line_end(src, e)?);
        let text = self.render_all(items, indent_at(src, s))?;
        self.edits.push(Edit::insert(at, text));
        Ok(())
    }

    /// Insert `items` on new lines before source node `u` and its attached
    /// comments.
    fn insert_before(&mut self, u: &KdlNode, items: &[Item<'_>]) -> Result<(), PatchError> {
        let src = self.texts.src;
        let (s, _) = node_range(u);
        if !starts_line(src, s) {
            return Err(layout_error(
                "the sibling shares its line with another node",
            ));
        }
        let at = self.place(attach_start(src, line_start(src, s)));
        let text = self.render_all(items, indent_at(src, s))?;
        self.edits.push(Edit::insert(at, text));
        Ok(())
    }

    /// Insert `items` into `parent`, none of whose source children stay.
    fn insert_into(
        &mut self,
        parent: Option<&KdlNode>,
        un: &[KdlNode],
        items: &[Item<'_>],
    ) -> Result<(), PatchError> {
        let parent = parent.ok_or_else(|| layout_error("new top-level node"))?;
        let src = self.texts.src;
        let eol = self.layout.eol;
        if let Some(first) = un.first() {
            // Every source child goes: the new children take the first one's place.
            let (s, _) = node_range(first);
            if !starts_line(src, s) {
                return Err(layout_error("the child shares its line with another node"));
            }
            let at = self.place(attach_start(src, line_start(src, s)));
            let text = self.render_all(items, indent_at(src, s))?;
            self.edits.push(Edit::insert(at, text));
            return Ok(());
        }
        let (ps, _) = node_range(parent);
        let indent = indent_at(src, ps).to_owned();
        let child_indent = format!("{indent}{}", self.layout.unit);
        let body = self.render_all(items, &child_indent)?;
        if parent.children().is_none() {
            let block = format!(" {{{eol}{body}{indent}}}");
            self.edits.push(Edit::insert(entries_end(parent), block));
            return Ok(());
        }
        let (open, close) = block_braces(src, parent)?;
        if is_blank(slice(src, open + 1, close)?) {
            let block = format!("{{{eol}{body}{indent}}}");
            self.edits.push(Edit::replace(open, close + 1, block));
            return Ok(());
        }
        // The block holds only comments: the children go above the `}` line.
        if !starts_line(src, close) {
            return Err(layout_error(
                "the child block holds comments and closes on a text line",
            ));
        }
        self.edits.push(Edit::insert(line_start(src, close), body));
        Ok(())
    }

    fn render_all(&self, items: &[Item<'_>], indent: &str) -> Result<String, PatchError> {
        let mut out = String::new();
        for item in items {
            out.push_str(&match item {
                Item::New(a) => self.render_new(a, indent)?,
                Item::Moved(t) => self.moved_text(*t, indent)?,
            });
        }
        Ok(out)
    }

    /// Whole lines at `indent` for after node `a`, which has no counterpart
    /// in its source list. A node that exists once elsewhere in the source
    /// moves here. A new node that holds moved nodes keeps their source
    /// text inside its canonical head.
    fn render_new(&self, a: &KdlNode, indent: &str) -> Result<String, PatchError> {
        if let Some((u, b)) = self.index.moved(a) {
            return self.moved_text(Triple { u, b, a }, indent);
        }
        let eol = self.layout.eol;
        let unit = self.layout.unit.as_str();
        let (start, end) = node_range(a);
        let base = indent_at(self.texts.after, start);
        if !self.holds_moved(a) {
            let text = self.after_text((start, end))?.trim_end();
            let body = reindent(text, base, indent, unit, eol)?;
            return Ok(format!("{indent}{body}{eol}"));
        }
        let head = self.after_text((start, entries_end(a)))?;
        let head = reindent(head, base, indent, unit, eol)?;
        let child_indent = format!("{indent}{unit}");
        let mut out = format!("{indent}{head} {{{eol}");
        for child in children_of(a) {
            out.push_str(&self.render_new(child, &child_indent)?);
        }
        out.push_str(&format!("{indent}}}{eol}"));
        Ok(out)
    }

    /// `true` when a descendant of new node `a` moves here from elsewhere.
    fn holds_moved(&self, a: &KdlNode) -> bool {
        children_of(a)
            .iter()
            .any(|c| self.index.moved(c).is_some() || self.holds_moved(c))
    }

    /// The source lines of moved node `t.u` with its attached comments,
    /// patched to `t.a` and re-indented to `indent`.
    fn moved_text(&self, t: Triple<'_>, indent: &str) -> Result<String, PatchError> {
        let src = self.texts.src;
        let (start, end) = self.node_lines_range(t.u)?;
        let mut inner = self.nested();
        inner.diff_node(t)?;
        let mut local = Vec::with_capacity(inner.edits.len());
        for edit in inner.edits {
            if edit.start < start || edit.end > end {
                return Err(layout_error(
                    "an edit inside a moved node reaches outside its lines",
                ));
            }
            local.push(Edit::replace(
                edit.start - start,
                edit.end - start,
                edit.text,
            ));
        }
        let text = apply_edits(slice(src, start, end)?, local)?;
        let from = indent_at(src, node_range(t.u).0);
        if from == indent {
            return Ok(text);
        }
        move_lines(&text, from, indent, self.layout.eol)
    }
}
