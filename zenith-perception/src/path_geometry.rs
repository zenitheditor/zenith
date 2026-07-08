use zenith_core::{Dimension, PathAnchor, Unit};
use zenith_geometry::{
    CompoundPathGeometry, PathAnchor as GeometryPathAnchor, PathGeometry, Point2,
};

use crate::VectorPathContourInput;

pub(crate) fn complete_handle_count(anchor: &PathAnchor) -> usize {
    usize::from(anchor.in_x.is_some() && anchor.in_y.is_some())
        + usize::from(anchor.out_x.is_some() && anchor.out_y.is_some())
}

pub(crate) fn geometry_anchor(anchor: &PathAnchor) -> Option<GeometryPathAnchor> {
    GeometryPathAnchor::new(
        point_from_px_pair(anchor.x.as_ref(), anchor.y.as_ref())?,
        optional_point_from_px_pair(anchor.in_x.as_ref(), anchor.in_y.as_ref())?,
        optional_point_from_px_pair(anchor.out_x.as_ref(), anchor.out_y.as_ref())?,
    )
    .ok()
}

pub(crate) fn geometry_path(anchors: &[PathAnchor], closed: bool) -> Result<PathGeometry, ()> {
    let geometry_anchors = anchors
        .iter()
        .map(geometry_anchor)
        .collect::<Option<Vec<_>>>()
        .ok_or(())?;

    PathGeometry::new(geometry_anchors, closed).map_err(|_| ())
}

pub(crate) fn compound_geometry(
    contours: &[VectorPathContourInput<'_>],
) -> Result<CompoundPathGeometry, ()> {
    let geometry_contours = contours
        .iter()
        .map(|contour| geometry_path(contour.anchors, contour.closed))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(CompoundPathGeometry::new(geometry_contours))
}

fn optional_point_from_px_pair(
    x: Option<&Dimension>,
    y: Option<&Dimension>,
) -> Option<Option<Point2>> {
    match (x, y) {
        (None, None) => Some(None),
        (Some(x), Some(y)) => point_from_px_pair(Some(x), Some(y)).map(Some),
        (Some(_), None) | (None, Some(_)) => None,
    }
}

fn point_from_px_pair(x: Option<&Dimension>, y: Option<&Dimension>) -> Option<Point2> {
    Point2::new(px_value(x)?, px_value(y)?).ok()
}

fn px_value(dimension: Option<&Dimension>) -> Option<f64> {
    let dimension = dimension?;
    match dimension.unit {
        Unit::Px if dimension.value.is_finite() => Some(dimension.value),
        Unit::Px | Unit::Pt | Unit::Pct | Unit::Deg | Unit::Unknown(_) => None,
    }
}
