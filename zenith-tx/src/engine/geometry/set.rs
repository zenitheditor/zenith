//! `set_geometry` application.

use zenith_core::{Diagnostic, Dimension, Document, Node, PropertyValue, Unit};

use super::super::layout::{SizeArg, parse_size_arg, reject_layout_managed, write_size_keywords};
use super::super::{find_node_any_mut, px, record_affected};
use super::boxes::node_geometry_mut;
use super::required::{Removals, reject_required_removals};
use crate::op::SizeInput;

/// Bundled geometry deltas passed to [`apply_set_geometry`].
///
/// Each field is tri-state: `None` leaves the attribute, `Some(None)` removes
/// it, `Some(Some(v))` sets it. Grouping these avoids pushing
/// `apply_set_geometry` past the `clippy::too_many_arguments` threshold
/// without using `#[allow]`.
pub(in crate::engine) struct GeometryDelta<'a> {
    pub x: Option<Option<f64>>,
    pub y: Option<Option<f64>>,
    pub w: Option<Option<&'a SizeInput>>,
    pub h: Option<Option<&'a SizeInput>>,
    pub rotate: Option<Option<f64>>,
}

/// Return a mutable reference to a node's `rotate` slot, or `None` for node
/// variants that do not carry a `rotate` field.
///
/// Supported: `Rect`, `Ellipse`, `Frame`, `Image`, `Text`, `Code`, `Group`,
/// `Polygon`, `Polyline`, `Table`, `Shape`, `Connector`.
/// Unsupported: `Line`, `Instance`, `Field`, `Toc`, `Footnote`, `Unknown`.
fn node_rotate_mut(node: &mut Node) -> Option<&mut Option<Dimension>> {
    match node {
        Node::Rect(n) => Some(&mut n.rotate),
        Node::Ellipse(n) => Some(&mut n.rotate),
        Node::Frame(n) => Some(&mut n.rotate),
        Node::Image(n) => Some(&mut n.rotate),
        Node::Text(n) => Some(&mut n.rotate),
        Node::Code(n) => Some(&mut n.rotate),
        Node::Group(n) => Some(&mut n.rotate),
        Node::Polygon(n) => Some(&mut n.rotate),
        Node::Polyline(n) => Some(&mut n.rotate),
        Node::Path(n) => Some(&mut n.rotate),
        Node::Table(n) => Some(&mut n.rotate),
        Node::Shape(n) => Some(&mut n.rotate),
        Node::Connector(n) => Some(&mut n.rotate),
        Node::Pattern(n) => Some(&mut n.rotate),
        Node::Chart(n) => Some(&mut n.rotate),
        Node::Mesh(_) => None,
        Node::Light(_) => None,
        // Line has no rotate field.
        // Instance has no rotate field.
        // Field has no rotate field.
        // Toc has no rotate field.
        // Footnote has no rotate field.
        // Unknown has no rotate field.
        Node::Line(_)
        | Node::Instance(_)
        | Node::Field(_)
        | Node::Toc(_)
        | Node::Footnote(_)
        | Node::Unknown(_) => None,
    }
}

pub(in crate::engine) fn apply_set_geometry(
    node_id: &str,
    delta: GeometryDelta<'_>,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    let GeometryDelta { x, y, w, h, rotate } = delta;

    // Early-out: if every field is None this is a no-op — emit advisory.
    if x.is_none() && y.is_none() && w.is_none() && h.is_none() && rotate.is_none() {
        diagnostics.push(Diagnostic::advisory(
            "tx.noop",
            format!(
                "set_geometry on {:?} specified no fields; document is unchanged",
                node_id
            ),
            None,
            Some(node_id.to_owned()),
        ));
        return;
    }
    let (Ok(w), Ok(h)) = (
        parse_size_arg(w, "w", node_id, diagnostics),
        parse_size_arg(h, "h", node_id, diagnostics),
    ) else {
        return;
    };
    // Only a write places the node by hand. Removing a stale x/y from an
    // in-flow child is the cleanup that `layout.position_ignored` asks for.
    if (matches!(x, Some(Some(_))) || matches!(y, Some(Some(_))))
        && reject_layout_managed(doc, [node_id], "set_geometry", diagnostics)
    {
        return;
    }
    let removals = Removals {
        x: x == Some(None),
        y: y == Some(None),
        w: w == Some(SizeArg::Remove),
        h: h == Some(SizeArg::Remove),
    };
    if reject_required_removals(doc, node_id, removals, diagnostics) {
        return;
    }

    match find_node_any_mut(doc, node_id) {
        None => {
            diagnostics.push(Diagnostic::error(
                "tx.unknown_node",
                format!("node {:?} not found in document", node_id),
                None,
                Some(node_id.to_owned()),
            ));
        }
        Some(node) => {
            let kind = node.kind_str();

            // Apply x/y/w/h only when the node supports bbox geometry.
            // When x/y/w/h are all None but rotate is Some, we still proceed
            // (no unsupported_property for the geometry side).
            let has_geom_delta = x.is_some() || y.is_some() || w.is_some() || h.is_some();
            if has_geom_delta {
                // Instance stores placement as Option<Dimension>, not PropertyValue.
                if let Node::Instance(inst) = node {
                    if let Some(v) = x {
                        inst.x = v.map(px);
                    }
                    if let Some(v) = y {
                        inst.y = v.map(px);
                    }
                    if let Some(v) = w {
                        inst.w = size_dimension(v);
                    }
                    if let Some(v) = h {
                        inst.h = size_dimension(v);
                    }
                    if rotate.is_some() {
                        diagnostics.push(Diagnostic::error(
                            "tx.unsupported_property",
                            "set_geometry: rotate is not supported on instance nodes".to_owned(),
                            None,
                            Some(node_id.to_owned()),
                        ));
                        return;
                    }
                    write_size_keywords(node, w, h);
                    record_affected(node_id, affected);
                    return;
                }

                match node_geometry_mut(node) {
                    None => {
                        diagnostics.push(Diagnostic::error(
                            "tx.unsupported_property",
                            format!(
                                "set_geometry is not supported on a {} node (no x/y/w/h)",
                                kind
                            ),
                            None,
                            Some(node_id.to_owned()),
                        ));
                        return;
                    }
                    Some((nx, ny, nw, nh)) => {
                        if let Some(v) = x {
                            *nx = v.map(|v| PropertyValue::Dimension(px(v)));
                        }
                        if let Some(v) = y {
                            *ny = v.map(|v| PropertyValue::Dimension(px(v)));
                        }
                        if let Some(v) = w {
                            *nw = size_dimension(v).map(PropertyValue::Dimension);
                        }
                        if let Some(v) = h {
                            *nh = size_dimension(v).map(PropertyValue::Dimension);
                        }
                    }
                }
                write_size_keywords(node, w, h);
            }

            // Apply rotate when requested.
            if let Some(r) = rotate {
                match node_rotate_mut(node) {
                    None => {
                        diagnostics.push(Diagnostic::error(
                            "tx.unsupported_property",
                            format!("set_geometry: rotate is not supported on {} nodes", kind),
                            None,
                            Some(node_id.to_owned()),
                        ));
                        return;
                    }
                    Some(slot) => {
                        *slot = r.map(|value| Dimension {
                            value,
                            unit: Unit::Deg,
                        });
                    }
                }
            }

            record_affected(node_id, affected);
        }
    }
}

/// The px dimension a size write stores: a px size, or `None` for a keyword
/// (the keyword lives on the layout item) or a removal.
fn size_dimension(arg: SizeArg) -> Option<Dimension> {
    match arg {
        SizeArg::Px(v) => Some(px(v)),
        SizeArg::Keyword(_) | SizeArg::Remove => None,
    }
}
