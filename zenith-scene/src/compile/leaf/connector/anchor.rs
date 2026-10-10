//! Anchor-point geometry for connectors: resolving an anchor string (grid,
//! `auto`, or a divided `i/N`) to a page-absolute point on a target box and the
//! orientation the routed path leaves/enters through.

use zenith_core::ast::{ConnectorAnchor, parse_connector_anchor};
use zenith_geometry::math;
use zenith_geometry::{
    GeometryError, PATH_CONSUMPTION_TOLERANCE, Point2, sample_closed_polyline_perimeter,
    sample_open_polyline_perimeter, sample_outline_perimeter,
};

use crate::compile::field::{ConnectorTargetKind, PathConnectorGeometry};

/// Which edge of a box an anchor sits on, expressed as the orientation the path
/// must leave/enter through. `Horizontal` = a left/right edge → the path leaves
/// horizontally; `Vertical` = a top/bottom edge → the path leaves vertically.
///
/// Used by orthogonal routing to guarantee the first/last segment is
/// perpendicular to the box edge, so arrowheads land axis-aligned.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum AnchorSide {
    Horizontal,
    Vertical,
}

/// Exact outline geometry for divided-anchor sampling on polygon / polyline /
/// path targets. Named/`auto`/grid anchors ignore this and stay bounds-based.
#[derive(Clone, Copy)]
pub(super) enum ExactOutlineRef<'a> {
    ClosedRing(&'a [Point2]),
    OpenPolyline(&'a [Point2]),
    Path(&'a PathConnectorGeometry),
}

/// Compute the page-absolute anchor point on a `(x, y, w, h)` box, AND the
/// orientation used to leave/enter it ([`AnchorSide`]).
///
/// Anchors are a **nine-point grid**: a horizontal band (`left` / `center` /
/// `right`) optionally combined with a vertical band (`top` / `center` /
/// `bottom`) via a hyphen — e.g. `top-left`, `bottom-center`, `center-right`.
/// A bare single token names the corresponding edge mid-point (`top` =
/// `top-center`, `left` = `center-left`); `center` is the box center. `mid` and
/// `middle` are accepted synonyms for `center`. The five pre-grid names
/// (`top`/`bottom`/`left`/`right`/`center`) resolve identically, so existing
/// output is unchanged.
///
/// `"auto"` (the default for an absent / unrecognized anchor) chooses the edge
/// by the dominant axis toward `toward` (the OTHER box's center): a larger
/// horizontal delta picks left/right (`Horizontal`), otherwise top/bottom
/// (`Vertical`).
///
/// Divided anchors (`i/N`) on polygon/polyline/path walk the true outline when
/// `exact` is present; named/auto/grid always use the bounds box.
pub(super) fn resolve_anchor(
    boxr: (f64, f64, f64, f64),
    kind: ConnectorTargetKind,
    anchor: &str,
    toward: (f64, f64),
    exact: Option<ExactOutlineRef<'_>>,
) -> ((f64, f64), AnchorSide) {
    let (x, y, w, h) = boxr;
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;

    if let Ok(ConnectorAnchor::Divided { index, count }) = parse_connector_anchor(anchor) {
        return divided_anchor(boxr, kind, index, count, exact);
    }

    if let Some(resolved) = grid_anchor(anchor, boxr) {
        return resolved;
    }

    // "auto" and any unrecognized value: dominant-axis edge toward `toward`.
    let dx = toward.0 - cx;
    let dy = toward.1 - cy;
    if dx.abs() >= dy.abs() {
        let pt = if dx >= 0.0 { (x + w, cy) } else { (x, cy) };
        (pt, AnchorSide::Horizontal)
    } else if dy >= 0.0 {
        ((cx, y + h), AnchorSide::Vertical)
    } else {
        ((cx, y), AnchorSide::Vertical)
    }
}

fn divided_anchor(
    boxr: (f64, f64, f64, f64),
    kind: ConnectorTargetKind,
    index: usize,
    count: usize,
    exact: Option<ExactOutlineRef<'_>>,
) -> ((f64, f64), AnchorSide) {
    match kind {
        ConnectorTargetKind::BoxLike => divided_box_anchor(boxr, index, count),
        ConnectorTargetKind::RoundedRect { tl, tr, br, bl } => {
            divided_rounded_rect_anchor(boxr, [tl, tr, br, bl], index, count)
        }
        ConnectorTargetKind::Capsule => divided_capsule_anchor(boxr, index, count),
        ConnectorTargetKind::Diamond => divided_diamond_anchor(boxr, index, count),
        ConnectorTargetKind::Ellipse => divided_ellipse_anchor(boxr, index, count),
        ConnectorTargetKind::ClosedRing => {
            if let Some(ExactOutlineRef::ClosedRing(points)) = exact
                && let Some(pt) = sample_poly_divided(
                    sample_closed_polyline_perimeter,
                    points,
                    index,
                    count,
                    boxr,
                )
            {
                return pt;
            }
            divided_box_anchor(boxr, index, count)
        }
        ConnectorTargetKind::OpenPolyline => {
            if let Some(ExactOutlineRef::OpenPolyline(points)) = exact
                && let Some(pt) =
                    sample_poly_divided(sample_open_polyline_perimeter, points, index, count, boxr)
            {
                return pt;
            }
            divided_box_anchor(boxr, index, count)
        }
        ConnectorTargetKind::PathOutline => {
            if let Some(ExactOutlineRef::Path(path_geom)) = exact
                && let Some(pt) = sample_path_divided(path_geom, index, count, boxr)
            {
                return pt;
            }
            divided_box_anchor(boxr, index, count)
        }
    }
}

/// Sample a closed or open polyline perimeter and attach an orthogonal leave
/// side from the target box center.
fn sample_poly_divided(
    sample: fn(&[Point2], usize, usize) -> Result<Point2, GeometryError>,
    points: &[Point2],
    index: usize,
    count: usize,
    boxr: (f64, f64, f64, f64),
) -> Option<((f64, f64), AnchorSide)> {
    let p = sample(points, index, count).ok()?;
    let center = box_center(boxr);
    Some(((p.x, p.y), anchor_side_from_center((p.x, p.y), center)))
}

/// Closed exterior outline when available; otherwise open polyline walk of the
/// first open flattened contour (open-only paths).
fn sample_path_divided(
    path_geom: &PathConnectorGeometry,
    index: usize,
    count: usize,
    boxr: (f64, f64, f64, f64),
) -> Option<((f64, f64), AnchorSide)> {
    let center = box_center(boxr);
    if let Ok(p) = sample_outline_perimeter(
        &path_geom.geometry,
        index,
        count,
        PATH_CONSUMPTION_TOLERANCE,
        path_geom.fill_rule,
    ) {
        return Some(((p.x, p.y), anchor_side_from_center((p.x, p.y), center)));
    }

    let flattened = path_geom
        .geometry
        .flatten_contours(PATH_CONSUMPTION_TOLERANCE)
        .ok()?;
    for contour in flattened {
        if contour.closed || contour.points.len() < 2 {
            continue;
        }
        let p = sample_open_polyline_perimeter(&contour.points, index, count).ok()?;
        return Some(((p.x, p.y), anchor_side_from_center((p.x, p.y), center)));
    }
    None
}

fn box_center(boxr: (f64, f64, f64, f64)) -> (f64, f64) {
    (boxr.0 + boxr.2 / 2.0, boxr.1 + boxr.3 / 2.0)
}

/// Walk the true rounded-rect perimeter (straight edges + quarter-circle
/// corners), starting at top-center — same origin convention as
/// [`divided_box_anchor`].
///
/// Corner radii are `(tl, tr, br, bl)` and clamped so adjacent corners never
/// exceed the shared edge length and never exceed `min(w, h) / 2`.
fn divided_rounded_rect_anchor(
    boxr: (f64, f64, f64, f64),
    radii: [f64; 4],
    index: usize,
    count: usize,
) -> ((f64, f64), AnchorSide) {
    let (x, y, w, h) = boxr;
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    if w <= 0.0 || h <= 0.0 {
        return ((cx, y), AnchorSide::Vertical);
    }

    // Clamp each radius to half-min dimension, then scale pairs that share an
    // edge so they never exceed that edge length.
    let max_r = (w.min(h) / 2.0).max(0.0);
    let mut tl = radii[0].clamp(0.0, max_r);
    let mut tr = radii[1].clamp(0.0, max_r);
    let mut br = radii[2].clamp(0.0, max_r);
    let mut bl = radii[3].clamp(0.0, max_r);
    if tl + tr > w && w > 0.0 {
        let s = w / (tl + tr);
        tl *= s;
        tr *= s;
    }
    if bl + br > w && w > 0.0 {
        let s = w / (bl + br);
        bl *= s;
        br *= s;
    }
    if tl + bl > h && h > 0.0 {
        let s = h / (tl + bl);
        tl *= s;
        bl *= s;
    }
    if tr + br > h && h > 0.0 {
        let s = h / (tr + br);
        tr *= s;
        br *= s;
    }

    // Segments in walk order starting at top-center:
    // 0 top mid→right, 1 TR arc, 2 right, 3 BR arc, 4 bottom, 5 BL arc, 6 left, 7 TL arc, 8 top left→mid
    let top_right = (w / 2.0 - tr).max(0.0);
    let right = (h - tr - br).max(0.0);
    let bottom = (w - br - bl).max(0.0);
    let left = (h - bl - tl).max(0.0);
    let top_left = (w / 2.0 - tl).max(0.0);
    let arc = |r: f64| std::f64::consts::FRAC_PI_2 * r;
    let segs = [
        top_right,
        arc(tr),
        right,
        arc(br),
        bottom,
        arc(bl),
        left,
        arc(tl),
        top_left,
    ];
    let perimeter: f64 = segs.iter().sum();
    if perimeter <= 0.0 {
        return ((cx, y), AnchorSide::Vertical);
    }

    let mut distance = perimeter * (index as f64 / count as f64);
    for (i, &len) in segs.iter().enumerate() {
        if len <= 0.0 {
            continue;
        }
        if distance > len {
            distance -= len;
            continue;
        }
        let t = distance / len;
        return match i {
            0 => ((cx + t * top_right, y), AnchorSide::Vertical),
            1 => {
                // TR arc center (x+w-tr, y+tr); angle runs from −π/2 (top) to 0 (right).
                let angle = -std::f64::consts::FRAC_PI_2 + t * std::f64::consts::FRAC_PI_2;
                let px = x + w - tr + tr * math::cos(angle);
                let py = y + tr + tr * math::sin(angle);
                ((px, py), anchor_side_from_center((px, py), (cx, cy)))
            }
            2 => ((x + w, y + tr + t * right), AnchorSide::Horizontal),
            3 => {
                // BR: 0…π/2
                let angle = t * std::f64::consts::FRAC_PI_2;
                let px = x + w - br + br * math::cos(angle);
                let py = y + h - br + br * math::sin(angle);
                ((px, py), anchor_side_from_center((px, py), (cx, cy)))
            }
            4 => ((x + w - br - t * bottom, y + h), AnchorSide::Vertical),
            5 => {
                // BL: π/2…π
                let angle = std::f64::consts::FRAC_PI_2 + t * std::f64::consts::FRAC_PI_2;
                let px = x + bl + bl * math::cos(angle);
                let py = y + h - bl + bl * math::sin(angle);
                ((px, py), anchor_side_from_center((px, py), (cx, cy)))
            }
            6 => ((x, y + h - bl - t * left), AnchorSide::Horizontal),
            7 => {
                // TL: π…3π/2
                let angle = std::f64::consts::PI + t * std::f64::consts::FRAC_PI_2;
                let px = x + tl + tl * math::cos(angle);
                let py = y + tl + tl * math::sin(angle);
                ((px, py), anchor_side_from_center((px, py), (cx, cy)))
            }
            _ => ((x + tl + t * top_left, y), AnchorSide::Vertical),
        };
    }
    ((cx, y), AnchorSide::Vertical)
}

fn divided_ellipse_anchor(
    boxr: (f64, f64, f64, f64),
    index: usize,
    count: usize,
) -> ((f64, f64), AnchorSide) {
    let (x, y, w, h) = boxr;
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let rx = w / 2.0;
    let ry = h / 2.0;
    let angle =
        -std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * (index as f64 / count as f64);
    let px = cx + rx * math::cos(angle);
    let py = cy + ry * math::sin(angle);
    let side = if (px - cx).abs() >= (py - cy).abs() {
        AnchorSide::Horizontal
    } else {
        AnchorSide::Vertical
    };
    ((px, py), side)
}

fn divided_box_anchor(
    boxr: (f64, f64, f64, f64),
    index: usize,
    count: usize,
) -> ((f64, f64), AnchorSide) {
    let (x, y, w, h) = boxr;
    let perimeter = 2.0 * (w + h);
    if perimeter <= 0.0 {
        return ((x + w / 2.0, y), AnchorSide::Vertical);
    }
    let mut distance = perimeter * (index as f64 / count as f64);
    let top_right = w / 2.0;
    if distance <= top_right {
        return ((x + w / 2.0 + distance, y), AnchorSide::Vertical);
    }
    distance -= top_right;
    if distance <= h {
        return ((x + w, y + distance), AnchorSide::Horizontal);
    }
    distance -= h;
    if distance <= w {
        return ((x + w - distance, y + h), AnchorSide::Vertical);
    }
    distance -= w;
    if distance <= h {
        return ((x, y + h - distance), AnchorSide::Horizontal);
    }
    distance -= h;
    ((x + distance, y), AnchorSide::Vertical)
}

fn divided_diamond_anchor(
    boxr: (f64, f64, f64, f64),
    index: usize,
    count: usize,
) -> ((f64, f64), AnchorSide) {
    let (x, y, w, h) = boxr;
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let vertices = [(cx, y), (x + w, cy), (cx, y + h), (x, cy), (cx, y)];
    let segment_len = ((w / 2.0) * (w / 2.0) + (h / 2.0) * (h / 2.0)).sqrt();
    if segment_len <= 0.0 {
        return ((cx, y), AnchorSide::Vertical);
    }
    let mut distance = 4.0 * segment_len * (index as f64 / count as f64);
    for segment in vertices.windows(2) {
        if distance <= segment_len {
            let t = distance / segment_len;
            let px = segment[0].0 + (segment[1].0 - segment[0].0) * t;
            let py = segment[0].1 + (segment[1].1 - segment[0].1) * t;
            return ((px, py), anchor_side_from_center((px, py), (cx, cy)));
        }
        distance -= segment_len;
    }
    ((cx, y), AnchorSide::Vertical)
}

fn divided_capsule_anchor(
    boxr: (f64, f64, f64, f64),
    index: usize,
    count: usize,
) -> ((f64, f64), AnchorSide) {
    let (x, y, w, h) = boxr;
    if w <= h {
        return divided_ellipse_anchor(boxr, index, count);
    }

    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let radius = h / 2.0;
    if radius <= 0.0 {
        return ((cx, y), AnchorSide::Vertical);
    }

    let straight = w - 2.0 * radius;
    let top_half = straight / 2.0;
    let arc_len = std::f64::consts::PI * radius;
    let perimeter = 2.0 * straight + 2.0 * arc_len;
    let mut distance = perimeter * (index as f64 / count as f64);

    if distance <= top_half {
        return ((cx + distance, y), AnchorSide::Vertical);
    }
    distance -= top_half;

    if distance <= arc_len {
        let angle = -std::f64::consts::FRAC_PI_2 + distance / radius;
        let px = x + w - radius + radius * math::cos(angle);
        let py = cy + radius * math::sin(angle);
        return ((px, py), anchor_side_from_center((px, py), (cx, cy)));
    }
    distance -= arc_len;

    if distance <= straight {
        return ((x + w - radius - distance, y + h), AnchorSide::Vertical);
    }
    distance -= straight;

    if distance <= arc_len {
        let angle = std::f64::consts::FRAC_PI_2 + distance / radius;
        let px = x + radius + radius * math::cos(angle);
        let py = cy + radius * math::sin(angle);
        return ((px, py), anchor_side_from_center((px, py), (cx, cy)));
    }
    distance -= arc_len;

    ((x + radius + distance, y), AnchorSide::Vertical)
}

fn anchor_side_from_center(pt: (f64, f64), center: (f64, f64)) -> AnchorSide {
    if (pt.0 - center.0).abs() >= (pt.1 - center.1).abs() {
        AnchorSide::Horizontal
    } else {
        AnchorSide::Vertical
    }
}

/// Resolve a nine-point grid anchor string (e.g. `top-left`, `bottom-center`,
/// `center`) to its point and leave/enter orientation. Returns `None` when the
/// string names no grid position (e.g. `auto`), so the caller falls back to
/// dominant-axis resolution. `mid`/`middle` are synonyms for `center`.
fn grid_anchor(anchor: &str, boxr: (f64, f64, f64, f64)) -> Option<((f64, f64), AnchorSide)> {
    let (x, y, w, h) = boxr;
    let (mut top, mut bottom, mut left, mut right, mut recognized) =
        (false, false, false, false, false);
    for part in anchor.split('-') {
        match part {
            "top" => {
                top = true;
                recognized = true;
            }
            "bottom" => {
                bottom = true;
                recognized = true;
            }
            "left" => {
                left = true;
                recognized = true;
            }
            "right" => {
                right = true;
                recognized = true;
            }
            "center" | "centre" | "mid" | "middle" => {
                recognized = true;
            }
            _ => continue,
        }
    }
    if !recognized {
        return None;
    }
    let px = if left {
        x
    } else if right {
        x + w
    } else {
        x + w / 2.0
    };
    let py = if top {
        y
    } else if bottom {
        y + h
    } else {
        y + h / 2.0
    };
    // Orientation: a pure top/bottom anchor leaves vertically; a pure left/right
    // anchor leaves horizontally; the center and the four corners default to
    // horizontal (matching the pre-grid `center` behavior).
    let vertical_only = (top || bottom) && !(left || right);
    let side = if vertical_only {
        AnchorSide::Vertical
    } else {
        AnchorSide::Horizontal
    };
    Some(((px, py), side))
}
