//! Path op application: `set_path_anchors`, `insert_path_anchor`,
//! `insert_path_anchor_at_point`, `move_path_anchor`, `simplify_path_anchors`,
//! and `transform_path_anchors`.

use zenith_core::{
    AnchorKind, Diagnostic, Dimension, Document, Node, PathAnchor as CorePathAnchor, Unit,
};
use zenith_geometry::{
    AffineTransform, CompoundPathGeometry, GeometryError, PathAnchor, PathGeometry, PathSegment,
    Point2, fit_cubic_path_anchors_to_points, simplify_polyline,
};

use crate::op::{OpPathAnchor, OpPathTransform};

use super::path_contour::path_contour_mut;
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

pub(super) fn reject_compound_path(
    node_id: &str,
    op_name: &str,
    path: &zenith_core::PathNode,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if path.subpaths.is_empty() {
        return false;
    }
    diagnostics.push(Diagnostic::error(
        "tx.unsupported_property",
        format!("{op_name} is not supported on compound path '{node_id}'"),
        None,
        Some(node_id.to_owned()),
    ));
    true
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
                    if reject_compound_path(node_id, "set_path_anchors", path, diagnostics) {
                        return;
                    }
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
    subpath_index: Option<usize>,
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
                    let Some(contour) = path_contour_mut(
                        node_id,
                        "set_path_anchor_kind",
                        path,
                        subpath_index,
                        diagnostics,
                    ) else {
                        return;
                    };
                    let Some(anchor) = contour.anchors.get_mut(anchor_index) else {
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
    subpath_index: Option<usize>,
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
                    let Some(contour) = path_contour_mut(
                        node_id,
                        "simplify_path_anchors",
                        path,
                        subpath_index,
                        diagnostics,
                    ) else {
                        return;
                    };
                    if contour.closed {
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
                        match flattened_path_points(node_id, contour.anchors, tolerance_budget) {
                            Ok(points) => points,
                            Err(diagnostic) => {
                                diagnostics.push(diagnostic);
                                return;
                            }
                        };

                    match simplify_polyline(&points, tolerance_budget) {
                        Ok(simplified) => {
                            *contour.anchors = if path_has_handles(contour.anchors) {
                                match fit_cubic_path_anchors_to_points(
                                    &simplified,
                                    tolerance_budget,
                                ) {
                                    Ok(Some(anchors)) => anchors
                                        .into_iter()
                                        .map(|anchor| geometry_anchor_to_core(anchor, None))
                                        .collect(),
                                    Ok(None) => simplified_points_to_core_anchors(&simplified),
                                    Err(error) => {
                                        diagnostics.push(geometry_diagnostic(node_id, error));
                                        return;
                                    }
                                }
                            } else {
                                simplified_points_to_core_anchors(&simplified)
                            };
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
    subpath_index: Option<usize>,
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
                    let Some(contour) = path_contour_mut(
                        node_id,
                        "insert_path_anchor",
                        path,
                        subpath_index,
                        diagnostics,
                    ) else {
                        return;
                    };
                    let geometry =
                        match resolved_path_geometry(node_id, contour.anchors, contour.closed) {
                            Ok(geometry) => geometry,
                            Err(diagnostic) => {
                                diagnostics.push(diagnostic);
                                return;
                            }
                        };
                    *contour.anchors = match split_geometry_anchors(
                        node_id,
                        &geometry,
                        contour.anchors,
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
                    if !path.subpaths.is_empty() {
                        let mut geometries = Vec::with_capacity(path.subpaths.len());
                        for subpath in &path.subpaths {
                            let geometry = match resolved_path_geometry(
                                node_id,
                                &subpath.anchors,
                                subpath.closed == Some(true),
                            ) {
                                Ok(geometry) => geometry,
                                Err(diagnostic) => {
                                    diagnostics.push(diagnostic);
                                    return;
                                }
                            };
                            geometries.push(geometry);
                        }
                        let compound = CompoundPathGeometry::new(geometries);
                        let projection = match compound.project(point, tolerance) {
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
                                diagnostics
                                    .push(insert_at_point_geometry_diagnostic(node_id, error));
                                return;
                            }
                        };
                        if projection.projection.distance_squared > tolerance * tolerance {
                            diagnostics.push(Diagnostic::error(
                                "tx.invalid_geometry",
                                "insert_path_anchor_at_point found no path projection within tolerance",
                                None,
                                Some(node_id.to_owned()),
                            ));
                            return;
                        }
                        let Some(geometry) = compound.contours().get(projection.contour_index)
                        else {
                            diagnostics.push(Diagnostic::error(
                                "tx.out_of_range",
                                format!(
                                    "subpath_index {} is out of range for path '{node_id}'",
                                    projection.contour_index
                                ),
                                None,
                                Some(node_id.to_owned()),
                            ));
                            return;
                        };
                        let Some(subpath) = path.subpaths.get_mut(projection.contour_index) else {
                            diagnostics.push(Diagnostic::error(
                                "tx.out_of_range",
                                format!(
                                    "subpath_index {} is out of range for path '{node_id}'",
                                    projection.contour_index
                                ),
                                None,
                                Some(node_id.to_owned()),
                            ));
                            return;
                        };
                        subpath.anchors = match split_geometry_anchors(
                            node_id,
                            geometry,
                            &subpath.anchors,
                            projection.projection.segment_index,
                            projection.projection.segment_t,
                        ) {
                            Ok(anchors) => anchors,
                            Err(diagnostic) => {
                                diagnostics.push(diagnostic);
                                return;
                            }
                        };
                        record_affected(node_id, affected);
                        return;
                    }
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
                    if path.subpaths.is_empty() {
                        path.anchors = match transform_core_anchors(
                            node_id,
                            &path.anchors,
                            path.closed == Some(true),
                            affine,
                        ) {
                            Ok(anchors) => anchors,
                            Err(diagnostic) => {
                                diagnostics.push(diagnostic);
                                return;
                            }
                        };
                    } else {
                        let mut transformed_subpaths = Vec::with_capacity(path.subpaths.len());
                        for subpath in &path.subpaths {
                            let anchors = match transform_core_anchors(
                                node_id,
                                &subpath.anchors,
                                subpath.closed == Some(true),
                                affine,
                            ) {
                                Ok(anchors) => anchors,
                                Err(diagnostic) => {
                                    diagnostics.push(diagnostic);
                                    return;
                                }
                            };
                            transformed_subpaths.push(anchors);
                        }
                        for (subpath, anchors) in path.subpaths.iter_mut().zip(transformed_subpaths)
                        {
                            subpath.anchors = anchors;
                        }
                    }
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

fn transform_core_anchors(
    node_id: &str,
    anchors: &[CorePathAnchor],
    closed: bool,
    affine: AffineTransform,
) -> Result<Vec<CorePathAnchor>, Diagnostic> {
    let geometry = resolved_path_geometry(node_id, anchors, closed)?;
    let transformed = geometry
        .transform(affine)
        .map_err(|error| transform_geometry_diagnostic(node_id, error))?;

    Ok(transformed
        .anchors()
        .iter()
        .zip(anchors.iter())
        .map(|(anchor, original)| geometry_anchor_to_core(*anchor, original.kind.clone()))
        .collect())
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

fn path_has_handles(anchors: &[CorePathAnchor]) -> bool {
    anchors.iter().any(|anchor| {
        anchor.in_x.is_some()
            || anchor.in_y.is_some()
            || anchor.out_x.is_some()
            || anchor.out_y.is_some()
    })
}

fn simplified_points_to_core_anchors(points: &[Point2]) -> Vec<CorePathAnchor> {
    points
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
        .collect()
}

pub(super) fn resolved_path_geometry(
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
            | Err(GeometryError::InvalidContour)
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

pub(super) fn geometry_anchor_to_core(
    anchor: PathAnchor,
    kind: Option<AnchorKind>,
) -> CorePathAnchor {
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
        | GeometryError::InvalidContour
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
        GeometryError::InvalidContour => "transform_path_anchors contour geometry is invalid",
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
        | GeometryError::InvalidContour
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
        | GeometryError::InvalidContour
        | GeometryError::NonFiniteTransform
        | GeometryError::SingularTransform => Diagnostic::error(
            "tx.invalid_geometry",
            "insert_path_anchor_at_point geometry is invalid",
            None,
            Some(node_id.to_owned()),
        ),
    }
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
