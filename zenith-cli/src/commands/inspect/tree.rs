//! Inspect tree builders: the per-page node tree with authored geometry, the
//! node finder, and the authored-geometry helpers.

use std::collections::BTreeMap;

use zenith_core::{
    Dimension, FrameNode, GroupNode, Node, Page, PropertyValue, ResolvedValue, Unit,
};

use super::document::{NodeEntry, NodeGeometry, PageEntry, Resolved};

// ── Tree builders ─────────────────────────────────────────────────────────────

/// Build the full page tree for all pages in the document (in order).
///
/// `resolved` is the document's resolved token table; it turns `(token)"id"`
/// dimension refs into px values in each node's geometry.
pub fn build_doc_tree(pages: &[Page], resolved: &Resolved) -> Vec<PageEntry> {
    pages
        .iter()
        .map(|p| build_page_entry(p, resolved))
        .collect()
}

fn build_page_entry(page: &Page, resolved: &Resolved) -> PageEntry {
    PageEntry {
        id: page.id.clone(),
        name: page.name.clone(),
        width: dim_to_f64(&page.width),
        height: dim_to_f64(&page.height),
        expanded: BTreeMap::new(),
        children: page
            .children
            .iter()
            .map(|n| build_node_entry(n, resolved))
            .collect(),
    }
}

fn build_node_entry(node: &Node, resolved: &Resolved) -> NodeEntry {
    let children: Vec<NodeEntry> = match node {
        Node::Frame(FrameNode { children, .. }) | Node::Group(GroupNode { children, .. }) => {
            children
                .iter()
                .map(|c| build_node_entry(c, resolved))
                .collect()
        }
        Node::Unknown(n) => n
            .children
            .iter()
            .map(|c| build_node_entry(c, resolved))
            .collect(),
        // Report each cell's child nodes (flattened in row→cell order) so a
        // table's content is visible in the inspect tree.
        Node::Table(n) => n
            .rows
            .iter()
            .flat_map(|row| row.cells.iter())
            .flat_map(|cell| cell.children.iter())
            .map(|c| build_node_entry(c, resolved))
            .collect(),
        // A shape owns label spans (TextSpans), not child Nodes.
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
        | Node::Mesh(_) => Vec::new(),
    };
    let kind = if let Node::Unknown(n) = node {
        n.kind.clone()
    } else {
        node.kind_str().to_owned()
    };
    NodeEntry {
        id: node_id_str(node).to_owned(),
        kind,
        role: node.role().map(str::to_owned),
        geometry: authored_geometry(node, resolved),
        resolved: None,
        visible: node.visible(),
        locked: node_locked(node),
        children,
    }
}

/// The authored geometry of `node`, resolved to px.
fn authored_geometry(node: &Node, resolved: &Resolved) -> Option<NodeGeometry> {
    if let Some(b) = node.box_view() {
        // Every box kind reports its x/y/w/h (a field may omit some; they
        // default to the page live area at compile time).
        return bbox_geom(b.x, b.y, b.w, b.h, resolved);
    }
    let empty = NodeGeometry {
        x: None,
        y: None,
        w: None,
        h: None,
        x1: None,
        y1: None,
        x2: None,
        y2: None,
        point_count: None,
    };
    match node {
        Node::Line(n) => Some(NodeGeometry {
            x1: n.x1.as_ref().map(dim_to_f64),
            y1: n.y1.as_ref().map(dim_to_f64),
            x2: n.x2.as_ref().map(dim_to_f64),
            y2: n.y2.as_ref().map(dim_to_f64),
            ..empty
        }),
        Node::Polygon(n) => Some(NodeGeometry {
            point_count: Some(n.points.len()),
            ..empty
        }),
        Node::Polyline(n) => Some(NodeGeometry {
            point_count: Some(n.points.len()),
            ..empty
        }),
        Node::Path(n) => Some(NodeGeometry {
            point_count: Some(n.anchors.len()),
            ..empty
        }),
        // An instance's authored x/y stay raw `Dimension` (not token-ref
        // geometry), so report them directly; it has no authored w/h box.
        Node::Instance(n) => Some(NodeGeometry {
            x: opt_dim_to_f64(n.x.as_ref()),
            y: opt_dim_to_f64(n.y.as_ref()),
            ..empty
        }),
        Node::Light(n) => light_geom(n, resolved),
        // A footnote is positioned in the footnote zone, a connector derives
        // its endpoints from its targets, and an unknown kind is opaque.
        Node::Footnote(_) | Node::Connector(_) | Node::Unknown(_) => None,
        // Box kinds returned above.
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Text(_)
        | Node::Code(_)
        | Node::Frame(_)
        | Node::Group(_)
        | Node::Image(_)
        | Node::Field(_)
        | Node::Toc(_)
        | Node::Table(_)
        | Node::Shape(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Mesh(_) => None,
    }
}

/// The authored `locked` flag; `None` for kinds without one.
fn node_locked(node: &Node) -> Option<bool> {
    match node {
        Node::Rect(n) => n.locked,
        Node::Ellipse(n) => n.locked,
        Node::Line(n) => n.locked,
        Node::Text(n) => n.locked,
        Node::Code(n) => n.locked,
        Node::Frame(n) => n.locked,
        Node::Group(n) => n.locked,
        Node::Image(n) => n.locked,
        Node::Polygon(n) => n.locked,
        Node::Polyline(n) => n.locked,
        Node::Path(n) => n.locked,
        Node::Instance(n) => n.locked,
        Node::Field(n) => n.locked,
        Node::Toc(n) => n.locked,
        Node::Table(n) => n.locked,
        Node::Shape(n) => n.locked,
        Node::Connector(n) => n.locked,
        Node::Pattern(n) => n.locked,
        Node::Chart(n) => n.locked,
        Node::Light(n) => n.locked,
        Node::Mesh(n) => n.locked,
        Node::Footnote(_) | Node::Unknown(_) => None,
    }
}

// ── Node finder ───────────────────────────────────────────────────────────────

/// Search all pages (depth-first, in source order) for a node with the given
/// id.  Returns a fully-built [`NodeEntry`] subtree when found.
pub fn find_node_tree(pages: &[Page], id: &str, resolved: &Resolved) -> Option<NodeEntry> {
    for page in pages {
        if let Some(entry) = search_nodes(&page.children, id, resolved) {
            return Some(entry);
        }
    }
    None
}

fn search_nodes(nodes: &[Node], id: &str, resolved: &Resolved) -> Option<NodeEntry> {
    for node in nodes {
        // Check if this node matches.
        let node_id = node_id_str(node);
        if node_id == id {
            return Some(build_node_entry(node, resolved));
        }
        // Recurse into Frame/Group/Unknown children via node_children.
        if let Some(children) = node_children(node)
            && let Some(found) = search_nodes(children, id, resolved)
        {
            return Some(found);
        }
        // Recurse into table cell children (node_children returns None for Table).
        if let Node::Table(t) = node {
            for row in &t.rows {
                for cell in &row.cells {
                    if let Some(found) = search_nodes(&cell.children, id, resolved) {
                        return Some(found);
                    }
                }
            }
        }
    }
    None
}

/// Return the `id` field of a node as a `&str`.
fn node_id_str(node: &Node) -> &str {
    match node {
        Node::Rect(n) => &n.id,
        Node::Ellipse(n) => &n.id,
        Node::Line(n) => &n.id,
        Node::Text(n) => &n.id,
        Node::Code(n) => &n.id,
        Node::Frame(n) => &n.id,
        Node::Group(n) => &n.id,
        Node::Image(n) => &n.id,
        Node::Polygon(n) => &n.id,
        Node::Polyline(n) => &n.id,
        Node::Path(n) => &n.id,
        Node::Instance(n) => &n.id,
        Node::Field(n) => &n.id,
        Node::Toc(n) => &n.id,
        Node::Footnote(n) => &n.id,
        Node::Table(n) => &n.id,
        Node::Shape(n) => &n.id,
        Node::Connector(n) => &n.id,
        Node::Pattern(n) => &n.id,
        Node::Chart(n) => &n.id,
        Node::Light(n) => &n.id,
        Node::Mesh(n) => &n.id,
        Node::Unknown(n) => n.id.as_deref().unwrap_or(""),
    }
}

/// Return a reference to a container node's children slice, or `None` for leaf
/// nodes.
fn node_children(node: &Node) -> Option<&[Node]> {
    match node {
        Node::Frame(FrameNode { children, .. }) | Node::Group(GroupNode { children, .. }) => {
            Some(children)
        }
        Node::Unknown(n) => Some(&n.children),
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
        | Node::Table(_)
        | Node::Shape(_)
        | Node::Connector(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Mesh(_) => None,
    }
}

// ── Geometry helpers ──────────────────────────────────────────────────────────

fn dim_to_f64(d: &Dimension) -> f64 {
    match d.unit {
        Unit::Pt => d.value * 96.0 / 72.0,
        Unit::Px | Unit::Pct | Unit::Deg | Unit::Unknown(_) => d.value,
    }
}

fn opt_dim_to_f64(d: Option<&Dimension>) -> Option<f64> {
    d.map(dim_to_f64)
}

/// A geometry property is `(px)N` literal OR `(token)"id"` dimension ref.
/// Inspect reports the resolved px value: a literal yields its own value; a
/// token ref is resolved against the document's token table. A ref that is
/// missing, cyclic, or not a dimension token has no px value, so it shows as
/// `None` (the field is omitted from the JSON).
fn opt_pv_to_f64(pv: Option<&PropertyValue>, resolved: &Resolved) -> Option<f64> {
    match pv? {
        PropertyValue::Dimension(d) => Some(dim_to_f64(d)),
        PropertyValue::TokenRef(id) => match resolved.get(id).map(|t| &t.value) {
            Some(ResolvedValue::Dimension(d)) => Some(dim_to_f64(d)),
            Some(
                ResolvedValue::Color(_)
                | ResolvedValue::CmykColor { .. }
                | ResolvedValue::Number(_)
                | ResolvedValue::FontFamily(_)
                | ResolvedValue::FontWeight(_)
                | ResolvedValue::Gradient(_)
                | ResolvedValue::Shadow(_)
                | ResolvedValue::Filter(_)
                | ResolvedValue::Mask(_),
            )
            | None => None,
        },
        PropertyValue::Literal(_) | PropertyValue::DataRef(_) => None,
    }
}

fn bbox_geom(
    x: Option<&PropertyValue>,
    y: Option<&PropertyValue>,
    w: Option<&PropertyValue>,
    h: Option<&PropertyValue>,
    resolved: &Resolved,
) -> Option<NodeGeometry> {
    Some(NodeGeometry {
        x: opt_pv_to_f64(x, resolved),
        y: opt_pv_to_f64(y, resolved),
        w: opt_pv_to_f64(w, resolved),
        h: opt_pv_to_f64(h, resolved),
        x1: None,
        y1: None,
        x2: None,
        y2: None,
        point_count: None,
    })
}

fn light_geom(n: &zenith_core::LightNode, resolved: &Resolved) -> Option<NodeGeometry> {
    let x = opt_pv_to_f64(n.x.as_ref(), resolved)?;
    let y = opt_pv_to_f64(n.y.as_ref(), resolved)?;
    let radius = opt_pv_to_f64(n.radius.as_ref(), resolved)?;
    Some(NodeGeometry {
        x: Some(x - radius),
        y: Some(y - radius),
        w: Some(radius * 2.0),
        h: Some(radius * 2.0),
        x1: None,
        y1: None,
        x2: None,
        y2: None,
        point_count: None,
    })
}
