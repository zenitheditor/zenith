//! 9-point anchor pre-pass supporting page-relative, safe-zone-relative,
//! parent-container-relative, and sibling-relative anchoring.
//!
//! A node may carry `anchor="<name>"` where name is one of the nine positions:
//! `top-left`, `top-center`, `top-right`, `center-left`, `center`,
//! `center-right`, `bottom-left`, `bottom-center`, `bottom-right`. When present
//! and recognized, the compile step derives the node's x and/or y from a
//! reference rectangle and the node's resolved w/h. An explicitly-authored x or
//! y always wins over the anchor-derived value.
//!
//! The derivation itself is [`zenith_core::derive_anchor_origin`], shared
//! with tx. This module walks the page and supplies its reference boxes.
//!
//! **Page-relative:** reference rectangle is the full page.
//!
//! Page-, zone-, and parent-relative entries stay page-absolute at any
//! depth: each subtracts the summed child spaces of the enclosing frames,
//! groups, and instances (a frame's child space is its `x` / `y`).
//!
//! **Safe-zone-relative:** when the node also carries
//! `anchor-zone="<id>"` and a safe-zone with that id is declared on the same
//! page, the reference rectangle is that zone's rect instead of the page.
//! Unrecognized zone ids and non-px zone dimensions silently fall back to no
//! anchor entry (the validator emits `anchor.unresolved_zone`).
//!
//! **Parent-relative:** when the node carries `anchor-parent="true"`
//! (and NOT `anchor-zone`, which takes precedence), the reference rectangle is
//! its DIRECT PARENT CONTAINER's box (a `frame` or `group`). The pre-pass
//! recurses into frame/group children, threading the parent box and the
//! cumulative container translation so the stored value cancels the
//! `ctx.dx`/`ctx.dy` that the leaf compiler re-applies.
//!
//! **Sibling-relative:** when the node carries `anchor-sibling="<id>"`
//! (and NOT `anchor-zone`, which takes precedence), the reference rectangle is
//! the resolved box of the named sibling in the SAME scope (same direct
//! parent's children). Because node and sibling share the same accumulated
//! container translation, this derivation is purely local — no `acc` term is added
//! or subtracted. Each scope is processed in sibling-dependency (topological)
//! order so a referenced sibling's entry exists before its dependent derives.
//!
//! ## Pre-pass
//!
//! [`build_anchor_map`] is called once per page compile, AFTER `page_w`/
//! `page_h` are resolved, and walks the page tree, descending into `frame` and
//! `group` containers (only those two are anchor-parent containers). For
//! each node that carries a recognized anchor AND has both `w` and `h` in a
//! px-convertible unit, the map stores the derived `(x, y)` pair keyed by node
//! id.
//!
//! ## Leaf application
//!
//! Each leaf compiler (`compile_rect`, `compile_ellipse`, etc.) receives the
//! `AnchorMap` by reference. When the node's own `x` is `None`, the compiler
//! looks up the node id in the map and, if found, uses the pre-derived x
//! (adding the usual `ctx.dx` translation). When `x` is `Some`, it is used
//! as-is (explicit wins). Same for y.

use std::collections::{BTreeMap, BTreeSet};

use zenith_core::{
    AnchorRefs, AnchorSiblings, AnchorView, Node, Page, PropertyValue, ResolvedToken, SafeZone,
    derive_anchor_origin,
};

use super::util::resolve_geometry_px;

/// Pre-derived anchor coordinates keyed by node id.
///
/// A node appears in this map if and only if it carries a recognized anchor
/// value AND its `w` and `h` both resolved to px. The stored pair is the raw
/// coordinate `(x, y)` BEFORE the `ctx.dx`/`ctx.dy` container-translation
/// offset is applied by the leaf compiler; the parent-, zone-, and
/// page-relative derivations pre-subtract the accumulated translation so
/// adding `ctx.dx`/`ctx.dy` lands the node at the intended device position.
pub(crate) type AnchorMap = BTreeMap<String, (f64, f64)>;

/// Walk-wide immutable pre-pass environment (page dims + zone table + token
/// table). The token table resolves geometry token refs (`(token)"dim.h"`) on
/// box nodes during anchor derivation. The auto-layout lowering derives the
/// origin of an anchored layout frame with the same environment.
#[derive(Clone, Copy)]
pub(crate) struct PrePassEnv<'a> {
    pub(crate) page_w: f64,
    pub(crate) page_h: f64,
    pub(crate) safe_zones: &'a [SafeZone],
    pub(crate) resolved: &'a BTreeMap<String, ResolvedToken>,
}

impl<'a> PrePassEnv<'a> {
    /// The reference boxes of a scope with container context `ctx`.
    fn refs(self, ctx: ParentCtx) -> AnchorRefs<'a> {
        AnchorRefs {
            page: Some((self.page_w, self.page_h)),
            safe_zones: self.safe_zones,
            parent_box: ctx.parent_box,
            origin: Some((ctx.acc_dx, ctx.acc_dy)),
            resolved: self.resolved,
        }
    }
}

/// Per-recursion container context for parent-relative derivation.
///
/// `parent_box` = `Some((ref_x, ref_y, ref_w, ref_h))` is the enclosing
/// container's reference rectangle, or `None` at the page root (and when a
/// container box is unresolvable). `acc_dx`/`acc_dy` is the cumulative
/// container translation (summed child spaces) that will be active as
/// `ctx.dx`/`ctx.dy` when the current node compiles; the parent-, zone-, and
/// page-relative derivations subtract it so the leaf's re-add cancels to the
/// intended device coordinate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ParentCtx {
    pub(crate) parent_box: Option<(f64, f64, f64, f64)>,
    pub(crate) acc_dx: f64,
    pub(crate) acc_dy: f64,
}

impl ParentCtx {
    pub(crate) const ROOT: ParentCtx = ParentCtx {
        parent_box: None,
        acc_dx: 0.0,
        acc_dy: 0.0,
    };
}

/// Walk the page tree and build the [`AnchorMap`].
///
/// Top-level nodes resolve page/zone-relative anchors. Frame and
/// group children additionally resolve parent-relative anchors against
/// their enclosing container's box. Only nodes with a recognized anchor,
/// present `w`/`h`, and px-convertible `w`/`h` produce entries; all others are
/// absent (byte-identical to before for any node not using anchor-parent).
pub(crate) fn build_anchor_map(
    page: &Page,
    page_w: f64,
    page_h: f64,
    resolved: &BTreeMap<String, ResolvedToken>,
) -> AnchorMap {
    let env = PrePassEnv {
        page_w,
        page_h,
        safe_zones: &page.safe_zones,
        resolved,
    };
    let mut map = AnchorMap::new();
    // The page-children form one sibling scope; process them in sibling-
    // dependency order so a node referencing an earlier-resolved sibling sees
    // that sibling's entry already in the map.
    let scope: BTreeMap<&str, &Node> = page
        .children
        .iter()
        .filter_map(|n| n.anchor_view().map(|f| (f.id, n)))
        .collect();
    for node in sibling_topo_order(&page.children) {
        collect_anchor(node, env, ParentCtx::ROOT, &scope, &mut map);
    }
    map
}

/// Order `children` so that any node carrying `anchor-sibling="<id>"` (where
/// `<id>` is an in-scope anchor-bearing sibling) is processed AFTER that
/// sibling. Uses Kahn's algorithm over the in-scope sibling-dependency graph
/// (edge: target → dependent). Anchor-bearing nodes with no in-scope sibling
/// dependency are emitted in sorted-id order (the ready-set is a `BTreeSet`);
/// their derivations are mutually independent, so the resulting anchor map is
/// identical regardless of their relative order. Non-anchor-bearing kinds, and
/// nodes left in a dependency cycle (nonzero in-degree after Kahn), are appended
/// at the end in source order; cyclic nodes naturally fail to resolve (the
/// validator reports the cycle separately). Deterministic throughout via
/// `BTreeSet`/`BTreeMap`.
fn sibling_topo_order(children: &[Node]) -> Vec<&Node> {
    // In-scope anchor-bearing ids, plus a quick id → node lookup.
    let mut by_id: BTreeMap<&str, &Node> = BTreeMap::new();
    for node in children {
        if let Some(f) = node.anchor_view() {
            by_id.insert(f.id, node);
        }
    }

    // in_degree[id] = number of in-scope sibling targets `id` depends on (0 or
    // 1, since a node carries at most one anchor-sibling). adjacency[target] =
    // the dependents that reference `target`.
    let mut in_degree: BTreeMap<&str, usize> = BTreeMap::new();
    let mut adjacency: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (&id, node) in &by_id {
        in_degree.entry(id).or_insert(0);
        if let Some(f) = node.anchor_view()
            && let Some(target) = f.anchor_sibling
            && target != id
            && by_id.contains_key(target)
        {
            adjacency.entry(target).or_default().insert(id);
            *in_degree.entry(id).or_insert(0) += 1;
        }
    }

    // Kahn: seed the ready-set with all zero-in-degree ids (sorted), emit, and
    // decrement dependents. A single pass; no recursion, no unbounded loop.
    let mut ready: BTreeSet<&str> = in_degree
        .iter()
        .filter_map(|(&id, &deg)| (deg == 0).then_some(id))
        .collect();
    // `emitted` retains Kahn's dequeue ORDER (a target is always dequeued before
    // its dependents), which is the topological order we must process in.
    let mut emitted: Vec<&str> = Vec::with_capacity(by_id.len());
    while let Some(&id) = ready.first() {
        ready.remove(id);
        emitted.push(id);
        if let Some(deps) = adjacency.get(id) {
            for &dep in deps {
                if let Some(deg) = in_degree.get_mut(dep) {
                    *deg = deg.saturating_sub(1);
                    if *deg == 0 {
                        ready.insert(dep);
                    }
                }
            }
        }
    }

    // Emit anchor-bearing nodes in topological (Kahn dequeue) order, then append
    // any node not emitted — non-anchor-bearing kinds and cycle members — in
    // source order. When no node has an in-scope anchor-sibling, every node is
    // zero-in-degree and dequeued in sorted id order; the source-order tail then
    // contributes nothing new, so the order is a stable permutation that still
    // honours dependencies. Independent equal-rank nodes resolve identically
    // regardless of order (their entries don't depend on each other).
    let mut order: Vec<&Node> = Vec::with_capacity(children.len());
    let mut placed: BTreeSet<&str> = BTreeSet::new();
    for &id in &emitted {
        if let Some(&node) = by_id.get(id)
            && placed.insert(id)
        {
            order.push(node);
        }
    }
    for node in children {
        match node.anchor_view() {
            Some(f) if placed.contains(f.id) => {}
            _ => order.push(node),
        }
    }
    order
}

/// The `anchor-sibling` target id of `node`, when it names one.
pub(crate) fn anchor_sibling_of(node: &Node) -> Option<&str> {
    node.anchor_view().and_then(|f| f.anchor_sibling)
}

/// Resolve the px box `(x, y, w, h)` of a node from its four geometry
/// properties, returning `None` when any of the four is absent, a non-dimension,
/// an unresolved token, or carries a non-px unit. Raw `(px)` dims are
/// byte-identical to the prior `dim_to_px` read; dimension token refs resolve
/// via the token table.
fn px_box(
    x: Option<&PropertyValue>,
    y: Option<&PropertyValue>,
    w: Option<&PropertyValue>,
    h: Option<&PropertyValue>,
    resolved: &BTreeMap<String, ResolvedToken>,
) -> Option<(f64, f64, f64, f64)> {
    let x = resolve_geometry_px(x, resolved)?;
    let y = resolve_geometry_px(y, resolved)?;
    let w = resolve_geometry_px(w, resolved)?;
    let h = resolve_geometry_px(h, resolved)?;
    Some((x, y, w, h))
}

/// Try to build an anchor map entry for a single node, then recurse into
/// `frame`/`group` containers carrying their box as the parent reference for
/// anchor-parent children.
fn collect_anchor(
    node: &Node,
    env: PrePassEnv,
    ctx: ParentCtx,
    scope: &BTreeMap<&str, &Node>,
    map: &mut AnchorMap,
) {
    if let Some(fields) = node.anchor_view() {
        derive_entry(fields, env, ctx, scope, map);
    }

    // Recurse into the two anchor-parent containers: frame and group. Each
    // adds its child space (`Node::child_space`) to the accumulated
    // translation of its children. Other node kinds are leaves for anchor
    // purposes.
    match node {
        Node::Frame(frame) => {
            // A frame draws at its own x/y plus the inherited translation.
            let frame_box = px_box(
                frame.x.as_ref(),
                frame.y.as_ref(),
                frame.w.as_ref(),
                frame.h.as_ref(),
                env.resolved,
            )
            .map(|(x, y, w, h)| (x + ctx.acc_dx, y + ctx.acc_dy, w, h));
            // Children count from the frame's top-left.
            let (sx, sy) = frame.child_space(env.resolved);
            let child_ctx = ParentCtx {
                parent_box: frame_box,
                acc_dx: ctx.acc_dx + sx,
                acc_dy: ctx.acc_dy + sy,
            };
            // The frame's direct children form a new sibling scope.
            let child_scope: BTreeMap<&str, &Node> = frame
                .children
                .iter()
                .filter_map(|n| n.anchor_view().map(|f| (f.id, n)))
                .collect();
            for child in sibling_topo_order(&frame.children) {
                collect_anchor(child, env, child_ctx, &child_scope, map);
            }
        }
        Node::Group(group) => {
            // Group translates children by its child space (its x/y, 0 when
            // absent or non-px). The child's compile context acc becomes
            // acc + group_x.
            let (group_x, group_y) = group.child_space(env.resolved);
            let child_dx = ctx.acc_dx + group_x;
            let child_dy = ctx.acc_dy + group_y;
            // The group reference box origin is its device origin (child_dx,
            // child_dy); width/height come from the declared w/h. When either w
            // or h is absent/non-px the box is unknown → no parent-relative entry
            // for the group's children (validator flags it).
            let group_box = resolve_geometry_px(group.w.as_ref(), env.resolved)
                .zip(resolve_geometry_px(group.h.as_ref(), env.resolved))
                .map(|(gw, gh)| (child_dx, child_dy, gw, gh));
            let child_ctx = ParentCtx {
                parent_box: group_box,
                acc_dx: child_dx,
                acc_dy: child_dy,
            };
            // The group's direct children form a new sibling scope.
            let child_scope: BTreeMap<&str, &Node> = group
                .children
                .iter()
                .filter_map(|n| n.anchor_view().map(|f| (f.id, n)))
                .collect();
            for child in sibling_topo_order(&group.children) {
                collect_anchor(child, env, child_ctx, &child_scope, map);
            }
        }
        // Every other node kind is a leaf for anchor pre-pass purposes.
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Line(_)
        | Node::Text(_)
        | Node::Code(_)
        | Node::Image(_)
        | Node::Shape(_)
        | Node::Polygon(_)
        | Node::Polyline(_)
        | Node::Path(_)
        | Node::Connector(_)
        | Node::Instance(_)
        | Node::Field(_)
        | Node::Toc(_)
        | Node::Footnote(_)
        | Node::Table(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Mesh(_)
        | Node::Unknown(_) => {}
    }
}

/// Derive and insert the anchor map entry for one node from its fields.
///
/// The node's `w` and `h` must both resolve to px; otherwise no entry.
fn derive_entry(
    fields: AnchorView<'_>,
    env: PrePassEnv,
    ctx: ParentCtx,
    scope: &BTreeMap<&str, &Node>,
    map: &mut AnchorMap,
) {
    // Both w and h must be present and px-convertible for derivation. Raw `(px)`
    // dims and dimension token refs both resolve via the token table.
    let (Some(node_w), Some(node_h)) = (
        resolve_geometry_px(fields.w, env.resolved),
        resolve_geometry_px(fields.h, env.resolved),
    ) else {
        return;
    };
    let mut siblings = Siblings {
        lookup: &|sib| scope.get(sib).copied(),
        map,
    };
    let xy = derive_anchor_origin(&fields, (node_w, node_h), env.refs(ctx), &mut siblings);
    if let Some(xy) = xy {
        map.insert(fields.id.to_owned(), xy);
    }
}

/// The anchor-derived origin of `node` at size `size`, or `None` when `node`
/// carries no resolvable anchor.
///
/// The auto-layout lowering calls this for an anchored layout frame after it
/// measures the frame, so the anchor sees the hugged size. `lookup` finds a
/// sibling in the node's scope by id. `map` holds the entries already
/// derived in that scope.
pub(crate) fn anchor_origin<'n>(
    node: &Node,
    size: (f64, f64),
    env: PrePassEnv,
    ctx: ParentCtx,
    lookup: &dyn Fn(&str) -> Option<&'n Node>,
    map: &AnchorMap,
) -> Option<(f64, f64)> {
    let fields = node.anchor_view()?;
    let mut siblings = Siblings { lookup, map };
    derive_anchor_origin(&fields, size, env.refs(ctx), &mut siblings)
}

/// The siblings of one scope: `lookup` finds a node by id, and `map` holds
/// the anchor entries already derived.
struct Siblings<'s, 'n> {
    lookup: &'s dyn Fn(&str) -> Option<&'n Node>,
    map: &'s AnchorMap,
}

impl AnchorSiblings for Siblings<'_, '_> {
    fn sibling(&self, id: &str) -> Option<&Node> {
        (self.lookup)(id)
    }

    fn sibling_origin(&mut self, id: &str, _size: (f64, f64)) -> Option<(f64, f64)> {
        self.map.get(id).copied()
    }
}
