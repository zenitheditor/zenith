//! Re-id a page copy and point its internal references at the copies.

use std::collections::BTreeMap;

use zenith_core::Node;
use zenith_core::ast::document::{Fold, PortDef, SafeZone};

use super::super::anchor::anchor_fields_mut;
use super::duplicate::{node_set_id_any, suffix_ids_in_children, suffix_zone_and_fold_ids};

/// Append `id_suffix` to every node, safe-zone, and fold id of a page copy.
///
/// References inside the copy follow: an `anchor-sibling`, an
/// `anchor-zone`, or a connector `from` / `to` that names an id of the copy
/// gets the suffix too, with or without a `#port` tail. A page port follows
/// the node it names. A reference to an id outside the copy stays as is.
pub(crate) fn suffix_page_copy(
    children: &mut [Node],
    safe_zones: &mut [SafeZone],
    folds: &mut [Fold],
    ports: &mut [PortDef],
    id_suffix: &str,
) {
    let mut ids = BTreeMap::new();
    collect_ids(children, id_suffix, &mut ids);
    let zones: BTreeMap<String, String> = safe_zones
        .iter()
        .map(|z| (z.id.clone(), format!("{}{id_suffix}", z.id)))
        .collect();
    suffix_ids_in_children(children, id_suffix);
    suffix_zone_and_fold_ids(safe_zones, folds, id_suffix);
    for port in ports.iter_mut() {
        remap(Some(&mut port.node), &ids);
    }
    remap_refs(
        children,
        Refs {
            ids: &ids,
            zones: &zones,
        },
    );
}

/// Re-id a cloned subtree: `root` takes `new_id` and every descendant takes
/// its entry in `ids` (old id to new id, root included).
///
/// References inside the copy follow: an `anchor-sibling` or a connector
/// `from` / `to` (with or without a `#port` tail) that names a node of the
/// subtree points at the copy of that node. A reference to a node outside the
/// subtree, and any `anchor-zone` (a page safe-zone), stays as is.
pub(super) fn copy_subtree(root: &mut Node, new_id: &str, ids: &BTreeMap<String, String>) {
    node_set_id_any(root, new_id.to_owned());
    let zones = BTreeMap::new();
    let refs = Refs { ids, zones: &zones };
    for list in nested(root) {
        reid_children(list, ids);
        remap_refs(list, refs);
    }
}

/// Give every id-bearing node under `children` its entry in `ids`.
fn reid_children(children: &mut [Node], ids: &BTreeMap<String, String>) {
    for child in children.iter_mut() {
        if let Some(new) = child.id().and_then(|old| ids.get(old)).cloned() {
            node_set_id_any(child, new);
        }
        for list in nested(child) {
            reid_children(list, ids);
        }
    }
}

/// Copy every page port that names a node in `ids` for the copy of that node,
/// directly after the original port.
pub(super) fn copy_ports(ports: &mut Vec<PortDef>, ids: &BTreeMap<String, String>) {
    let old = std::mem::take(ports);
    for port in old {
        let copy = ids.get(&port.node).map(|node| PortDef {
            node: node.clone(),
            id: port.id.clone(),
            anchor: port.anchor.clone(),
            source_span: None,
        });
        ports.push(port);
        ports.extend(copy);
    }
}

/// Old id to new id for the nodes and zones of a copy.
#[derive(Clone, Copy)]
struct Refs<'a> {
    ids: &'a BTreeMap<String, String>,
    zones: &'a BTreeMap<String, String>,
}

/// The nested node lists that [`suffix_ids_in_children`] walks.
fn nested(node: &mut Node) -> Vec<&mut Vec<Node>> {
    match node {
        Node::Frame(f) => vec![&mut f.children],
        Node::Group(g) => vec![&mut g.children],
        Node::Table(t) => t
            .rows
            .iter_mut()
            .flat_map(|row| row.cells.iter_mut().map(|cell| &mut cell.children))
            .collect(),
        Node::Unknown(u) => vec![&mut u.children],
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Line(_)
        | Node::Text(_)
        | Node::Code(_)
        | Node::Image(_)
        | Node::Polygon(_)
        | Node::Polyline(_)
        | Node::Path(_)
        | Node::Instance(_)
        | Node::Field(_)
        | Node::Footnote(_)
        | Node::Toc(_)
        | Node::Shape(_)
        | Node::Connector(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Mesh(_) => Vec::new(),
    }
}

/// Map every old id under `children` to `old + suffix`.
fn collect_ids(children: &mut [Node], suffix: &str, out: &mut BTreeMap<String, String>) {
    for child in children.iter_mut() {
        if let Some(id) = child.id() {
            out.insert(id.to_owned(), format!("{id}{suffix}"));
        }
        for list in nested(child) {
            collect_ids(list, suffix, out);
        }
    }
}

fn remap_refs(children: &mut [Node], refs: Refs<'_>) {
    for child in children.iter_mut() {
        if let Some(fields) = anchor_fields_mut(child) {
            remap(fields.sibling.as_mut(), refs.ids);
            remap(fields.zone.as_mut(), refs.zones);
        }
        if let Node::Connector(c) = child {
            remap_endpoint(&mut c.from, refs.ids);
            remap_endpoint(&mut c.to, refs.ids);
        }
        for list in nested(child) {
            remap_refs(list, refs);
        }
    }
}

fn remap(slot: Option<&mut String>, map: &BTreeMap<String, String>) {
    if let Some(value) = slot
        && let Some(new) = map.get(value.as_str())
    {
        value.clone_from(new);
    }
}

/// Remap a connector endpoint: a node id, or `node#port`.
fn remap_endpoint(slot: &mut Option<String>, map: &BTreeMap<String, String>) {
    let Some(value) = slot else {
        return;
    };
    match value.split_once('#') {
        Some((node, port)) => {
            if let Some(new) = map.get(node) {
                *value = format!("{new}#{port}");
            }
        }
        None => remap(Some(value), map),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{KdlAdapter, KdlSource};

    fn page_children(src: &str) -> Vec<Node> {
        let doc = KdlAdapter.parse(src.as_bytes()).expect("parse");
        doc.body.pages[0].children.clone()
    }

    const SRC: &str = r#"zenith version=1 {
  tokens format="zenith-token-v1" {}
  styles {}
  document id="d" {
    page id="p" w=(px)100 h=(px)100 {
      rect id="a" x=(px)0 y=(px)0 w=(px)10 h=(px)10
      frame id="f" x=(px)0 y=(px)0 w=(px)50 h=(px)50 {
        rect id="b" anchor-sibling="c" anchor-edge="below" w=(px)10 h=(px)10
        rect id="c" x=(px)0 y=(px)0 w=(px)10 h=(px)10
      }
      rect id="d2" anchor-sibling="a" anchor-edge="after" w=(px)10 h=(px)10
      connector id="e" from="a" to="outside"
    }
  }
}
"#;

    #[test]
    fn copy_references_follow_the_suffix() {
        let mut children = page_children(SRC);
        suffix_page_copy(&mut children, &mut [], &mut [], &mut [], ".v2");
        let Node::Frame(f) = &children[1] else {
            panic!("frame");
        };
        let Node::Rect(b) = &f.children[0] else {
            panic!("rect");
        };
        assert_eq!(b.id, "b.v2");
        assert_eq!(b.anchor_sibling.as_deref(), Some("c.v2"));
        let Node::Rect(d) = &children[2] else {
            panic!("rect");
        };
        assert_eq!(d.anchor_sibling.as_deref(), Some("a.v2"));
        let Node::Connector(e) = &children[3] else {
            panic!("connector");
        };
        assert_eq!(e.from.as_deref(), Some("a.v2"));
        assert_eq!(e.to.as_deref(), Some("outside"));
    }
}
