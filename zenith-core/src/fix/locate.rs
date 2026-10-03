//! Find nodes and property entries in a KDL document by node id.
//!
//! A node is addressed by its index path from the document root. Paths stay
//! valid while no node is inserted or removed, so the planner locates every
//! site first and the applier inserts tokens last.

use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};

use crate::ast::Span;

/// Index path from the document root to a node.
pub(super) type NodePath = Vec<usize>;

/// The part of a node a fix edits.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Slot {
    /// The entry at this index (a property or positional argument).
    Entry(usize),
    /// The node name (a `defaults` row kind).
    Name,
    /// A property the node does not set yet; the fix adds it.
    NewProperty(String),
}

/// One edit site: the node path plus the slot on that node.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Site {
    pub node: NodePath,
    pub slot: Slot,
}

impl Site {
    /// The entry at `index` on the node at `node`.
    pub(super) fn entry(node: NodePath, index: usize) -> Self {
        Self {
            node,
            slot: Slot::Entry(index),
        }
    }
}

/// The string value of the `id` property on `node`, if any.
pub(super) fn node_id(node: &KdlNode) -> Option<&str> {
    match node.get("id") {
        Some(KdlValue::String(s)) => Some(s.as_str()),
        _ => None,
    }
}

/// Path of the first node (depth first) whose `id` is `id`.
///
/// `token` nodes never match: their ids name tokens, not subjects.
pub(super) fn find_node(doc: &KdlDocument, id: &str) -> Option<NodePath> {
    for (i, node) in doc.nodes().iter().enumerate() {
        if node.name().value() != "token" && node_id(node) == Some(id) {
            return Some(vec![i]);
        }
        if let Some(children) = node.children()
            && let Some(mut rest) = find_node(children, id)
        {
            rest.insert(0, i);
            return Some(rest);
        }
    }
    None
}

/// The node at `path`.
pub(super) fn node_at<'a>(doc: &'a KdlDocument, path: &[usize]) -> Option<&'a KdlNode> {
    let (first, rest) = path.split_first()?;
    let node = doc.nodes().get(*first)?;
    if rest.is_empty() {
        Some(node)
    } else {
        node_at(node.children()?, rest)
    }
}

/// The node at `path`, mutably.
pub(super) fn node_at_mut<'a>(doc: &'a mut KdlDocument, path: &[usize]) -> Option<&'a mut KdlNode> {
    let (first, rest) = path.split_first()?;
    let node = doc.nodes_mut().get_mut(*first)?;
    if rest.is_empty() {
        Some(node)
    } else {
        node_at_mut(node.children_mut().as_mut()?, rest)
    }
}

/// The entry at `site`.
pub(super) fn entry_at<'a>(doc: &'a KdlDocument, site: &Site) -> Option<&'a KdlEntry> {
    match site.slot {
        Slot::Entry(index) => node_at(doc, &site.node)?.entries().get(index),
        Slot::Name | Slot::NewProperty(_) => None,
    }
}

/// The entry at `site`, mutably.
pub(super) fn entry_at_mut<'a>(doc: &'a mut KdlDocument, site: &Site) -> Option<&'a mut KdlEntry> {
    match site.slot {
        Slot::Entry(index) => node_at_mut(doc, &site.node)?.entries_mut().get_mut(index),
        Slot::Name | Slot::NewProperty(_) => None,
    }
}

/// Every entry named `prop` on the node at `path`, then on its id-less
/// descendants (for example `span` children, whose properties report under
/// the parent id). Descendants with their own id are other subjects.
pub(super) fn property_sites(doc: &KdlDocument, path: &[usize], prop: &str) -> Vec<Site> {
    let mut out = Vec::new();
    if let Some(node) = node_at(doc, path) {
        collect_sites(node, path.to_vec(), prop, &mut out);
    }
    out
}

fn collect_sites(node: &KdlNode, path: NodePath, prop: &str, out: &mut Vec<Site>) {
    for (i, entry) in node.entries().iter().enumerate() {
        if entry.name().map(|n| n.value()) == Some(prop) {
            out.push(Site::entry(path.clone(), i));
        }
    }
    let Some(children) = node.children() else {
        return;
    };
    for (i, child) in children.nodes().iter().enumerate() {
        if node_id(child).is_none() {
            let mut child_path = path.clone();
            child_path.push(i);
            collect_sites(child, child_path, prop, out);
        }
    }
}

/// Path of the node whose source span is exactly `span` (byte offsets).
///
/// Diagnostics on rows without an id (`defaults` rows, `style` blocks) carry
/// the row span; the planner parses the same source, so spans match.
pub(super) fn find_node_by_span(doc: &KdlDocument, span: Span) -> Option<NodePath> {
    for (i, node) in doc.nodes().iter().enumerate() {
        let s = node.span();
        if s.offset() == span.start && s.offset() + s.len() == span.end {
            return Some(vec![i]);
        }
        if let Some(children) = node.children()
            && let Some(mut rest) = find_node_by_span(children, span)
        {
            rest.insert(0, i);
            return Some(rest);
        }
    }
    None
}

/// The first positional entry of the child of the node at `path` whose name
/// satisfies `is_name` and whose first positional value is the string
/// `value` (a `style` property child such as `align "centre"`).
pub(super) fn child_value_site(
    doc: &KdlDocument,
    path: &[usize],
    is_name: impl Fn(&str) -> bool,
    value: &str,
) -> Option<Site> {
    let children = node_at(doc, path)?.children()?;
    for (i, child) in children.nodes().iter().enumerate() {
        if !is_name(child.name().value()) {
            continue;
        }
        let Some(index) = child.entries().iter().position(|e| e.name().is_none()) else {
            continue;
        };
        if child.entries().get(index)?.value().as_string() == Some(value) {
            let mut child_path = path.to_vec();
            child_path.push(i);
            return Some(Site::entry(child_path, index));
        }
    }
    None
}

/// The type annotation on `entry` (`px` in `(px)24`).
pub(super) fn annotation(entry: &KdlEntry) -> Option<&str> {
    entry.ty().map(|t| t.value())
}

/// Source text of an entry value: `"#ffffff"`, `(px)24`, `700`.
///
/// Strings are always quoted.
pub(super) fn value_text(entry: &KdlEntry) -> String {
    let value = match entry.value() {
        KdlValue::String(s) => quote(s),
        other => other.to_string(),
    };
    match annotation(entry) {
        Some(ty) => format!("({ty}){value}"),
        None => value,
    }
}

/// The KDL value a fix value text denotes, with its type annotation:
/// `(px)140` → `(Some("px"), 140)`, `700` → `(None, 700)`, `cover` →
/// `(None, "cover")`. `None` for an unclosed annotation.
pub(super) fn parse_value_text(text: &str) -> Option<(Option<String>, KdlValue)> {
    let (ty, raw) = match text.strip_prefix('(') {
        Some(rest) => {
            let (ty, raw) = rest.split_once(')')?;
            (Some(ty.to_owned()), raw)
        }
        None => (None, text),
    };
    let value = if let Ok(i) = raw.parse::<i128>() {
        KdlValue::Integer(i)
    } else if let Ok(f) = raw.parse::<f64>()
        && f.is_finite()
    {
        KdlValue::Float(f)
    } else {
        KdlValue::String(raw.to_owned())
    };
    Some((ty, value))
}

/// A quoted KDL string.
pub(super) fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Plain text of a scalar value: the string itself, or the number.
pub(super) fn scalar_text(value: &KdlValue) -> Option<String> {
    match value {
        KdlValue::String(s) => Some(s.clone()),
        KdlValue::Integer(i) => Some(i.to_string()),
        KdlValue::Float(f) => Some(f.to_string()),
        KdlValue::Bool(_) | KdlValue::Null => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r##"root {
  token id="card" value=1
  page id="p" {
    text id="card" fill="#fff" {
      span fill="#000"
      group id="inner" fill="#111"
    }
  }
}"##;

    #[test]
    fn parse_value_text_reads_annotation_and_number() {
        assert_eq!(
            parse_value_text("(px)140"),
            Some((Some("px".to_owned()), KdlValue::Integer(140)))
        );
        assert_eq!(
            parse_value_text("(px)12.5"),
            Some((Some("px".to_owned()), KdlValue::Float(12.5)))
        );
        assert_eq!(
            parse_value_text("cover"),
            Some((None, KdlValue::String("cover".to_owned())))
        );
        assert_eq!(parse_value_text("(px140"), None);
    }

    #[test]
    fn find_node_skips_tokens() {
        let doc: KdlDocument = SRC.parse().expect("kdl");
        assert_eq!(find_node(&doc, "card"), Some(vec![0, 1, 0]));
        assert_eq!(find_node(&doc, "missing"), None);
    }

    #[test]
    fn property_sites_cover_idless_descendants_only() {
        let doc: KdlDocument = SRC.parse().expect("kdl");
        let sites = property_sites(&doc, &[0, 1, 0], "fill");
        let texts: Vec<String> = sites
            .iter()
            .filter_map(|s| entry_at(&doc, s).map(value_text))
            .collect();
        assert_eq!(texts, vec!["\"#fff\"", "\"#000\""]);
    }
}
