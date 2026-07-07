//! Path op application: `set_path_anchors` and `simplify_path_anchors`.

use zenith_core::{Diagnostic, Dimension, Document, Node, PathAnchor, Unit};
use zenith_geometry::{GeometryError, Point2, simplify_polyline};

use crate::op::OpPathAnchor;

use super::{find_node_any_mut, node_kind_str, px, record_affected};

pub(super) fn apply_set_path_anchors(
    node_id: &str,
    anchors: &[OpPathAnchor],
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
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
            let kind = node_kind_str(node);
            match node {
                Node::Path(path) => {
                    path.anchors = anchors
                        .iter()
                        .map(|anchor| PathAnchor {
                            x: Some(px(anchor.x)),
                            y: Some(px(anchor.y)),
                            in_x: anchor.in_x.map(px),
                            in_y: anchor.in_y.map(px),
                            out_x: anchor.out_x.map(px),
                            out_y: anchor.out_y.map(px),
                        })
                        .collect();
                    record_affected(node_id, affected);
                }
                Node::Rect(_)
                | Node::Ellipse(_)
                | Node::Line(_)
                | Node::Text(_)
                | Node::Code(_)
                | Node::Frame(_)
                | Node::Group(_)
                | Node::Image(_)
                | Node::Polygon(_)
                | Node::Polyline(_)
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
                | Node::Mesh(_)
                | Node::Unknown(_) => {
                    diagnostics.push(Diagnostic::error(
                        "tx.unsupported_property",
                        format!("set_path_anchors is not supported on a {} node", kind),
                        None,
                        Some(node_id.to_owned()),
                    ));
                }
            }
        }
    }
}

pub(super) fn apply_simplify_path_anchors(
    node_id: &str,
    tolerance: f64,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    match find_node_any_mut(doc, node_id) {
        None => diagnostics.push(unknown_node(node_id)),
        Some(node) => {
            let kind = node_kind_str(node);
            match node {
                Node::Path(path) => {
                    if path.closed == Some(true) {
                        diagnostics.push(Diagnostic::error(
                            "tx.unsupported_closed_path",
                            "simplify_path_anchors only supports open paths",
                            None,
                            Some(node_id.to_owned()),
                        ));
                        return;
                    }

                    let points = match path_points(node_id, &path.anchors) {
                        Ok(points) => points,
                        Err(diagnostic) => {
                            diagnostics.push(diagnostic);
                            return;
                        }
                    };

                    match simplify_polyline(&points, tolerance) {
                        Ok(simplified) => {
                            path.anchors = simplified
                                .iter()
                                .map(|point| PathAnchor {
                                    x: Some(px(point.x)),
                                    y: Some(px(point.y)),
                                    in_x: None,
                                    in_y: None,
                                    out_x: None,
                                    out_y: None,
                                })
                                .collect();
                            record_affected(node_id, affected);
                        }
                        Err(error) => diagnostics.push(geometry_diagnostic(node_id, error)),
                    }
                }
                Node::Rect(_)
                | Node::Ellipse(_)
                | Node::Line(_)
                | Node::Text(_)
                | Node::Code(_)
                | Node::Frame(_)
                | Node::Group(_)
                | Node::Image(_)
                | Node::Polygon(_)
                | Node::Polyline(_)
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
                | Node::Mesh(_)
                | Node::Unknown(_) => {
                    diagnostics.push(Diagnostic::error(
                        "tx.unsupported_property",
                        format!("simplify_path_anchors is not supported on a {} node", kind),
                        None,
                        Some(node_id.to_owned()),
                    ));
                }
            }
        }
    }
}

fn path_points(node_id: &str, anchors: &[PathAnchor]) -> Result<Vec<Point2>, Diagnostic> {
    let mut points = Vec::with_capacity(anchors.len());

    for anchor in anchors {
        if anchor.in_x.is_some()
            || anchor.in_y.is_some()
            || anchor.out_x.is_some()
            || anchor.out_y.is_some()
        {
            return Err(Diagnostic::error(
                "tx.unsupported_path_handles",
                "simplify_path_anchors only supports path anchors without in/out handles",
                None,
                Some(node_id.to_owned()),
            ));
        }

        let Some(x) = anchor_coordinate(node_id, &anchor.x, "x")? else {
            return Err(invalid_anchor(
                node_id,
                "path anchor is missing required x coordinate",
            ));
        };
        let Some(y) = anchor_coordinate(node_id, &anchor.y, "y")? else {
            return Err(invalid_anchor(
                node_id,
                "path anchor is missing required y coordinate",
            ));
        };

        match Point2::new(x, y) {
            Ok(point) => points.push(point),
            Err(GeometryError::NonFinitePoint) => {
                return Err(Diagnostic::error(
                    "tx.invalid_geometry",
                    "path anchor coordinates must be finite",
                    None,
                    Some(node_id.to_owned()),
                ));
            }
            Err(GeometryError::NonFiniteParameter)
            | Err(GeometryError::ParameterOutOfRange)
            | Err(GeometryError::NonFiniteTolerance)
            | Err(GeometryError::NonPositiveTolerance) => {
                return Err(Diagnostic::error(
                    "tx.invalid_geometry",
                    "path anchor coordinates are invalid",
                    None,
                    Some(node_id.to_owned()),
                ));
            }
        }
    }

    Ok(points)
}

fn anchor_coordinate(
    node_id: &str,
    dimension: &Option<Dimension>,
    field: &str,
) -> Result<Option<f64>, Diagnostic> {
    match dimension {
        None => Ok(None),
        Some(dimension) if dimension.unit == Unit::Px => Ok(Some(dimension.value)),
        Some(_) => Err(invalid_anchor(
            node_id,
            &format!("path anchor {field} coordinate must be a px value"),
        )),
    }
}

fn geometry_diagnostic(node_id: &str, error: GeometryError) -> Diagnostic {
    match error {
        GeometryError::NonFiniteTolerance => Diagnostic::error(
            "tx.invalid_geometry_tolerance",
            "simplify_path_anchors tolerance must be finite",
            None,
            Some(node_id.to_owned()),
        ),
        GeometryError::NonPositiveTolerance => Diagnostic::error(
            "tx.invalid_geometry_tolerance",
            "simplify_path_anchors tolerance must be positive",
            None,
            Some(node_id.to_owned()),
        ),
        GeometryError::NonFinitePoint => Diagnostic::error(
            "tx.invalid_geometry",
            "path anchor coordinates must be finite",
            None,
            Some(node_id.to_owned()),
        ),
        GeometryError::NonFiniteParameter | GeometryError::ParameterOutOfRange => {
            Diagnostic::error(
                "tx.invalid_geometry",
                "path geometry is invalid",
                None,
                Some(node_id.to_owned()),
            )
        }
    }
}

fn invalid_anchor(node_id: &str, message: &str) -> Diagnostic {
    Diagnostic::error(
        "tx.invalid_path_anchor",
        message,
        None,
        Some(node_id.to_owned()),
    )
}

fn unknown_node(node_id: &str) -> Diagnostic {
    Diagnostic::error(
        "tx.unknown_node",
        format!("node {:?} not found in document", node_id),
        None,
        Some(node_id.to_owned()),
    )
}
