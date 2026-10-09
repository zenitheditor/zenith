//! Node identity: a node's name plus its `id`, and lookups by identity.

use std::collections::BTreeMap;

use kdl::{KdlNode, KdlValue};

use super::super::error::{PatchError, PatchErrorCode};
use super::nodes::children_of;

/// A node's identity among its siblings: its name and its `id` property.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Key {
    name: String,
    id: Option<String>,
}

impl Key {
    pub(super) fn of(node: &KdlNode) -> Self {
        let id = match node.get("id") {
            Some(KdlValue::String(s)) => Some(s.clone()),
            _ => None,
        };
        Self {
            name: node.name().value().to_owned(),
            id,
        }
    }

    fn describe(&self) -> String {
        match &self.id {
            Some(id) => format!("`{} id=\"{id}\"`", self.name),
            None => format!("`{}`", self.name),
        }
    }
}

/// Every node's key and its occurrence index among same-key siblings.
pub(super) fn keyed(nodes: &[KdlNode]) -> Vec<(Key, usize)> {
    let mut seen: BTreeMap<Key, usize> = BTreeMap::new();
    nodes
        .iter()
        .map(|n| {
            let key = Key::of(n);
            let slot = seen.entry(key.clone()).or_insert(0);
            let occ = *slot;
            *slot += 1;
            (key, occ)
        })
        .collect()
}

/// The source node that matches canonical `key`/`occ`. The source must hold
/// the key exactly as often as the canonical list `canon` does.
pub(super) fn find_source<'u>(
    un: &'u [KdlNode],
    key: &Key,
    occ: usize,
    canon: &[(Key, usize)],
) -> Result<&'u KdlNode, PatchError> {
    let canon_count = canon.iter().filter(|(k, _)| k == key).count();
    let matches: Vec<&KdlNode> = un.iter().filter(|n| Key::of(n) == *key).collect();
    if matches.len() != canon_count {
        return Err(PatchError::new(
            PatchErrorCode::UnalignedSource,
            format!(
                "the source has {} {} node(s), the canonical form has {canon_count}",
                matches.len(),
                key.describe()
            ),
        ));
    }
    matches.get(occ).copied().ok_or_else(|| index_error(occ))
}

pub(super) fn index_error(i: usize) -> PatchError {
    PatchError::new(
        PatchErrorCode::UnalignedSource,
        format!("child index {i} is out of range"),
    )
}

/// Every id-bearing node of the source, canonical before, and canonical
/// after trees, by key. It finds the old site of a node that moves to
/// another parent.
pub(in crate::patch) struct Index<'t> {
    src: BTreeMap<Key, Vec<&'t KdlNode>>,
    before: BTreeMap<Key, Vec<&'t KdlNode>>,
    after: BTreeMap<Key, Vec<&'t KdlNode>>,
}

impl<'t> Index<'t> {
    pub(in crate::patch) fn new(
        src: &'t [KdlNode],
        before: &'t [KdlNode],
        after: &'t [KdlNode],
    ) -> Self {
        Self {
            src: collect(src),
            before: collect(before),
            after: collect(after),
        }
    }

    /// The source and canonical before node of after node `a`, when `a`
    /// carries an `id` that occurs exactly once in each of the three trees.
    pub(super) fn moved(&self, a: &KdlNode) -> Option<(&'t KdlNode, &'t KdlNode)> {
        let key = Key::of(a);
        key.id.as_ref()?;
        let one = |map: &BTreeMap<Key, Vec<&'t KdlNode>>| match map.get(&key).map(Vec::as_slice) {
            Some([only]) => Some(*only),
            _ => None,
        };
        one(&self.after)?;
        Some((one(&self.src)?, one(&self.before)?))
    }
}

fn collect(nodes: &[KdlNode]) -> BTreeMap<Key, Vec<&KdlNode>> {
    let mut out = BTreeMap::new();
    let mut stack: Vec<&KdlNode> = nodes.iter().rev().collect();
    while let Some(node) = stack.pop() {
        let key = Key::of(node);
        if key.id.is_some() {
            out.entry(key).or_insert_with(Vec::new).push(node);
        }
        stack.extend(children_of(node).iter().rev());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use kdl::KdlDocument;

    fn nodes(src: &str) -> KdlDocument {
        src.parse().expect("kdl")
    }

    #[test]
    fn keyed_counts_occurrences_per_key() {
        let doc = nodes("span \"a\"\nrect id=\"r\"\nspan \"b\"\nrect id=\"s\"");
        let keys = keyed(doc.nodes());
        let occ: Vec<usize> = keys.iter().map(|(_, o)| *o).collect();
        assert_eq!(occ, vec![0, 0, 1, 0]);
    }

    #[test]
    fn similar_ids_are_distinct_keys() {
        let doc = nodes("rect id=\"card\"\nrect id=\"card.a\"\nrect id=\"card.a.b\"");
        let keys = keyed(doc.nodes());
        assert!(keys.iter().all(|(_, o)| *o == 0));
    }

    #[test]
    fn find_source_requires_equal_counts() {
        let un = nodes("span \"a\"\nspan \"b\"");
        let canon = nodes("span \"a\"");
        let key = Key::of(&canon.nodes()[0]);
        let err = find_source(un.nodes(), &key, 0, &keyed(canon.nodes())).expect_err("count");
        assert_eq!(err.code, PatchErrorCode::UnalignedSource);
    }

    #[test]
    fn index_finds_a_node_under_another_parent() {
        let src = nodes("g id=\"g\" {\n  rect id=\"r\"\n}");
        let before = nodes("g id=\"g\" {\n  rect id=\"r\"\n}");
        let after = nodes("g id=\"g\"\nrect id=\"r\"");
        let index = Index::new(src.nodes(), before.nodes(), after.nodes());
        let (u, b) = index.moved(&after.nodes()[1]).expect("moved");
        assert_eq!(u.name().value(), "rect");
        assert_eq!(b.name().value(), "rect");
    }

    #[test]
    fn index_skips_repeated_and_id_less_nodes() {
        let src = nodes("rect id=\"r\"\nrect id=\"r\"\nspan \"a\"");
        let index = Index::new(src.nodes(), src.nodes(), src.nodes());
        assert!(index.moved(&src.nodes()[0]).is_none());
        assert!(index.moved(&src.nodes()[2]).is_none());
    }
}
