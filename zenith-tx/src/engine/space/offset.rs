//! The px offset a container adds to its children, read from authored
//! values the way the scene reads them.
//!
//! - A group translates by its `x` / `y`, 0 when absent or not px. An
//!   `anchor` on a group does not move its children.
//! - A frame translates by its px `x` / `y`. A layout frame placed by an
//!   anchor takes each missing axis from the anchor, as auto-layout
//!   lowering derives it.

use std::collections::{BTreeMap, BTreeSet};

use zenith_core::{
    AnchorRefs, AnchorSiblings, Dimension, FrameNode, LayoutKind, Node, Page, PropertyValue,
    ResolvedToken, derive_anchor_origin, dim_to_px, resolve_geometry_px,
};

use super::super::layout::places_in_flow;

/// Where a child list sits, for reading the offsets of its containers.
#[derive(Clone, Copy)]
pub(in crate::engine) struct Scope<'d> {
    /// The child list.
    pub siblings: &'d [Node],
    /// The frame that holds the list, if any.
    pub parent: Option<&'d FrameNode>,
    /// The px size of the anchor-parent box of the list's container, when it
    /// resolves.
    pub parent_size: Option<(f64, f64)>,
    /// The page the list renders on. `None` for master chrome.
    pub page: Option<&'d Page>,
    /// The page-space origin of the list's space, when it resolves.
    pub acc: Option<(f64, f64)>,
    pub resolved: &'d BTreeMap<String, ResolvedToken>,
}

impl<'d> Scope<'d> {
    /// The scope of `children`, held by a container that adds `offset`.
    pub fn enter(
        &self,
        children: &'d [Node],
        parent: Option<&'d FrameNode>,
        parent_size: Option<(f64, f64)>,
        offset: Option<(f64, f64)>,
    ) -> Scope<'d> {
        let acc = self
            .acc
            .zip(offset)
            .map(|((ax, ay), (dx, dy))| (ax + dx, ay + dy));
        Scope {
            siblings: children,
            parent,
            parent_size,
            page: self.page,
            acc,
            resolved: self.resolved,
        }
    }

    fn px(&self, v: Option<&PropertyValue>) -> Option<f64> {
        resolve_geometry_px(v, self.resolved)
    }
}

/// The px offset `node` adds to its children, or `None` when its own
/// placement does not resolve.
///
/// `None` when the parent frame places the node in flow, when the node has a
/// non-zero `rotate` (a shift does not map a rotated space), and for a frame
/// whose `x` / `y` neither resolve to px nor derive from an anchor. A node
/// without a child space gives `(0, 0)`.
pub(in crate::engine) fn own_offset(node: &Node, scope: &Scope<'_>) -> Option<(f64, f64)> {
    if scope.parent.is_some_and(|f| places_in_flow(f, node)) {
        return None;
    }
    let unrotated = |r: Option<&Dimension>| r.is_none_or(|d| d.value == 0.0);
    match node {
        Node::Frame(f) => {
            if !unrotated(f.rotate.as_ref()) {
                return None;
            }
            let (x, y) = (scope.px(f.x.as_ref()), scope.px(f.y.as_ref()));
            if let (Some(x), Some(y)) = (x, y) {
                return Some((x, y));
            }
            // Lowering places an anchored layout frame: each missing axis
            // comes from the anchor at the frame's fixed size.
            let anchored = node.box_view().is_some_and(|v| v.anchored);
            if !(is_layout_root(f) && anchored) {
                return None;
            }
            let size = fixed_size(f, scope)?;
            let (ax, ay) = anchor_origin(node, size, scope, &mut BTreeSet::new())?;
            Some((x.unwrap_or(ax), y.unwrap_or(ay)))
        }
        Node::Group(g) => unrotated(g.rotate.as_ref()).then(|| g.child_space(scope.resolved)),
        Node::Instance(_)
        | Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Line(_)
        | Node::Text(_)
        | Node::Code(_)
        | Node::Image(_)
        | Node::Polygon(_)
        | Node::Polyline(_)
        | Node::Path(_)
        | Node::Field(_)
        | Node::Footnote(_)
        | Node::Toc(_)
        | Node::Table(_)
        | Node::Shape(_)
        | Node::Connector(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Mesh(_)
        | Node::Unknown(_) => Some(node.child_space(scope.resolved).unwrap_or((0.0, 0.0))),
    }
}

/// `true` for a `row` / `column` / `grid` frame.
fn is_layout_root(f: &FrameNode) -> bool {
    match f.layout.as_ref() {
        Some(LayoutKind::Row | LayoutKind::Column | LayoutKind::Grid) => true,
        Some(LayoutKind::Absolute | LayoutKind::Unknown(_)) | None => false,
    }
}

/// The size lowering gives an anchored layout frame, when authored data
/// fixes it: px `w` and `h`, with no hug / fill keyword and no min / max.
/// A hugging frame measures its content, which needs the scene.
fn fixed_size(f: &FrameNode, scope: &Scope<'_>) -> Option<(f64, f64)> {
    let item = &f.layout_item;
    let free = item.w_keyword.is_none()
        && item.h_keyword.is_none()
        && item.min_w.is_none()
        && item.max_w.is_none()
        && item.min_h.is_none()
        && item.max_h.is_none();
    free.then(|| scope.px(f.w.as_ref()).zip(scope.px(f.h.as_ref())))
        .flatten()
}

/// The anchor origin of `node`, a member of `scope`, at its authored px
/// size, in the space of `scope`. `None` when the parent frame places it
/// in flow, its size does not resolve to px (a layout frame needs a fixed
/// size), or the anchor origin does not derive.
pub(in crate::engine) fn placed_anchor_origin(
    node: &Node,
    scope: &Scope<'_>,
) -> Option<(f64, f64)> {
    if scope.parent.is_some_and(|f| places_in_flow(f, node)) {
        return None;
    }
    let size = if let Node::Frame(f) = node
        && is_layout_root(f)
    {
        fixed_size(f, scope)?
    } else {
        let view = node.anchor_view()?;
        scope.px(view.w).zip(scope.px(view.h))?
    };
    anchor_origin(node, size, scope, &mut BTreeSet::new())
}

/// The anchor origin of `node` at `size` in the space of `scope`, as the
/// scene derives it ([`derive_anchor_origin`]). `None` when it does not
/// derive from authored data.
///
/// `visiting` holds the sibling ids whose origin this derivation already
/// asked for. It stops a sibling cycle.
fn anchor_origin(
    node: &Node,
    size: (f64, f64),
    scope: &Scope<'_>,
    visiting: &mut BTreeSet<String>,
) -> Option<(f64, f64)> {
    let view = node.anchor_view()?;
    let page = scope.page.and_then(|p| {
        let px = |d: &Dimension| dim_to_px(d.value, &d.unit);
        px(&p.width).zip(px(&p.height))
    });
    // The parent box starts at the origin of the parent's child space.
    let parent_box = scope
        .parent_size
        .zip(scope.acc)
        .map(|((w, h), (x, y))| (x, y, w, h));
    let refs = AnchorRefs {
        page,
        safe_zones: scope.page.map_or(&[], |p| p.safe_zones.as_slice()),
        parent_box,
        origin: scope.acc,
        resolved: scope.resolved,
    };
    let mut siblings = Siblings { scope, visiting };
    derive_anchor_origin(&view, size, refs, &mut siblings)
}

/// The siblings of a scope, read the way lowering reads them.
struct Siblings<'a, 'd> {
    scope: &'a Scope<'d>,
    visiting: &'a mut BTreeSet<String>,
}

impl AnchorSiblings for Siblings<'_, '_> {
    fn sibling(&self, id: &str) -> Option<&Node> {
        find_sibling(self.scope, id)
    }

    /// The sibling's own anchor origin. Lowering anchors a layout frame
    /// sibling at its measured size, so it resolves only at a fixed size.
    fn sibling_origin(&mut self, id: &str, size: (f64, f64)) -> Option<(f64, f64)> {
        if !self.visiting.insert(id.to_owned()) {
            return None;
        }
        let node = find_sibling(self.scope, id)?;
        if let Node::Frame(f) = node
            && is_layout_root(f)
        {
            fixed_size(f, self.scope)?;
        }
        anchor_origin(node, size, self.scope, self.visiting)
    }
}

/// The first node with `id` in `scope`, as lowering indexes a scope. A
/// sibling the parent frame places in flow has no authored box, so it gives
/// `None`.
fn find_sibling<'d>(scope: &Scope<'d>, id: &str) -> Option<&'d Node> {
    let node = scope.siblings.iter().find(|n| n.id() == Some(id))?;
    let in_flow = scope.parent.is_some_and(|f| places_in_flow(f, node));
    (!in_flow).then_some(node)
}
