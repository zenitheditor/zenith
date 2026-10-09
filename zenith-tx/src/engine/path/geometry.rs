//! Geometry resolution and anchor/coordinate conversion helpers for path ops.

use zenith_core::{
    AnchorKind, Diagnostic, Dimension, Document, PathAnchor as CorePathAnchor, Unit,
};
use zenith_geometry::{
    AffineTransform, GeometryError, PathAnchor, PathGeometry, PathSegment, Point2,
};

use super::diagnostics::{
    geometry_diagnostic, insert_geometry_diagnostic, invalid_anchor, transform_geometry_diagnostic,
};
use crate::engine::px;
use crate::engine::space::{parent_chain, resolved_tokens, shift_between};
use crate::op::OpPathTransform;

const MAX_SIMPLIFY_INTERMEDIATE_POINTS: usize = 8192;

pub(super) fn transform_core_anchors(
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

pub(super) fn path_transform(
    transform: &OpPathTransform,
) -> Result<AffineTransform, GeometryError> {
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
        OpPathTransform::Scale { sx, sy, cx, cy } => {
            let pivot = Point2::new(*cx, *cy)?;
            AffineTransform::scale(*sx, *sy, pivot)
        }
    }
}

pub(super) fn flattened_path_points(
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

pub(super) fn path_has_handles(anchors: &[CorePathAnchor]) -> bool {
    anchors.iter().any(|anchor| {
        anchor.in_x.is_some()
            || anchor.in_y.is_some()
            || anchor.out_x.is_some()
            || anchor.out_y.is_some()
    })
}

pub(super) fn simplified_points_to_core_anchors(points: &[Point2]) -> Vec<CorePathAnchor> {
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

pub(crate) fn resolved_path_geometry(
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

pub(super) fn split_geometry_anchors(
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

pub(crate) fn geometry_anchor_to_core(
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

pub(crate) fn anchor_coordinate(
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

pub(crate) fn optional_handle(
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

/// The px shift that moves the coordinates of path `target_id` into the
/// parent space of path `source_id`: origin(target parent) minus
/// origin(source parent), counted below their shared ancestor.
///
/// `(0, 0)` when either node is missing (the caller reports it). Errors
/// with `tx.invalid_geometry` when a container origin does not resolve.
pub(super) fn target_shift(
    doc: &Document,
    source_id: &str,
    target_id: &str,
    op_name: &str,
) -> Result<(f64, f64), Diagnostic> {
    let resolved = resolved_tokens(doc);
    let (Some(source), Some(target)) = (
        parent_chain(doc, source_id, &resolved),
        parent_chain(doc, target_id, &resolved),
    ) else {
        return Ok((0.0, 0.0));
    };
    shift_between(&target, &source).map_err(|container| {
        Diagnostic::error(
            "tx.invalid_geometry",
            format!(
                "{op_name}: paths {source_id:?} and {target_id:?} sit in different containers, \
                 and the px origin of container {container:?} does not resolve; move both \
                 paths into one container with reparent"
            ),
            None,
            Some(source_id.to_owned()),
        )
    })
}
