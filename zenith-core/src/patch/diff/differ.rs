//! The differ: aligns source, canonical before, and canonical after nodes
//! and records the source edits.

use std::collections::{BTreeMap, BTreeSet};

use kdl::KdlNode;

use super::super::error::{PatchError, PatchErrorCode};
use super::super::text::{Edit, slice};
use super::key::{Index, Key, find_source, index_error, keyed};
use super::nodes::{children_of, node_range};
use super::order::kept_in_place;

/// The three texts the diff reads.
#[derive(Debug, Clone, Copy)]
pub(in crate::patch) struct Texts<'a> {
    /// The user's source text.
    pub(in crate::patch) src: &'a str,
    /// Canonical text of the before document.
    pub(in crate::patch) before: &'a str,
    /// Canonical text of the after document.
    pub(in crate::patch) after: &'a str,
}

/// How new lines look in the source.
#[derive(Debug, Clone)]
pub(in crate::patch) struct Layout {
    /// `\n` or `\r\n`.
    pub(in crate::patch) eol: &'static str,
    /// One indentation step.
    pub(in crate::patch) unit: String,
}

/// A source node and its canonical counterparts, aligned.
#[derive(Clone, Copy)]
pub(super) struct Triple<'n> {
    /// The source node.
    pub(super) u: &'n KdlNode,
    /// The canonical before node.
    pub(super) b: &'n KdlNode,
    /// The canonical after node.
    pub(super) a: &'n KdlNode,
}

/// One sibling list to diff: source, canonical before, and canonical after
/// children, plus the source and after parent (`None` at the top level).
#[derive(Clone, Copy)]
pub(super) struct List<'n> {
    pub(super) parent: Option<(&'n KdlNode, &'n KdlNode)>,
    pub(super) un: &'n [KdlNode],
    pub(super) bn: &'n [KdlNode],
    pub(super) an: &'n [KdlNode],
}

/// Collects edits while walking the three trees.
pub(in crate::patch) struct Differ<'a> {
    pub(in crate::patch) texts: Texts<'a>,
    pub(in crate::patch) layout: Layout,
    pub(super) index: &'a Index<'a>,
    pub(in crate::patch) edits: Vec<Edit>,
    /// Source ranges this differ removes. Insertions never land inside one.
    pub(super) removed: Vec<(usize, usize)>,
}

impl<'a> Differ<'a> {
    pub(in crate::patch) fn new(texts: Texts<'a>, layout: Layout, index: &'a Index<'a>) -> Self {
        Self {
            texts,
            layout,
            index,
            edits: Vec::new(),
            removed: Vec::new(),
        }
    }

    /// A differ over the same texts with no edits yet.
    pub(super) fn nested(&self) -> Self {
        Self::new(self.texts, self.layout.clone(), self.index)
    }

    /// Canonical before text in byte range `r`.
    pub(in crate::patch) fn before_text(&self, r: (usize, usize)) -> Result<&'a str, PatchError> {
        slice(self.texts.before, r.0, r.1)
    }

    /// Canonical after text in byte range `r`.
    pub(in crate::patch) fn after_text(&self, r: (usize, usize)) -> Result<&'a str, PatchError> {
        slice(self.texts.after, r.0, r.1)
    }

    /// `true` when the canonical before and after texts of `b` and `a` match.
    fn unchanged(&self, b: &KdlNode, a: &KdlNode) -> Result<bool, PatchError> {
        Ok(self.before_text(node_range(b))? == self.after_text(node_range(a))?)
    }

    /// Diff one aligned node triple.
    pub(super) fn diff_node(&mut self, t: Triple<'_>) -> Result<(), PatchError> {
        let Triple { u, b, a } = t;
        if self.unchanged(b, a)? {
            return Ok(());
        }
        if b.name().value() != a.name().value() || u.name().value() != b.name().value() {
            return Err(PatchError::new(
                PatchErrorCode::UnalignedSource,
                format!(
                    "node `{}` does not match canonical node `{}`",
                    u.name().value(),
                    a.name().value()
                ),
            ));
        }
        self.diff_entries(u, b, a)?;
        self.diff_list(List {
            parent: Some((u, a)),
            un: children_of(u),
            bn: children_of(b),
            an: children_of(a),
        })
    }

    /// Diff the top-level node list.
    pub(in crate::patch) fn diff_root(
        &mut self,
        un: &[KdlNode],
        bn: &[KdlNode],
        an: &[KdlNode],
    ) -> Result<(), PatchError> {
        self.diff_list(List {
            parent: None,
            un,
            bn,
            an,
        })
    }

    /// Diff a sibling list.
    ///
    /// Children pair by key and occurrence. The longest in-order run of
    /// pairs stays in place and is diffed node by node. Every other pair
    /// moves: its source text is removed here and re-inserted at its after
    /// position. Unpaired before children are removed, and unpaired after
    /// children are inserted.
    fn diff_list(&mut self, list: List<'_>) -> Result<(), PatchError> {
        let bk = keyed(list.bn);
        let ak = keyed(list.an);
        let b_index: BTreeMap<&(Key, usize), usize> =
            bk.iter().enumerate().map(|(i, k)| (k, i)).collect();
        // `paired[j]` is the before index of after child `j`.
        let paired: Vec<Option<usize>> = ak.iter().map(|k| b_index.get(k).copied()).collect();
        let kept = kept_in_place(&paired);
        // `a_to_b[j]` is the before index of after child `j` when it stays.
        let a_to_b: Vec<Option<usize>> = paired
            .iter()
            .zip(&kept)
            .map(|(p, k)| if *k { *p } else { None })
            .collect();
        let staying: BTreeSet<usize> = a_to_b.iter().flatten().copied().collect();

        for (j, bi) in a_to_b.iter().enumerate() {
            let (Some(i), Some(a)) = (*bi, list.an.get(j)) else {
                continue;
            };
            let b = list.bn.get(i).ok_or_else(|| index_error(i))?;
            if self.unchanged(b, a)? {
                continue;
            }
            let (key, occ) = bk.get(i).ok_or_else(|| index_error(i))?;
            let u = find_source(list.un, key, *occ, &bk)?;
            self.diff_node(Triple { u, b, a })?;
        }

        let mut gone = Vec::new();
        for (i, (key, occ)) in bk.iter().enumerate() {
            if !staying.contains(&i) {
                gone.push(find_source(list.un, key, *occ, &bk)?);
            }
        }
        let inserts = a_to_b.iter().any(Option::is_none);
        let emptied = !list.un.is_empty() && gone.len() == list.un.len() && !inserts;
        self.remove_nodes(list.parent, &gone, emptied)?;
        self.insert_runs(list, &bk, &paired, &a_to_b)
    }
}
