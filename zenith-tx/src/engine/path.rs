//! Path op application: `set_path_anchors`, `insert_path_anchor`,
//! `insert_path_anchor_at_point`, `move_path_anchor`, `simplify_path_anchors`,
//! and `transform_path_anchors`.

use zenith_core::{
    AnchorKind, Diagnostic, Dimension, Document, Node, PathAnchor as CorePathAnchor, Unit,
};
use zenith_geometry::{
    AffineTransform, GeometryError, PathAnchor, PathGeometry, PathSegment, Point2,
    simplify_polyline,
};

use crate::op::{OpPathAnchor, OpPathTransform};

use super::{find_node_any_mut, node_kind_str, px, record_affected};

const MAX_SIMPLIFY_INTERMEDIATE_POINTS: usize = 8192;

macro_rules! non_path_nodes {
    () => {
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
            | Node::Unknown(_)
    };
}

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
                            kind: anchor.kind.as_deref().map(AnchorKind::from_kind_str),
                            in_x: anchor.in_x.map(px),
                            in_y: anchor.in_y.map(px),
                            out_x: anchor.out_x.map(px),
                            out_y: anchor.out_y.map(px),
                        })
                        .collect();
                    record_affected(node_id, affected);
                }
                non_path_nodes!() => {
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

pub(super) fn apply_set_path_anchor_kind(
    node_id: &str,
    anchor_index: usize,
    kind: Option<&str>,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    match find_node_any_mut(doc, node_id) {
        None => diagnostics.push(unknown_node(node_id)),
        Some(node) => {
            let node_kind = node_kind_str(node);
            match node {
                Node::Path(path) => {
                    let Some(anchor) = path.anchors.get_mut(anchor_index) else {
                        diagnostics.push(Diagnostic::error(
                            "tx.out_of_range",
                            format!(
                                "anchor_index {anchor_index} is out of range for path '{node_id}'"
                            ),
                            None,
                            Some(node_id.to_owned()),
                        ));
                        return;
                    };

                    anchor.kind = kind.map(AnchorKind::from_kind_str);
                    record_affected(node_id, affected);
                }
                non_path_nodes!() => {
                    diagnostics.push(Diagnostic::error(
                        "tx.unsupported_property",
                        format!(
                            "set_path_anchor_kind is not supported on a {} node",
                            node_kind
                        ),
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
                                    kind: None,
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
                non_path_nodes!() => {
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

pub(super) fn apply_insert_path_anchor(
    node_id: &str,
    segment_index: usize,
    t: f64,
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
                    path.anchors = match split_geometry_anchors(
                        node_id,
                        &geometry,
                        &path.anchors,
                        segment_index,
                        t,
                    ) {
                        Ok(anchors) => anchors,
                        Err(diagnostic) => {
                            diagnostics.push(diagnostic);
                            return;
                        }
                    };
                    record_affected(node_id, affected);
                }
                non_path_nodes!() => {
                    diagnostics.push(Diagnostic::error(
                        "tx.unsupported_property",
                        format!("insert_path_anchor is not supported on a {} node", kind),
                        None,
                        Some(node_id.to_owned()),
                    ));
                }
            }
        }
    }
}

pub(super) fn apply_insert_path_anchor_at_point(
    node_id: &str,
    x: f64,
    y: f64,
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
                    let point = match Point2::new(x, y) {
                        Ok(point) => point,
                        Err(error) => {
                            diagnostics.push(insert_at_point_geometry_diagnostic(node_id, error));
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
                    let projection = match geometry.project(point, tolerance) {
                        Ok(Some(projection)) => projection,
                        Ok(None) => {
                            diagnostics.push(Diagnostic::error(
                                "tx.invalid_geometry",
                                "insert_path_anchor_at_point requires a path segment to project onto",
                                None,
                                Some(node_id.to_owned()),
                            ));
                            return;
                        }
                        Err(error) => {
                            diagnostics.push(insert_at_point_geometry_diagnostic(node_id, error));
                            return;
                        }
                    };
                    if projection.distance_squared > tolerance * tolerance {
                        diagnostics.push(Diagnostic::error(
                            "tx.invalid_geometry",
                            "insert_path_anchor_at_point found no path projection within tolerance",
                            None,
                            Some(node_id.to_owned()),
                        ));
                        return;
                    }

                    path.anchors = match split_geometry_anchors(
                        node_id,
                        &geometry,
                        &path.anchors,
                        projection.segment_index,
                        projection.segment_t,
                    ) {
                        Ok(anchors) => anchors,
                        Err(diagnostic) => {
                            diagnostics.push(diagnostic);
                            return;
                        }
                    };
                    record_affected(node_id, affected);
                }
                non_path_nodes!() => {
                    diagnostics.push(Diagnostic::error(
                        "tx.unsupported_property",
                        format!(
                            "insert_path_anchor_at_point is not supported on a {} node",
                            kind
                        ),
                        None,
                        Some(node_id.to_owned()),
                    ));
                }
            }
        }
    }
}

pub(super) fn apply_move_path_anchor(
    node_id: &str,
    anchor_index: usize,
    dx: f64,
    dy: f64,
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
                    if !dx.is_finite() || !dy.is_finite() {
                        diagnostics.push(Diagnostic::error(
                            "tx.invalid_geometry",
                            "move_path_anchor dx and dy must be finite",
                            None,
                            Some(node_id.to_owned()),
                        ));
                        return;
                    }

                    let Some(anchor) = path.anchors.get(anchor_index) else {
                        diagnostics.push(Diagnostic::error(
                            "tx.out_of_range",
                            format!(
                                "anchor_index {anchor_index} is out of range for path '{node_id}'"
                            ),
                            None,
                            Some(node_id.to_owned()),
                        ));
                        return;
                    };

                    let x = match anchor_coordinate(node_id, &anchor.x, "x") {
                        Ok(Some(x)) => x,
                        Ok(None) => {
                            diagnostics.push(invalid_anchor(
                                node_id,
                                "path anchor is missing required x coordinate",
                            ));
                            return;
                        }
                        Err(diagnostic) => {
                            diagnostics.push(diagnostic);
                            return;
                        }
                    };
                    let y = match anchor_coordinate(node_id, &anchor.y, "y") {
                        Ok(Some(y)) => y,
                        Ok(None) => {
                            diagnostics.push(invalid_anchor(
                                node_id,
                                "path anchor is missing required y coordinate",
                            ));
                            return;
                        }
                        Err(diagnostic) => {
                            diagnostics.push(diagnostic);
                            return;
                        }
                    };
                    if let Err(error) = Point2::new(x, y) {
                        diagnostics.push(move_anchor_geometry_diagnostic(node_id, error));
                        return;
                    }
                    let in_handle = match optional_handle(node_id, &anchor.in_x, &anchor.in_y, "in")
                    {
                        Ok(handle) => handle,
                        Err(diagnostic) => {
                            diagnostics.push(diagnostic);
                            return;
                        }
                    };
                    let out_handle =
                        match optional_handle(node_id, &anchor.out_x, &anchor.out_y, "out") {
                            Ok(handle) => handle,
                            Err(diagnostic) => {
                                diagnostics.push(diagnostic);
                                return;
                            }
                        };

                    let moved_x = x + dx;
                    let moved_y = y + dy;
                    if let Err(error) = Point2::new(moved_x, moved_y) {
                        diagnostics.push(move_anchor_geometry_diagnostic(node_id, error));
                        return;
                    }
                    let moved_in_handle = match in_handle {
                        Some(point) => {
                            let moved = Point2::new(point.x + dx, point.y + dy);
                            match moved {
                                Ok(point) => Some(point),
                                Err(error) => {
                                    diagnostics
                                        .push(move_anchor_geometry_diagnostic(node_id, error));
                                    return;
                                }
                            }
                        }
                        None => None,
                    };
                    let moved_out_handle = match out_handle {
                        Some(point) => {
                            let moved = Point2::new(point.x + dx, point.y + dy);
                            match moved {
                                Ok(point) => Some(point),
                                Err(error) => {
                                    diagnostics
                                        .push(move_anchor_geometry_diagnostic(node_id, error));
                                    return;
                                }
                            }
                        }
                        None => None,
                    };

                    let Some(anchor) = path.anchors.get_mut(anchor_index) else {
                        diagnostics.push(Diagnostic::error(
                            "tx.out_of_range",
                            format!(
                                "anchor_index {anchor_index} is out of range for path '{node_id}'"
                            ),
                            None,
                            Some(node_id.to_owned()),
                        ));
                        return;
                    };
                    anchor.x = Some(px(moved_x));
                    anchor.y = Some(px(moved_y));
                    if let Some(point) = moved_in_handle {
                        anchor.in_x = Some(px(point.x));
                        anchor.in_y = Some(px(point.y));
                    }
                    if let Some(point) = moved_out_handle {
                        anchor.out_x = Some(px(point.x));
                        anchor.out_y = Some(px(point.y));
                    }
                    record_affected(node_id, affected);
                }
                non_path_nodes!() => {
                    diagnostics.push(Diagnostic::error(
                        "tx.unsupported_property",
                        format!("move_path_anchor is not supported on a {} node", kind),
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
                        .zip(path.anchors.iter())
                        .map(|(anchor, original)| {
                            geometry_anchor_to_core(*anchor, original.kind.clone())
                        })
                        .collect();
                    record_affected(node_id, affected);
                }
                non_path_nodes!() => {
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

fn segment_kind(
    geometry: &PathGeometry,
    segment_index: usize,
) -> Result<PathSegment, GeometryError> {
    geometry
        .segments()?
        .get(segment_index)
        .copied()
        .ok_or(GeometryError::CountOutOfRange)
}

fn split_geometry_anchors(
    node_id: &str,
    geometry: &PathGeometry,
    original_anchors: &[CorePathAnchor],
    segment_index: usize,
    t: f64,
) -> Result<Vec<CorePathAnchor>, Diagnostic> {
    let inserted_kind = match segment_kind(geometry, segment_index) {
        Ok(PathSegment::Cubic { .. }) => Some(AnchorKind::Smooth),
        Ok(PathSegment::Line { .. }) => None,
        Err(error) => return Err(insert_geometry_diagnostic(node_id, error)),
    };
    let (split, inserted_index) = geometry
        .split_segment(segment_index, t)
        .map_err(|error| insert_geometry_diagnostic(node_id, error))?;

    Ok(split
        .anchors()
        .iter()
        .enumerate()
        .map(|(index, anchor)| {
            let kind = if index == inserted_index {
                inserted_kind.clone()
            } else {
                existing_anchor_kind_at(original_anchors, index, inserted_index)
            };
            geometry_anchor_to_core(*anchor, kind)
        })
        .collect())
}

fn existing_anchor_kind_at(
    anchors: &[CorePathAnchor],
    index: usize,
    inserted_index: usize,
) -> Option<AnchorKind> {
    let original_index = if index < inserted_index {
        index
    } else {
        index.saturating_sub(1)
    };
    anchors
        .get(original_index)
        .and_then(|anchor| anchor.kind.clone())
}

fn geometry_anchor_to_core(anchor: PathAnchor, kind: Option<AnchorKind>) -> CorePathAnchor {
    CorePathAnchor {
        x: Some(px(anchor.point.x)),
        y: Some(px(anchor.point.y)),
        kind,
        in_x: anchor.in_handle.map(|point| px(point.x)),
        in_y: anchor.in_handle.map(|point| px(point.y)),
        out_x: anchor.out_handle.map(|point| px(point.x)),
        out_y: anchor.out_handle.map(|point| px(point.y)),
    }
}

pub(super) fn anchor_coordinate(
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

pub(super) fn optional_handle(
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

fn insert_geometry_diagnostic(node_id: &str, error: GeometryError) -> Diagnostic {
    let message = match error {
        GeometryError::NonFiniteParameter => "insert_path_anchor t must be finite",
        GeometryError::ParameterOutOfRange => "insert_path_anchor t must be between 0 and 1",
        GeometryError::CountOutOfRange => {
            "insert_path_anchor segment_index is outside the path segment range"
        }
        GeometryError::NonFinitePoint => "insert_path_anchor path coordinates must be finite",
        GeometryError::NonFiniteTolerance
        | GeometryError::NonPositiveTolerance
        | GeometryError::NonPositiveCount
        | GeometryError::DegenerateLine
        | GeometryError::NonFiniteTransform
        | GeometryError::SingularTransform => "insert_path_anchor geometry is invalid",
    };

    Diagnostic::error(
        "tx.invalid_geometry",
        message,
        None,
        Some(node_id.to_owned()),
    )
}

fn insert_at_point_geometry_diagnostic(node_id: &str, error: GeometryError) -> Diagnostic {
    match error {
        GeometryError::NonFiniteTolerance => Diagnostic::error(
            "tx.invalid_geometry_tolerance",
            "insert_path_anchor_at_point tolerance must be finite",
            None,
            Some(node_id.to_owned()),
        ),
        GeometryError::NonPositiveTolerance => Diagnostic::error(
            "tx.invalid_geometry_tolerance",
            "insert_path_anchor_at_point tolerance must be positive",
            None,
            Some(node_id.to_owned()),
        ),
        GeometryError::NonFinitePoint => Diagnostic::error(
            "tx.invalid_geometry",
            "insert_path_anchor_at_point point coordinates must be finite",
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
            "insert_path_anchor_at_point geometry is invalid",
            None,
            Some(node_id.to_owned()),
        ),
    }
}

fn move_anchor_geometry_diagnostic(node_id: &str, error: GeometryError) -> Diagnostic {
    let message = match error {
        GeometryError::NonFinitePoint => "move_path_anchor path coordinates must be finite",
        GeometryError::NonFiniteParameter
        | GeometryError::ParameterOutOfRange
        | GeometryError::NonFiniteTolerance
        | GeometryError::NonPositiveTolerance
        | GeometryError::NonPositiveCount
        | GeometryError::CountOutOfRange
        | GeometryError::DegenerateLine
        | GeometryError::NonFiniteTransform
        | GeometryError::SingularTransform => "move_path_anchor geometry is invalid",
    };

    Diagnostic::error(
        "tx.invalid_geometry",
        message,
        None,
        Some(node_id.to_owned()),
    )
}

pub(super) fn invalid_anchor(node_id: &str, message: &str) -> Diagnostic {
    Diagnostic::error(
        "tx.invalid_path_anchor",
        message,
        None,
        Some(node_id.to_owned()),
    )
}

pub(super) fn unknown_node(node_id: &str) -> Diagnostic {
    Diagnostic::error(
        "tx.unknown_node",
        format!("node {:?} not found in document", node_id),
        None,
        Some(node_id.to_owned()),
    )
}
