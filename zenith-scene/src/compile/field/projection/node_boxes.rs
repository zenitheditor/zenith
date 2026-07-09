//! Per-page `node id → ABSOLUTE bounding box` collection for text-runaround
//! exclusion lookup.

use std::collections::BTreeMap;

use zenith_core::{
    ComponentDef, InstanceNode, Node, Page, PropertyValue, ResolvedToken, dim_to_px,
};

use super::common::{node_id, resolve_imported_component};
use crate::compile::container::prefix_ids_in_children;
use crate::compile::imports::ImportScopes;
use crate::compile::util::resolve_geometry_px;

/// Build a single page's `node id → ABSOLUTE bounding box (x, y, w, h)` map in
/// pixels for text-runaround exclusion lookup.
///
/// Walks the page's children recursively, accumulating the translation offset of
/// each ancestor container: a `group` (and an `instance`, which compiles as a
/// translated synthetic group) shifts its children by its own `x`/`y`; a `frame`
/// is clip-only and does NOT translate (matching the render-offset semantics in
/// [`crate::compile::container`]). A node's absolute box is `(dx + node_x, dy +
/// node_y, node_w, node_h)`. Only nodes whose x/y/w/h ALL resolve to pixels are
/// recorded (a node without a complete rect — `line`/`polygon`/`polyline`, or
/// any node missing a dimension — is skipped: it cannot serve as a rectangular
/// exclusion). Deterministic: source-order walk; the FIRST occurrence of an id
/// wins.
pub(in crate::compile) fn build_node_boxes(
    page: &Page,
    resolved: &BTreeMap<String, ResolvedToken>,
    components: &BTreeMap<&str, &ComponentDef>,
    imports: &ImportScopes<'_>,
) -> BTreeMap<String, (f64, f64, f64, f64)> {
    let mut map: BTreeMap<String, (f64, f64, f64, f64)> = BTreeMap::new();
    collect_node_boxes(
        &page.children,
        0.0,
        0.0,
        resolved,
        components,
        imports,
        &mut map,
    );
    map
}

/// Recursive worker for [`build_node_boxes`]. `dx`/`dy` are the accumulated
/// ancestor translation in pixels.
fn collect_node_boxes(
    children: &[Node],
    dx: f64,
    dy: f64,
    resolved: &BTreeMap<String, ResolvedToken>,
    components: &BTreeMap<&str, &ComponentDef>,
    imports: &ImportScopes<'_>,
    map: &mut BTreeMap<String, (f64, f64, f64, f64)>,
) {
    for child in children {
        if let Some(id) = node_id(child)
            && let Some((x, y, w, h)) = node_rect(child, resolved)
        {
            map.entry(id.to_owned()).or_insert((dx + x, dy + y, w, h));
        }
        match child {
            // A frame is clip-only: its children are NOT translated by its origin.
            Node::Frame(f) => {
                collect_node_boxes(&f.children, dx, dy, resolved, components, imports, map);
            }
            // A group translates its children by its own x/y (absent/bad-unit → 0).
            Node::Group(g) => {
                let gx = resolve_geometry_px(g.x.as_ref(), resolved);
                let gy = resolve_geometry_px(g.y.as_ref(), resolved);
                collect_node_boxes(
                    &g.children,
                    dx + gx.unwrap_or(0.0),
                    dy + gy.unwrap_or(0.0),
                    resolved,
                    components,
                    imports,
                    map,
                );
            }
            Node::Instance(i) => {
                if let Some(source) = i.source.as_deref() {
                    collect_imported_instance_node_boxes(i, source, dx, dy, imports, map);
                    continue;
                }
                if let Some(component_id) = i.component.as_deref()
                    && let Some(component) = components.get(component_id)
                {
                    let mut children = component.children.clone();
                    let prefix = format!("{}/", i.id);
                    prefix_ids_in_children(&mut children, &prefix);
                    let ix = i
                        .x
                        .as_ref()
                        .and_then(|d| dim_to_px(d.value, &d.unit))
                        .unwrap_or(0.0);
                    let iy = i
                        .y
                        .as_ref()
                        .and_then(|d| dim_to_px(d.value, &d.unit))
                        .unwrap_or(0.0);
                    collect_node_boxes(
                        &children,
                        dx + ix,
                        dy + iy,
                        resolved,
                        components,
                        imports,
                        map,
                    );
                }
            }
            // A table records its OWN box (above); its cell content is
            // translated at render time, so cell children are not added to the
            // authored-coordinate exclusion map in this unit.
            Node::Table(_)
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
            | Node::Toc(_)
            | Node::Footnote(_)
            | Node::Shape(_)
            // A connector has no authored box (its endpoints are derived from
            // its targets' boxes), so it contributes nothing to the box map.
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Unknown(_) => {}
        }
    }
}

fn collect_imported_instance_node_boxes(
    instance: &InstanceNode,
    source: &str,
    dx: f64,
    dy: f64,
    imports: &ImportScopes<'_>,
    map: &mut BTreeMap<String, (f64, f64, f64, f64)>,
) {
    let Some((imported, component)) = resolve_imported_component(source, imports) else {
        return;
    };

    let mut children = component.children.clone();
    let prefix = format!("{}/", instance.id);
    prefix_ids_in_children(&mut children, &prefix);
    let ix = instance
        .x
        .as_ref()
        .and_then(|d| dim_to_px(d.value, &d.unit))
        .unwrap_or(0.0);
    let iy = instance
        .y
        .as_ref()
        .and_then(|d| dim_to_px(d.value, &d.unit))
        .unwrap_or(0.0);
    collect_node_boxes(
        &children,
        dx + ix,
        dy + iy,
        &imported.resolved,
        &imported.components,
        imports,
        map,
    );
}

/// A node's LOCAL `(x, y, w, h)` rectangle in pixels, when all four resolve.
///
/// Returns `None` for a node kind without a rectangular box (`line`/`polygon`/
/// `polyline`/`footnote`/`unknown`) or one missing any of x/y/w/h.
fn node_rect(
    node: &Node,
    resolved: &BTreeMap<String, ResolvedToken>,
) -> Option<(f64, f64, f64, f64)> {
    let rect = |x: &Option<PropertyValue>,
                y: &Option<PropertyValue>,
                w: &Option<PropertyValue>,
                h: &Option<PropertyValue>|
     -> Option<(f64, f64, f64, f64)> {
        let x = resolve_geometry_px(x.as_ref(), resolved)?;
        let y = resolve_geometry_px(y.as_ref(), resolved)?;
        let w = resolve_geometry_px(w.as_ref(), resolved)?;
        let h = resolve_geometry_px(h.as_ref(), resolved)?;
        Some((x, y, w, h))
    };
    match node {
        Node::Rect(n) => rect(&n.x, &n.y, &n.w, &n.h),
        Node::Ellipse(n) => rect(&n.x, &n.y, &n.w, &n.h),
        Node::Text(n) => rect(&n.x, &n.y, &n.w, &n.h),
        Node::Code(n) => rect(&n.x, &n.y, &n.w, &n.h),
        Node::Frame(n) => rect(&n.x, &n.y, &n.w, &n.h),
        Node::Group(n) => rect(&n.x, &n.y, &n.w, &n.h),
        Node::Image(n) => rect(&n.x, &n.y, &n.w, &n.h),
        Node::Field(n) => rect(&n.x, &n.y, &n.w, &n.h),
        Node::Toc(n) => rect(&n.x, &n.y, &n.w, &n.h),
        Node::Table(n) => rect(&n.x, &n.y, &n.w, &n.h),
        Node::Shape(n) => rect(&n.x, &n.y, &n.w, &n.h),
        Node::Pattern(n) => rect(&n.x, &n.y, &n.w, &n.h),
        Node::Chart(n) => rect(&n.x, &n.y, &n.w, &n.h),
        Node::Light(_) => None,
        Node::Mesh(n) => rect(&n.x, &n.y, &n.w, &n.h),
        // An `instance` has no intrinsic w/h (its box is the expanded subtree),
        // and line/polygon/polyline have no rectangular box — none can serve as a
        // rectangular exclusion, so they are skipped.
        Node::Instance(_)
        | Node::Line(_)
        | Node::Polygon(_)
        | Node::Polyline(_)
        | Node::Path(_)
        | Node::Footnote(_)
        | Node::Connector(_)
        | Node::Unknown(_) => None,
    }
}
