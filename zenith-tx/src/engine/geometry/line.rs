//! `nudge_line_points` application: offset a line's endpoints by px deltas
//! and keep each endpoint's unit.

use zenith_core::{Diagnostic, Dimension, Document, Node};

use super::super::layout::reject_layout_managed;
use super::super::{find_node_any_mut, find_node_any_shared, record_affected};
use super::axis::{AxisCtx, dimension_px};

const OP: &str = "nudge_line_points";

/// The deltas of one `nudge_line_points` op. `None` leaves that endpoint
/// coordinate.
#[derive(Clone, Copy, Debug)]
pub(in crate::engine) struct LineDelta {
    pub dx1: Option<f64>,
    pub dy1: Option<f64>,
    pub dx2: Option<f64>,
    pub dy2: Option<f64>,
}

/// One endpoint coordinate of a line.
#[derive(Clone, Copy, Debug, PartialEq)]
enum End {
    X1,
    Y1,
    X2,
    Y2,
}

impl End {
    fn name(self) -> &'static str {
        match self {
            End::X1 => "x1",
            End::Y1 => "y1",
            End::X2 => "x2",
            End::Y2 => "y2",
        }
    }
}

pub(in crate::engine) fn apply_nudge_line_points(
    node_id: &str,
    delta: LineDelta,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    let requested: Vec<(End, f64)> = [
        (End::X1, delta.dx1),
        (End::Y1, delta.dy1),
        (End::X2, delta.dx2),
        (End::Y2, delta.dy2),
    ]
    .into_iter()
    .filter_map(|(end, d)| d.map(|d| (end, d)))
    .collect();
    if requested.is_empty() {
        diagnostics.push(Diagnostic::advisory(
            "tx.noop",
            format!("{OP} on {node_id:?} specified no deltas. The document is unchanged."),
            None,
            Some(node_id.to_owned()),
        ));
        return;
    }
    let mut bad_input = false;
    for (end, d) in &requested {
        if !d.is_finite() {
            bad_input = true;
            diagnostics.push(Diagnostic::error(
                "tx.invalid_value",
                format!(
                    "{OP}: d{} {d} on node {node_id:?} is not finite",
                    end.name()
                ),
                None,
                Some(node_id.to_owned()),
            ));
        }
    }
    if bad_input || reject_layout_managed(doc, [node_id], OP, diagnostics) {
        return;
    }
    let Some(node) = find_node_any_shared(doc, node_id) else {
        diagnostics.push(Diagnostic::error(
            "tx.unknown_node",
            format!("node {node_id:?} not found in document"),
            None,
            Some(node_id.to_owned()),
        ));
        return;
    };
    let line = match node {
        Node::Line(line) => line,
        Node::Connector(_) => {
            diagnostics.push(Diagnostic::error(
                "tx.derived_geometry",
                format!(
                    "{OP} is not supported on connector {node_id:?}: its endpoints derive \
                     from its from/to targets. Move the targets, or change from, to, \
                     from-anchor, or to-anchor."
                ),
                None,
                Some(node_id.to_owned()),
            ));
            return;
        }
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Text(_)
        | Node::Code(_)
        | Node::Frame(_)
        | Node::Group(_)
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
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Mesh(_)
        | Node::Unknown(_) => {
            diagnostics.push(Diagnostic::error(
                "tx.unsupported_property",
                format!(
                    "{OP} is not supported on {} {node_id:?}: it edits line endpoints only",
                    node.kind_str()
                ),
                None,
                Some(node_id.to_owned()),
            ));
            return;
        }
    };

    let mut writes: Vec<(End, Dimension)> = Vec::with_capacity(requested.len());
    let mut failed = false;
    for (end, d) in requested {
        let ctx = AxisCtx {
            op: OP,
            node_id,
            attr: end.name(),
        };
        let current = match end {
            End::X1 => line.x1.as_ref(),
            End::Y1 => line.y1.as_ref(),
            End::X2 => line.x2.as_ref(),
            End::Y2 => line.y2.as_ref(),
        };
        let planned = current
            .ok_or_else(|| ctx.unresolved("absent"))
            .and_then(|c| ctx.offset(c, d))
            .and_then(|value| match dimension_px(&value) {
                Some(px) if px.is_finite() => Ok(value),
                Some(_) | None => Err(ctx.error(
                    "tx.invalid_geometry",
                    format!(
                        "would become {}, which is not finite",
                        value.to_kdl_string()
                    ),
                )),
            });
        match planned {
            Ok(value) => writes.push((end, value)),
            Err(diagnostic) => {
                failed = true;
                diagnostics.push(diagnostic);
            }
        }
    }
    if failed {
        return;
    }
    if let Some(Node::Line(line)) = find_node_any_mut(doc, node_id) {
        for (end, value) in writes {
            let slot = match end {
                End::X1 => &mut line.x1,
                End::Y1 => &mut line.y1,
                End::X2 => &mut line.x2,
                End::Y2 => &mut line.y2,
            };
            *slot = Some(value);
        }
        record_affected(node_id, affected);
    }
}
