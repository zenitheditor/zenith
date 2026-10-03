//! Bbox geometry accessors shared by the geometry ops: read and write a
//! node's `x` / `y` / `w` / `h`, convert between a node's parent space and
//! page space, and find the page bounds.

use std::collections::BTreeMap;

use zenith_core::{
    Dimension, Document, Node, PropertyValue, ResolvedToken, dim_to_px, resolve_geometry_px,
};

use super::super::space::{Chain, Link, chain_origin, common_prefix, parent_chain};
use super::super::{find_node_any_mut, find_node_any_shared, px, subtree_contains};

/// Mutable references to a node's four bbox geometry slots `(x, y, w, h)`.
type GeometryMut<'a> = (
    &'a mut Option<PropertyValue>,
    &'a mut Option<PropertyValue>,
    &'a mut Option<PropertyValue>,
    &'a mut Option<PropertyValue>,
);

/// Return mutable references to the four bbox geometry fields `(x, y, w, h)`,
/// or `None` for node variants excluded from `set_geometry`.
///
/// The bbox nodes — `Rect`, `Ellipse`, `Frame`, `Image`, `Text`, `Code`, and
/// `Group` — are settable: each carries canonical `x/y/w/h` fields (a text/code
/// node's `x/y/w/h` is its text box; a group's `x/y` is a real translation
/// offset applied to its children at render time).
///
/// `Line` is excluded because it has no bbox — it uses `x1/y1/x2/y2` endpoints.
/// `Polygon` and `Polyline` are excluded because they have no bbox either — their
/// geometry is the `points` list. `Unknown` is excluded because its schema is opaque.
pub(in crate::engine) fn node_geometry_mut(node: &mut Node) -> Option<GeometryMut<'_>> {
    match node {
        Node::Rect(r) => Some((&mut r.x, &mut r.y, &mut r.w, &mut r.h)),
        Node::Ellipse(e) => Some((&mut e.x, &mut e.y, &mut e.w, &mut e.h)),
        Node::Frame(f) => Some((&mut f.x, &mut f.y, &mut f.w, &mut f.h)),
        Node::Image(i) => Some((&mut i.x, &mut i.y, &mut i.w, &mut i.h)),
        Node::Text(t) => Some((&mut t.x, &mut t.y, &mut t.w, &mut t.h)),
        Node::Code(c) => Some((&mut c.x, &mut c.y, &mut c.w, &mut c.h)),
        Node::Group(g) => Some((&mut g.x, &mut g.y, &mut g.w, &mut g.h)),
        // A field carries a real x/y/w/h box (the resolved single-line text box),
        // so set_geometry applies to it like any other bbox node.
        Node::Field(f) => Some((&mut f.x, &mut f.y, &mut f.w, &mut f.h)),
        // A toc likewise carries a real x/y/w/h box.
        Node::Toc(t) => Some((&mut t.x, &mut t.y, &mut t.w, &mut t.h)),
        // A table carries a real x/y/w/h box.
        Node::Table(t) => Some((&mut t.x, &mut t.y, &mut t.w, &mut t.h)),
        // A shape carries a real x/y/w/h background box.
        Node::Shape(s) => Some((&mut s.x, &mut s.y, &mut s.w, &mut s.h)),
        // A pattern carries a real x/y/w/h box (the region it tiles over).
        Node::Pattern(p) => Some((&mut p.x, &mut p.y, &mut p.w, &mut p.h)),
        // A chart carries a real x/y/w/h box.
        Node::Chart(c) => Some((&mut c.x, &mut c.y, &mut c.w, &mut c.h)),
        Node::Mesh(m) => Some((&mut m.x, &mut m.y, &mut m.w, &mut m.h)),
        Node::Light(_) => None,
        // `Instance` uses `Option<Dimension>` slots (not `PropertyValue`);
        // layout writes go through [`write_node_xy`] / [`apply_set_geometry`].
        // A footnote has NO x/y/w/h box (the renderer positions it in the
        // footnote zone), so set_geometry does not apply — it honestly surfaces
        // tx.unsupported_property rather than silently dropping the request.
        // A connector has NO authored box (its endpoints are derived from its
        // targets' boxes), so set_geometry does not apply either.
        Node::Line(_)
        | Node::Polygon(_)
        | Node::Polyline(_)
        | Node::Path(_)
        | Node::Instance(_)
        | Node::Footnote(_)
        | Node::Connector(_)
        | Node::Unknown(_) => None,
    }
}

/// Write absolute `x`/`y` (px) onto a node that participates in layout ops.
///
/// Handles both `PropertyValue` geometry (via [`node_geometry_mut`]) and
/// `Instance` `Option<Dimension>` slots. Returns `true` when a write path
/// existed for the node kind.
fn write_node_xy(node: &mut Node, new_x: Option<f64>, new_y: Option<f64>) -> bool {
    if new_x.is_none() && new_y.is_none() {
        return false;
    }
    if let Node::Instance(inst) = node {
        if let Some(v) = new_x {
            inst.x = Some(px(v));
        }
        if let Some(v) = new_y {
            inst.y = Some(px(v));
        }
        return true;
    }
    if let Some((nx, ny, _, _)) = node_geometry_mut(node) {
        if let Some(v) = new_x {
            *nx = Some(PropertyValue::Dimension(px(v)));
        }
        if let Some(v) = new_y {
            *ny = Some(PropertyValue::Dimension(px(v)));
        }
        return true;
    }
    false
}

/// Read the four bbox dimensions of a node as px values, in its parent's
/// space, if the node kind supports geometry and all four fields resolve.
///
/// Returns `Some((x, y, w, h))` or `None` if the node is unsupported, any
/// field is absent, or any value does not resolve to px (e.g. `%`, `deg`).
fn read_geometry_px(
    node: &Node,
    resolved: &BTreeMap<String, ResolvedToken>,
) -> Option<(f64, f64, f64, f64)> {
    let (x, y, w, h) = match node {
        Node::Rect(r) => (r.x.as_ref(), r.y.as_ref(), r.w.as_ref(), r.h.as_ref()),
        Node::Ellipse(e) => (e.x.as_ref(), e.y.as_ref(), e.w.as_ref(), e.h.as_ref()),
        Node::Frame(f) => (f.x.as_ref(), f.y.as_ref(), f.w.as_ref(), f.h.as_ref()),
        Node::Image(i) => (i.x.as_ref(), i.y.as_ref(), i.w.as_ref(), i.h.as_ref()),
        Node::Text(t) => (t.x.as_ref(), t.y.as_ref(), t.w.as_ref(), t.h.as_ref()),
        Node::Code(c) => (c.x.as_ref(), c.y.as_ref(), c.w.as_ref(), c.h.as_ref()),
        Node::Group(g) => (g.x.as_ref(), g.y.as_ref(), g.w.as_ref(), g.h.as_ref()),
        Node::Shape(s) => (s.x.as_ref(), s.y.as_ref(), s.w.as_ref(), s.h.as_ref()),
        Node::Pattern(p) => (p.x.as_ref(), p.y.as_ref(), p.w.as_ref(), p.h.as_ref()),
        Node::Chart(c) => (c.x.as_ref(), c.y.as_ref(), c.w.as_ref(), c.h.as_ref()),
        Node::Mesh(m) => (m.x.as_ref(), m.y.as_ref(), m.w.as_ref(), m.h.as_ref()),
        Node::Field(f) => (f.x.as_ref(), f.y.as_ref(), f.w.as_ref(), f.h.as_ref()),
        Node::Toc(t) => (t.x.as_ref(), t.y.as_ref(), t.w.as_ref(), t.h.as_ref()),
        Node::Table(t) => (t.x.as_ref(), t.y.as_ref(), t.w.as_ref(), t.h.as_ref()),
        Node::Light(_) => return None,
        // Instance stores x/y/w/h as Option<Dimension> (not PropertyValue).
        Node::Instance(i) => {
            let resolve_dim = |d: Option<&Dimension>| -> Option<f64> {
                d.and_then(|dim| dim_to_px(dim.value, &dim.unit))
            };
            return Some((
                resolve_dim(i.x.as_ref())?,
                resolve_dim(i.y.as_ref())?,
                resolve_dim(i.w.as_ref())?,
                resolve_dim(i.h.as_ref())?,
            ));
        }
        Node::Line(_)
        | Node::Polygon(_)
        | Node::Polyline(_)
        | Node::Path(_)
        | Node::Footnote(_)
        | Node::Connector(_)
        | Node::Unknown(_) => return None,
    };
    // A raw dimension or a dimension token resolves; a pct, literal, or data
    // value does not.
    let resolve = |pv: Option<&PropertyValue>| resolve_geometry_px(pv, resolved);
    Some((resolve(x)?, resolve(y)?, resolve(w)?, resolve(h)?))
}

/// Resolve page width/height in px for the page that owns `node_id`, or for a
/// page that references the master hosting `node_id` (chrome is page-coordinate).
pub(super) fn page_bounds_for_node(doc: &Document, node_id: &str) -> Option<(f64, f64)> {
    for page in &doc.body.pages {
        if page.children.iter().any(|n| subtree_contains(n, node_id)) {
            let pw = dim_to_px(page.width.value, &page.width.unit)?;
            let ph = dim_to_px(page.height.value, &page.height.unit)?;
            return Some((pw, ph));
        }
    }
    // Master-hosted: prefer a page that references that master.
    for master in &doc.masters {
        if !master.children.iter().any(|n| subtree_contains(n, node_id)) {
            continue;
        }
        let master_id = master.id.as_str();
        if let Some(page) = doc
            .body
            .pages
            .iter()
            .find(|p| p.master.as_deref() == Some(master_id))
        {
            let pw = dim_to_px(page.width.value, &page.width.unit)?;
            let ph = dim_to_px(page.height.value, &page.height.unit)?;
            return Some((pw, ph));
        }
        // No page opts into this master yet — fall back to first page artboard.
        if let Some(page) = doc.body.pages.first() {
            let pw = dim_to_px(page.width.value, &page.width.unit)?;
            let ph = dim_to_px(page.height.value, &page.height.unit)?;
            return Some((pw, ph));
        }
    }
    None
}

/// A found node: its box in its parent's space, when it resolves, and the
/// chain of that space.
pub(super) struct Placed {
    pub id: String,
    pub local: Option<(f64, f64, f64, f64)>,
    pub chain: Chain,
}

/// Read node `id`. `None` when no node has that id.
pub(super) fn placed(
    doc: &Document,
    id: &str,
    resolved: &BTreeMap<String, ResolvedToken>,
) -> Option<Placed> {
    let node = find_node_any_shared(doc, id)?;
    let local = read_geometry_px(node, resolved);
    let chain = parent_chain(doc, id, resolved)?;
    Some(Placed {
        id: id.to_owned(),
        local,
        chain,
    })
}

/// The number of chain links every resolvable box in `placed` shares: the
/// shared space of a selection.
pub(super) fn shared_links<'a>(placed: impl IntoIterator<Item = &'a Placed>) -> usize {
    let chains: Vec<&[Link]> = placed
        .into_iter()
        .filter(|p| p.local.is_some())
        .map(|p| p.chain.as_slice())
        .collect();
    common_prefix(&chains)
}

/// A node box in a shared space: page space, or the child space of the
/// ancestor container a selection shares. `origin` is the origin of the
/// node's parent space in that shared space: a shared `x` / `y` minus
/// `origin` gives the authored value.
pub(super) struct SpaceBox {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub origin: (f64, f64),
}

/// Why [`space_box`] gives no box.
pub(super) enum BoxError {
    /// The node has no resolvable `x` / `y` / `w` / `h`.
    Geometry,
    /// The px origin of this container does not resolve.
    Origin(String),
}

impl BoxError {
    /// A message tail: what does not resolve.
    pub(super) fn reason(&self) -> String {
        match self {
            BoxError::Geometry => "has no resolvable x/y/w/h geometry".to_owned(),
            BoxError::Origin(c) => {
                format!("sits in container {c:?}, whose px origin does not resolve")
            }
        }
    }
}

/// The box of `placed` in the space after its first `skip` chain links.
/// `skip` 0 gives page space.
pub(super) fn space_box(placed: &Placed, skip: usize) -> Result<SpaceBox, BoxError> {
    let (x, y, w, h) = placed.local.ok_or(BoxError::Geometry)?;
    let origin = chain_origin(&placed.chain, skip).map_err(BoxError::Origin)?;
    Ok(SpaceBox {
        id: placed.id.clone(),
        x: x + origin.0,
        y: y + origin.1,
        w,
        h,
        origin,
    })
}

/// Write shared-space `x` / `y` onto a node, converted into its parent's
/// space. `None` when the node is missing, `Some(false)` when it has no
/// position slots.
pub(super) fn write_space_xy(
    doc: &mut Document,
    space_box: &SpaceBox,
    new_x: Option<f64>,
    new_y: Option<f64>,
) -> Option<bool> {
    let node = find_node_any_mut(doc, &space_box.id)?;
    Some(write_node_xy(
        node,
        new_x.map(|v| v - space_box.origin.0),
        new_y.map(|v| v - space_box.origin.1),
    ))
}
