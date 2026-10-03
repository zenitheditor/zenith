use std::collections::BTreeMap;

use zenith_geometry::{
    CompoundFillRule, CompoundPathGeometry, PATH_CONSUMPTION_TOLERANCE,
    PathAnchor as GeomPathAnchor, PathGeometry, Point2, PointLocation, compound_path_fill_bounds,
    locate_point_in_compound_path,
};

use crate::ast::node::{Node, PathNode, Point, TextNode, anchor_xy, parse_anchor};
use crate::ast::value::{Dimension, PropertyValue, Unit, dim_to_px};
use crate::tokens::{ResolvedToken, ResolvedValue};

/// Inset in px of the corner sample points from the text box edge.
const SAMPLE_INSET: f64 = 0.5;

#[derive(Clone, Copy, Debug)]
pub(super) struct RectPx {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) w: f64,
    pub(super) h: f64,
}

/// The map from local px to page px: `page = (dx, dy) + (sx, sy) · local`.
#[derive(Clone, Copy, Debug)]
pub(super) struct Place {
    pub(super) dx: f64,
    pub(super) dy: f64,
    pub(super) sx: f64,
    pub(super) sy: f64,
}

impl Place {
    pub(super) fn point(self, x: f64, y: f64) -> (f64, f64) {
        (self.dx + self.sx * x, self.dy + self.sy * y)
    }

    pub(super) fn rect(self, r: RectPx) -> RectPx {
        let (x, y) = self.point(r.x, r.y);
        RectPx {
            x,
            y,
            w: r.w * self.sx,
            h: r.h * self.sy,
        }
    }
}

impl RectPx {
    pub(super) fn contains_rect(self, other: Self) -> bool {
        self.x <= other.x
            && self.y <= other.y
            && self.x + self.w >= other.x + other.w
            && self.y + self.h >= other.y + other.h
    }

    pub(super) fn intersect(self, other: Self) -> Option<Self> {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.w).min(other.x + other.w);
        let y2 = (self.y + self.h).min(other.y + other.h);
        if x2 >= x1 && y2 >= y1 {
            Some(Self {
                x: x1,
                y: y1,
                w: x2 - x1,
                h: y2 - y1,
            })
        } else {
            None
        }
    }

    /// The center and four corner points to sample under the box.
    ///
    /// The corners sit [`SAMPLE_INSET`] px inside the box (at most half its
    /// size), so a backdrop that only touches an edge (a stacked neighbour)
    /// does not count as under the text.
    pub(super) fn sample_points(self) -> [(f64, f64); 5] {
        let ix = SAMPLE_INSET.min(self.w / 2.0).max(0.0);
        let iy = SAMPLE_INSET.min(self.h / 2.0).max(0.0);
        let (left, right) = (self.x + ix, self.x + self.w - ix);
        let (top, bottom) = (self.y + iy, self.y + self.h - iy);
        [
            (self.x + self.w / 2.0, self.y + self.h / 2.0),
            (left, top),
            (right, top),
            (left, bottom),
            (right, bottom),
        ]
    }

    pub(super) fn contains_point(self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }
}

#[derive(Clone, Debug)]
pub(super) enum CoverageShape {
    Rect,
    /// A rounded rectangle: the axis-aligned box minus the four corner
    /// quarter-discs (per-corner radii, top-left/top-right/bottom-right/bottom-left).
    RoundedRect {
        tl: f64,
        tr: f64,
        br: f64,
        bl: f64,
    },
    Ellipse,
    Diamond,
    Capsule,
    Polygon(Vec<(f64, f64)>),
    /// Exact filled region of a compound `path` (fill-rule aware). Bounds on the
    /// candidate are extrema-aware closed-contour fill bounds for early reject.
    PathFill {
        geometry: CompoundPathGeometry,
        rule: CompoundFillRule,
    },
}

/// A rigid rotation of a candidate about a fixed center, in degrees. The
/// renderer rotates a leaf about its own box center; containment is tested by
/// inverse-rotating the sample point and testing the unrotated shape.
#[derive(Clone, Copy, Debug)]
pub(super) struct Rotation {
    pub(super) angle_deg: f64,
    pub(super) cx: f64,
    pub(super) cy: f64,
}

impl Rotation {
    /// Inverse-rotate a sample point back into the candidate's unrotated frame.
    pub(super) fn inverse_map(self, x: f64, y: f64) -> (f64, f64) {
        let (sin, cos) = (-self.angle_deg).to_radians().sin_cos();
        let dx = x - self.cx;
        let dy = y - self.cy;
        (self.cx + dx * cos - dy * sin, self.cy + dx * sin + dy * cos)
    }
}

impl CoverageShape {
    /// The shape drawn under a local-to-page scale of `k` (the smaller axis
    /// scale): corner radii grow with it. Outlines held in page px are kept.
    pub(super) fn scaled(self, k: f64) -> Self {
        match self {
            CoverageShape::RoundedRect { tl, tr, br, bl } => CoverageShape::RoundedRect {
                tl: tl * k,
                tr: tr * k,
                br: br * k,
                bl: bl * k,
            },
            CoverageShape::Rect
            | CoverageShape::Ellipse
            | CoverageShape::Diamond
            | CoverageShape::Capsule
            | CoverageShape::Polygon(_)
            | CoverageShape::PathFill { .. } => self,
        }
    }

    pub(super) fn contains_point(&self, bounds: RectPx, x: f64, y: f64) -> bool {
        if !bounds.contains_point(x, y) {
            return false;
        }
        match self {
            CoverageShape::Rect => true,
            CoverageShape::RoundedRect { tl, tr, br, bl } => {
                point_in_rounded_rect(bounds, *tl, *tr, *br, *bl, x, y)
            }
            CoverageShape::Ellipse => point_in_ellipse(bounds, x, y),
            CoverageShape::Diamond => point_in_diamond(bounds, x, y),
            CoverageShape::Capsule => point_in_capsule(bounds, x, y),
            CoverageShape::Polygon(points) => point_in_polygon(points, x, y),
            CoverageShape::PathFill { geometry, rule } => {
                path_fill_contains_point(geometry, *rule, x, y)
            }
        }
    }
}

fn path_fill_contains_point(
    geometry: &CompoundPathGeometry,
    rule: CompoundFillRule,
    x: f64,
    y: f64,
) -> bool {
    let Ok(point) = Point2::new(x, y) else {
        return false;
    };
    matches!(
        locate_point_in_compound_path(geometry, point, rule, PATH_CONSUMPTION_TOLERANCE),
        Ok(PointLocation::Inside | PointLocation::Boundary)
    )
}

/// Exact rounded-rectangle containment. Assumes the point is already inside
/// `bounds`; excludes the point only when it falls in a corner square outside
/// that corner's quarter-disc. Radii are expected pre-clamped to the box half
/// extents by the caller.
fn point_in_rounded_rect(
    bounds: RectPx,
    tl: f64,
    tr: f64,
    br: f64,
    bl: f64,
    x: f64,
    y: f64,
) -> bool {
    let left = bounds.x;
    let top = bounds.y;
    let right = bounds.x + bounds.w;
    let bottom = bounds.y + bounds.h;
    // (corner-arc center x, center y, radius, in-corner-square predicate)
    let corners = [
        (left + tl, top + tl, tl, x < left + tl && y < top + tl),
        (right - tr, top + tr, tr, x > right - tr && y < top + tr),
        (
            right - br,
            bottom - br,
            br,
            x > right - br && y > bottom - br,
        ),
        (left + bl, bottom - bl, bl, x < left + bl && y > bottom - bl),
    ];
    for (cx, cy, r, in_corner) in corners {
        if r > 0.0 && in_corner && distance_sq(x, y, cx, cy) > r * r {
            return false;
        }
    }
    true
}

pub(super) fn resolve_axis_px(
    value: Option<&PropertyValue>,
    basis: f64,
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
) -> Option<f64> {
    let dim = match value? {
        PropertyValue::Dimension(dim) => dim,
        PropertyValue::TokenRef(id) => {
            let token = resolved_tokens.get(id.as_str())?;
            let ResolvedValue::Dimension(dim) = &token.value else {
                return None;
            };
            dim
        }
        PropertyValue::Literal(_) | PropertyValue::DataRef(_) => return None,
    };
    resolve_dim_axis(dim, basis)
}

pub(super) fn local_box(
    node: &Node,
    page_size: (f64, f64),
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
) -> Option<RectPx> {
    let fields = match node {
        Node::Rect(n) => BoxFields {
            x: n.x.as_ref(),
            y: n.y.as_ref(),
            w: n.w.as_ref(),
            h: n.h.as_ref(),
            anchor: n.anchor.as_deref(),
        },
        Node::Ellipse(n) => BoxFields {
            x: n.x.as_ref(),
            y: n.y.as_ref(),
            w: n.w.as_ref(),
            h: n.h.as_ref(),
            anchor: n.anchor.as_deref(),
        },
        Node::Frame(n) => BoxFields {
            x: n.x.as_ref(),
            y: n.y.as_ref(),
            w: n.w.as_ref(),
            h: n.h.as_ref(),
            anchor: n.anchor.as_deref(),
        },
        Node::Text(n) => return text_box(n, page_size, resolved_tokens),
        Node::Shape(n) => BoxFields {
            x: n.x.as_ref(),
            y: n.y.as_ref(),
            w: n.w.as_ref(),
            h: n.h.as_ref(),
            anchor: n.anchor.as_deref(),
        },
        Node::Image(n) => BoxFields {
            x: n.x.as_ref(),
            y: n.y.as_ref(),
            w: n.w.as_ref(),
            h: n.h.as_ref(),
            anchor: n.anchor.as_deref(),
        },
        Node::Line(_)
        | Node::Code(_)
        | Node::Group(_)
        | Node::Polygon(_)
        | Node::Polyline(_)
        | Node::Path(_)
        | Node::Instance(_)
        | Node::Field(_)
        | Node::Footnote(_)
        | Node::Toc(_)
        | Node::Table(_)
        | Node::Connector(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Mesh(_)
        | Node::Unknown(_) => return None,
    };

    box_from_fields(fields, page_size, resolved_tokens)
}

pub(super) fn polygon_region(
    points: &[Point],
    place: Place,
    page_size: (f64, f64),
) -> Option<(RectPx, CoverageShape)> {
    let mut resolved = Vec::with_capacity(points.len());
    for point in points {
        let x = point
            .x
            .as_ref()
            .and_then(|dim| resolve_dim_axis(dim, page_size.0))?;
        let y = point
            .y
            .as_ref()
            .and_then(|dim| resolve_dim_axis(dim, page_size.1))?;
        resolved.push(place.point(x, y));
    }
    if resolved.len() < 3 {
        return None;
    }
    let bounds = polygon_bounds(&resolved)?;
    Some((bounds, CoverageShape::Polygon(resolved)))
}

/// Exact fill region of a `path` for contrast backdrop coverage.
///
/// Builds absolute-page [`CompoundPathGeometry`] from `path.effective_subpaths()`,
/// resolves anchor/handle dimensions with percent-aware page axes and the
/// ancestor map `place`, and returns extrema-aware closed-fill bounds
/// plus a [`CoverageShape::PathFill`].
///
/// Returns `None` when geometry is undecidable (mixed partial coordinates),
/// open/stroke-only (no closed contour), or bounds cannot be computed.
pub(super) fn path_fill_region(
    path: &PathNode,
    place: Place,
    page_size: (f64, f64),
) -> Option<(RectPx, CoverageShape)> {
    let mut contours = Vec::new();
    for sub in path.effective_subpaths() {
        let closed = sub.closed.unwrap_or(false);
        let mut anchors = Vec::with_capacity(sub.anchors.len());
        for anchor in sub.anchors {
            let point = resolve_abs_point(anchor.x.as_ref(), anchor.y.as_ref(), place, page_size)?;
            let in_handle = resolve_optional_abs_point(
                anchor.in_x.as_ref(),
                anchor.in_y.as_ref(),
                place,
                page_size,
            )?;
            let out_handle = resolve_optional_abs_point(
                anchor.out_x.as_ref(),
                anchor.out_y.as_ref(),
                place,
                page_size,
            )?;
            let geom_anchor = GeomPathAnchor::new(point, in_handle, out_handle).ok()?;
            anchors.push(geom_anchor);
        }
        let contour = PathGeometry::new(anchors, closed).ok()?;
        contours.push(contour);
    }

    let geometry = CompoundPathGeometry::new(contours);
    let rule = match path.fill_rule.as_deref() {
        Some("evenodd") => CompoundFillRule::EvenOdd,
        Some(_) | None => CompoundFillRule::NonZero,
    };

    let fill_bounds = match compound_path_fill_bounds(&geometry, PATH_CONSUMPTION_TOLERANCE) {
        Ok(Some(bounds)) => bounds,
        Ok(None) | Err(_) => return None,
    };

    let bounds = RectPx {
        x: fill_bounds.min_x,
        y: fill_bounds.min_y,
        w: fill_bounds.width(),
        h: fill_bounds.height(),
    };
    Some((bounds, CoverageShape::PathFill { geometry, rule }))
}

/// Resolve a required absolute point: both axes must be present.
fn resolve_abs_point(
    x: Option<&Dimension>,
    y: Option<&Dimension>,
    place: Place,
    page_size: (f64, f64),
) -> Option<Point2> {
    let (x, y) = place.point(
        resolve_dim_axis(x?, page_size.0)?,
        resolve_dim_axis(y?, page_size.1)?,
    );
    Point2::new(x, y).ok()
}

/// Resolve an optional handle point.
///
/// Both axes present → absolute point; both absent → `None` handle; mixed →
/// undecidable (`None` for the whole path region).
fn resolve_optional_abs_point(
    x: Option<&Dimension>,
    y: Option<&Dimension>,
    place: Place,
    page_size: (f64, f64),
) -> Option<Option<Point2>> {
    match (x, y) {
        (None, None) => Some(None),
        (Some(_), Some(_)) => Some(Some(resolve_abs_point(x, y, place, page_size)?)),
        (Some(_), None) | (None, Some(_)) => None,
    }
}

fn resolve_dim_axis(dim: &Dimension, basis: f64) -> Option<f64> {
    if dim.unit == Unit::Pct {
        Some(dim.value / 100.0 * basis)
    } else {
        dim_to_px(dim.value, &dim.unit)
    }
}

pub(super) fn text_box(
    text: &TextNode,
    page_size: (f64, f64),
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
) -> Option<RectPx> {
    let fields = BoxFields {
        x: text.x.as_ref(),
        y: text.y.as_ref(),
        w: text.w.as_ref(),
        h: text.h.as_ref(),
        anchor: text.anchor.as_deref(),
    };
    box_from_fields(fields, page_size, resolved_tokens)
}

fn box_from_fields(
    fields: BoxFields<'_>,
    page_size: (f64, f64),
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
) -> Option<RectPx> {
    let (page_w, page_h) = page_size;
    let w = resolve_axis_px(fields.w, page_w, resolved_tokens)?;
    let h = resolve_axis_px(fields.h, page_h, resolved_tokens)?;
    let (x, y) = match (fields.x, fields.y) {
        (Some(x), Some(y)) => (
            resolve_axis_px(Some(x), page_w, resolved_tokens)?,
            resolve_axis_px(Some(y), page_h, resolved_tokens)?,
        ),
        (None, None) => {
            let anchor = parse_anchor(fields.anchor?)?;
            anchor_xy(anchor, page_w, page_h, w, h)
        }
        (Some(_), None) | (None, Some(_)) => return None,
    };

    Some(RectPx { x, y, w, h })
}

struct BoxFields<'a> {
    x: Option<&'a PropertyValue>,
    y: Option<&'a PropertyValue>,
    w: Option<&'a PropertyValue>,
    h: Option<&'a PropertyValue>,
    anchor: Option<&'a str>,
}

fn point_in_ellipse(bounds: RectPx, x: f64, y: f64) -> bool {
    let rx = bounds.w / 2.0;
    let ry = bounds.h / 2.0;
    if rx <= 0.0 || ry <= 0.0 {
        return false;
    }
    let cx = bounds.x + rx;
    let cy = bounds.y + ry;
    let nx = (x - cx) / rx;
    let ny = (y - cy) / ry;
    nx * nx + ny * ny <= 1.0
}

fn point_in_diamond(bounds: RectPx, x: f64, y: f64) -> bool {
    let rx = bounds.w / 2.0;
    let ry = bounds.h / 2.0;
    if rx <= 0.0 || ry <= 0.0 {
        return false;
    }
    let cx = bounds.x + rx;
    let cy = bounds.y + ry;
    ((x - cx).abs() / rx) + ((y - cy).abs() / ry) <= 1.0
}

fn point_in_capsule(bounds: RectPx, x: f64, y: f64) -> bool {
    let radius = bounds.h.min(bounds.w) / 2.0;
    if radius <= 0.0 {
        return false;
    }
    if bounds.w >= bounds.h {
        let left = bounds.x + radius;
        let right = bounds.x + bounds.w - radius;
        if x >= left && x <= right && y >= bounds.y && y <= bounds.y + bounds.h {
            return true;
        }
        let cx = if x < left { left } else { right };
        let cy = bounds.y + radius;
        distance_sq(x, y, cx, cy) <= radius * radius
    } else {
        let top = bounds.y + radius;
        let bottom = bounds.y + bounds.h - radius;
        if y >= top && y <= bottom && x >= bounds.x && x <= bounds.x + bounds.w {
            return true;
        }
        let cx = bounds.x + radius;
        let cy = if y < top { top } else { bottom };
        distance_sq(x, y, cx, cy) <= radius * radius
    }
}

fn polygon_bounds(points: &[(f64, f64)]) -> Option<RectPx> {
    let mut iter = points.iter();
    let &(first_x, first_y) = iter.next()?;
    let mut min_x = first_x;
    let mut max_x = first_x;
    let mut min_y = first_y;
    let mut max_y = first_y;
    for &(x, y) in iter {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    Some(RectPx {
        x: min_x,
        y: min_y,
        w: max_x - min_x,
        h: max_y - min_y,
    })
}

fn point_in_polygon(points: &[(f64, f64)], x: f64, y: f64) -> bool {
    let Some(&(mut prev_x, mut prev_y)) = points.last() else {
        return false;
    };
    let mut inside = false;
    for &(curr_x, curr_y) in points {
        let crosses = (curr_y > y) != (prev_y > y);
        if crosses {
            let dy = prev_y - curr_y;
            if dy != 0.0 {
                let intersect_x = (prev_x - curr_x) * (y - curr_y) / dy + curr_x;
                if x < intersect_x {
                    inside = !inside;
                }
            }
        }
        prev_x = curr_x;
        prev_y = curr_y;
    }
    inside
}

fn distance_sq(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let dx = x1 - x2;
    let dy = y1 - y2;
    dx * dx + dy * dy
}
