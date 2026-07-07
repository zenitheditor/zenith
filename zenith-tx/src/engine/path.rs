//! Path op application: `set_path_anchors`, `simplify_path_anchors`, and
//! `transform_path_anchors`.

use zenith_core::{Diagnostic, Dimension, Document, Node, PathAnchor as CorePathAnchor, Unit};
use zenith_geometry::{
    AffineTransform, GeometryError, PathAnchor, PathGeometry, Point2, simplify_polyline,
};

use crate::op::{OpPathAnchor, OpPathTransform};

use super::{find_node_any_mut, node_kind_str, px, record_affected};

const MAX_SIMPLIFY_INTERMEDIATE_POINTS: usize = 8192;

pub(super) fn apply_set_path_anchors(
    node_id: &str,
    anchors: &[OpPathAnchor],
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
                    path.anchors = anchors
                        .iter()
                        .map(|anchor| CorePathAnchor {
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

                    let tolerance_budget = tolerance / 2.0;
                    let points =
                        match flattened_path_points(node_id, &path.anchors, tolerance_budget) {
                            Ok(points) => points,
                            Err(diagnostic) => {
                                diagnostics.push(diagnostic);
                                return;
                            }
                        };

                    match simplify_polyline(&points, tolerance_budget) {
                        Ok(simplified) => {
                            path.anchors = simplified
                                .iter()
                                .map(|point| CorePathAnchor {
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

pub(super) fn apply_transform_path_anchors(
    node_id: &str,
    transform: &OpPathTransform,
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
                    let affine = match path_transform(transform) {
                        Ok(affine) => affine,
                        Err(error) => {
                            diagnostics.push(transform_geometry_diagnostic(node_id, error));
                            return;
                        }
                    };
                    let geometry = match resolved_path_geometry(
                        node_id,
                        &path.anchors,
                        path.closed == Some(true),
                    ) {
                        Ok(geometry) => geometry,
                        Err(diagnostic) => {
                            diagnostics.push(diagnostic);
                            return;
                        }
                    };
                    let transformed = match geometry.transform(affine) {
                        Ok(transformed) => transformed,
                        Err(error) => {
                            diagnostics.push(transform_geometry_diagnostic(node_id, error));
                            return;
                        }
                    };

                    path.anchors = transformed
                        .anchors()
                        .iter()
                        .map(|anchor| geometry_anchor_to_core(*anchor))
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
                        format!("transform_path_anchors is not supported on a {} node", kind),
                        None,
                        Some(node_id.to_owned()),
                    ));
                }
            }
        }
    }
}

fn path_transform(transform: &OpPathTransform) -> Result<AffineTransform, GeometryError> {
    match transform {
        OpPathTransform::Translate { dx, dy } => AffineTransform::translation(*dx, *dy),
        OpPathTransform::Rotate {
            angle_degrees,
            cx,
            cy,
        } => {
            let pivot = Point2::new(*cx, *cy)?;
            AffineTransform::rotation(*angle_degrees, pivot)
        }
        OpPathTransform::Reflect { x1, y1, x2, y2 } => {
            let start = Point2::new(*x1, *y1)?;
            let end = Point2::new(*x2, *y2)?;
            AffineTransform::reflection_across_line(start, end)
        }
    }
}

fn flattened_path_points(
    node_id: &str,
    anchors: &[CorePathAnchor],
    tolerance: f64,
) -> Result<Vec<Point2>, Diagnostic> {
    let geometry = resolved_path_geometry(node_id, anchors, false)?;
    let points = geometry
        .flatten(tolerance)
        .map_err(|error| geometry_diagnostic(node_id, error))?;
    if points.len() > MAX_SIMPLIFY_INTERMEDIATE_POINTS {
        return Err(Diagnostic::error(
            "tx.invalid_geometry",
            "path simplification produced too many intermediate anchors",
            None,
            Some(node_id.to_owned()),
        ));
    }

    Ok(points)
}

fn resolved_path_geometry(
    node_id: &str,
    anchors: &[CorePathAnchor],
    closed: bool,
) -> Result<PathGeometry, Diagnostic> {
    let mut resolved = Vec::with_capacity(anchors.len());

    for anchor in anchors {
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

        let point = match Point2::new(x, y) {
            Ok(point) => point,
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
            | Err(GeometryError::NonPositiveTolerance)
            | Err(GeometryError::NonPositiveCount)
            | Err(GeometryError::CountOutOfRange)
            | Err(GeometryError::DegenerateLine)
            | Err(GeometryError::NonFiniteTransform)
            | Err(GeometryError::SingularTransform) => {
                return Err(Diagnostic::error(
                    "tx.invalid_geometry",
                    "path anchor coordinates are invalid",
                    None,
                    Some(node_id.to_owned()),
                ));
            }
        };
        let in_handle = optional_handle(node_id, &anchor.in_x, &anchor.in_y, "in")?;
        let out_handle = optional_handle(node_id, &anchor.out_x, &anchor.out_y, "out")?;

        resolved.push(
            PathAnchor::new(point, in_handle, out_handle)
                .map_err(|error| geometry_diagnostic(node_id, error))?,
        );
    }

    PathGeometry::new(resolved, closed).map_err(|error| geometry_diagnostic(node_id, error))
}

fn geometry_anchor_to_core(anchor: PathAnchor) -> CorePathAnchor {
    CorePathAnchor {
        x: Some(px(anchor.point.x)),
        y: Some(px(anchor.point.y)),
        in_x: anchor.in_handle.map(|point| px(point.x)),
        in_y: anchor.in_handle.map(|point| px(point.y)),
        out_x: anchor.out_handle.map(|point| px(point.x)),
        out_y: anchor.out_handle.map(|point| px(point.y)),
    }
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

fn optional_handle(
    node_id: &str,
    x: &Option<Dimension>,
    y: &Option<Dimension>,
    label: &str,
) -> Result<Option<Point2>, Diagnostic> {
    match (
        anchor_coordinate(node_id, x, &format!("{label}-x"))?,
        anchor_coordinate(node_id, y, &format!("{label}-y"))?,
    ) {
        (Some(x), Some(y)) => Point2::new(x, y)
            .map(Some)
            .map_err(|error| geometry_diagnostic(node_id, error)),
        (None, None) => Ok(None),
        (Some(_), None) | (None, Some(_)) => Err(invalid_anchor(
            node_id,
            &format!("path anchor {label} handle requires both {label}-x and {label}-y"),
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
        GeometryError::NonFiniteParameter
        | GeometryError::ParameterOutOfRange
        | GeometryError::NonPositiveCount
        | GeometryError::CountOutOfRange
        | GeometryError::DegenerateLine
        | GeometryError::NonFiniteTransform
        | GeometryError::SingularTransform => Diagnostic::error(
            "tx.invalid_geometry",
            "path geometry is invalid",
            None,
            Some(node_id.to_owned()),
        ),
    }
}

fn transform_geometry_diagnostic(node_id: &str, error: GeometryError) -> Diagnostic {
    let message = match error {
        GeometryError::NonFinitePoint => "transform_path_anchors point coordinates must be finite",
        GeometryError::NonFiniteParameter => "transform_path_anchors parameters must be finite",
        GeometryError::DegenerateLine => {
            "transform_path_anchors reflect line must use two distinct points"
        }
        GeometryError::NonFiniteTransform => {
            "transform_path_anchors produced a non-finite transform"
        }
        GeometryError::SingularTransform => "transform_path_anchors transform is singular",
        GeometryError::ParameterOutOfRange
        | GeometryError::NonFiniteTolerance
        | GeometryError::NonPositiveTolerance
        | GeometryError::NonPositiveCount
        | GeometryError::CountOutOfRange => "transform_path_anchors geometry is invalid",
    };

    Diagnostic::error(
        "tx.invalid_geometry",
        message,
        None,
        Some(node_id.to_owned()),
    )
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
