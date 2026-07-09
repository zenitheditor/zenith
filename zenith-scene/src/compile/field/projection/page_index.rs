//! Document-wide `node id → 1-based page index` map for `page-ref` resolution.

use std::collections::BTreeMap;

use zenith_core::Node;

use super::common::node_id;

/// Build the document-wide `node id → 1-based page index` map for `page-ref`
/// resolution. Deterministic: walks pages in order, descending into
/// `group`/`frame` containers in source order. The FIRST occurrence of an id
/// wins (ids are globally unique in a valid document; a duplicate keeps the
/// earliest page, deterministically).
pub(in crate::compile) fn build_page_index_map(
    doc: &zenith_core::Document,
) -> BTreeMap<String, usize> {
    let mut map: BTreeMap<String, usize> = BTreeMap::new();
    for (page_idx0, page) in doc.body.pages.iter().enumerate() {
        let page_index_1based = page_idx0 + 1;
        index_nodes(&page.children, page_index_1based, &mut map);
    }
    map
}

/// Recursively record each node's id → `page_index_1based`, descending into
/// `group`/`frame` children. First write wins (does not overwrite).
fn index_nodes(children: &[Node], page_index_1based: usize, map: &mut BTreeMap<String, usize>) {
    for child in children {
        if let Some(id) = node_id(child) {
            map.entry(id.to_owned()).or_insert(page_index_1based);
        }
        match child {
            Node::Frame(f) => index_nodes(&f.children, page_index_1based, map),
            Node::Group(g) => index_nodes(&g.children, page_index_1based, map),
            Node::Table(t) => {
                for row in &t.rows {
                    for cell in &row.cells {
                        index_nodes(&cell.children, page_index_1based, map);
                    }
                }
            }
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
            | Node::Toc(_)
            | Node::Footnote(_)
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Unknown(_) => {}
        }
    }
}
