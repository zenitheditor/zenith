//! Anchor-point geometry for connectors: resolving an anchor string (grid,
//! `auto`, or a divided `i/N`) to a page-absolute point on a target box and the
//! orientation the routed path leaves/enters through.

use zenith_core::ast::{ConnectorAnchor, parse_connector_anchor};

use crate::compile::field::ConnectorTargetKind;

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
pub(super) fn resolve_anchor(
    boxr: (f64, f64, f64, f64),
    kind: ConnectorTargetKind,
    anchor: &str,
    toward: (f64, f64),
) -> ((f64, f64), AnchorSide) {
    let (x, y, w, h) = boxr;
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;

    if let Ok(ConnectorAnchor::Divided { index, count }) = parse_connector_anchor(anchor) {
        return divided_anchor(boxr, kind, index, count);
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
) -> ((f64, f64), AnchorSide) {
    match kind {
        // `ApproxOutline` (rounded rect, `process` shape, polygon/polyline/path)
        // resolves on the bounds perimeter EXACTLY like `BoxLike` — the outline
        // approximation is intentional and byte-identical to the box fallback.
        ConnectorTargetKind::BoxLike | ConnectorTargetKind::ApproxOutline => {
            divided_box_anchor(boxr, index, count)
        }
        ConnectorTargetKind::Capsule => divided_capsule_anchor(boxr, index, count),
        ConnectorTargetKind::Diamond => divided_diamond_anchor(boxr, index, count),
        ConnectorTargetKind::Ellipse => divided_ellipse_anchor(boxr, index, count),
    }
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
    let px = cx + rx * angle.cos();
    let py = cy + ry * angle.sin();
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
        let px = x + w - radius + radius * angle.cos();
        let py = cy + radius * angle.sin();
        return ((px, py), anchor_side_from_center((px, py), (cx, cy)));
    }
    distance -= arc_len;

    if distance <= straight {
        return ((x + w - radius - distance, y + h), AnchorSide::Vertical);
    }
    distance -= straight;

    if distance <= arc_len {
        let angle = std::f64::consts::FRAC_PI_2 + distance / radius;
        let px = x + radius + radius * angle.cos();
        let py = cy + radius * angle.sin();
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
