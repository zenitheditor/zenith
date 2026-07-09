//! Path geometry for connectors: orthogonal (elbow) routing, self-loop paths,
//! outward face normals, polyline midpoint/point sampling, and arrowhead
//! construction.

use super::anchor::AnchorSide;

/// Build a flat right-angle (elbow) point list between two anchors, with the
/// first segment perpendicular to `f`'s edge (`fs`) and the last perpendicular to
/// `t`'s edge (`ts`). Returns 8 coords (4-point Z-route) when both anchors share
/// an orientation, or 6 coords (3-point L-corner) when they differ.
///
/// Collinear/degenerate elbows (e.g. `mx == f.0`) are left as-is — zero-length
/// sub-segments render harmlessly and are never special-cased.
pub(super) fn orthogonal_route(
    f: (f64, f64),
    fs: AnchorSide,
    t: (f64, f64),
    ts: AnchorSide,
) -> Vec<f64> {
    match (fs, ts) {
        // Both side edges → H–V–H Z-route, elbow at the mid x.
        (AnchorSide::Horizontal, AnchorSide::Horizontal) => {
            let mx = (f.0 + t.0) / 2.0;
            vec![f.0, f.1, mx, f.1, mx, t.1, t.0, t.1]
        }
        // Both top/bottom edges → V–H–V Z-route, elbow at the mid y.
        (AnchorSide::Vertical, AnchorSide::Vertical) => {
            let my = (f.1 + t.1) / 2.0;
            vec![f.0, f.1, f.0, my, t.0, my, t.0, t.1]
        }
        // Leaves F horizontally, enters T vertically → corner at (t.0, f.1).
        (AnchorSide::Horizontal, AnchorSide::Vertical) => {
            vec![f.0, f.1, t.0, f.1, t.0, t.1]
        }
        // Leaves F vertically, enters T horizontally → corner at (f.0, t.1).
        (AnchorSide::Vertical, AnchorSide::Horizontal) => {
            vec![f.0, f.1, f.0, t.1, t.0, t.1]
        }
    }
}

/// The unit outward normal of an anchor point on its box face, used to stub the
/// `route="avoid"` path cleanly out of (and into) each box. A `Horizontal` edge
/// (left/right) leaves along ±x toward the side the anchor sits on; a `Vertical`
/// edge (top/bottom) leaves along ±y.
pub(super) fn outward_dir(
    side: AnchorSide,
    pt: (f64, f64),
    boxr: (f64, f64, f64, f64),
) -> (f64, f64) {
    let cx = boxr.0 + boxr.2 / 2.0;
    let cy = boxr.1 + boxr.3 / 2.0;
    match side {
        AnchorSide::Horizontal => {
            let sign = if pt.0 - cx >= 0.0 { 1.0 } else { -1.0 };
            (sign, 0.0)
        }
        AnchorSide::Vertical => {
            let sign = if pt.1 - cy >= 0.0 { 1.0 } else { -1.0 };
            (0.0, sign)
        }
    }
}

/// How far a self-loop bulges out from the box edge, in pixels.
const LOOP_DEPTH: f64 = 28.0;
/// The largest half-width of a self-loop's two feet on the box edge, in pixels;
/// the actual half-width is also capped at 30% of the spanning box dimension so
/// the loop stays within the edge on small boxes.
const LOOP_HALF_MAX: f64 = 25.0;

/// Pick which edge a self-loop bulges from, parsed loosely from an anchor string
/// (`top`/`bottom`/`left`/`right`); anything else (including `auto`/absent)
/// defaults to the top edge.
pub(super) fn loop_side(anchor: Option<&str>) -> &'static str {
    match anchor {
        Some(a) if a.contains("bottom") => "bottom",
        Some(a) if a.contains("left") => "left",
        Some(a) if a.contains("right") => "right",
        _ => "top",
    }
}

/// Build a small rectangular self-loop off one `side` of a `(x, y, w, h)` box:
/// two feet on that edge, bulging out by [`LOOP_DEPTH`]. The path runs foot →
/// out → across → back → foot, so the last segment arrives perpendicular to the
/// edge and an end marker points back into the box.
pub(super) fn self_loop_path(boxr: (f64, f64, f64, f64), side: &str) -> Vec<f64> {
    let (x, y, w, h) = boxr;
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let d = LOOP_DEPTH;
    match side {
        "bottom" => {
            let half = (w * 0.3).min(LOOP_HALF_MAX);
            let yb = y + h;
            vec![
                cx - half,
                yb,
                cx - half,
                yb + d,
                cx + half,
                yb + d,
                cx + half,
                yb,
            ]
        }
        "left" => {
            let half = (h * 0.3).min(LOOP_HALF_MAX);
            vec![
                x,
                cy - half,
                x - d,
                cy - half,
                x - d,
                cy + half,
                x,
                cy + half,
            ]
        }
        "right" => {
            let half = (h * 0.3).min(LOOP_HALF_MAX);
            let xr = x + w;
            vec![
                xr,
                cy - half,
                xr + d,
                cy - half,
                xr + d,
                cy + half,
                xr,
                cy + half,
            ]
        }
        _ => {
            let half = (w * 0.3).min(LOOP_HALF_MAX);
            vec![
                cx - half,
                y,
                cx - half,
                y - d,
                cx + half,
                y - d,
                cx + half,
                y,
            ]
        }
    }
}

/// Bounds-safe read of the `i`-th `(x, y)` point from a flat `[x0,y0,x1,y1,…]`
/// list. Returns `None` if the point is out of range (no panic, no indexing).
pub(super) fn point_at(pts: &[f64], i: usize) -> Option<(f64, f64)> {
    let x = pts.get(i * 2)?;
    let y = pts.get(i * 2 + 1)?;
    Some((*x, *y))
}

/// Compute the geometric midpoint of a flat `[x0,y0, x1,y1, …]` polyline.
///
/// The midpoint is the point at half the total arc-length of the polyline.
/// Returns `None` when the list has fewer than two points (degenerate). The
/// computation is deterministic and allocation-free beyond the input slice.
pub(super) fn polyline_midpoint(pts: &[f64]) -> Option<(f64, f64)> {
    let n = pts.len() / 2;
    if n < 2 {
        return None;
    }

    // Accumulate segment lengths.
    let mut total = 0.0_f64;
    for i in 0..n.saturating_sub(1) {
        let (x0, y0) = (pts[i * 2], pts[i * 2 + 1]);
        let (x1, y1) = (pts[i * 2 + 2], pts[i * 2 + 3]);
        let dx = x1 - x0;
        let dy = y1 - y0;
        total += (dx * dx + dy * dy).sqrt();
    }

    // Walk to the half-length point.
    let half = total / 2.0;
    let mut walked = 0.0_f64;
    for i in 0..n.saturating_sub(1) {
        let (x0, y0) = (pts[i * 2], pts[i * 2 + 1]);
        let (x1, y1) = (pts[i * 2 + 2], pts[i * 2 + 3]);
        let dx = x1 - x0;
        let dy = y1 - y0;
        let seg_len = (dx * dx + dy * dy).sqrt();
        if walked + seg_len >= half {
            let t = if seg_len < 1e-9 {
                0.0
            } else {
                (half - walked) / seg_len
            };
            return Some((x0 + t * dx, y0 + t * dy));
        }
        walked += seg_len;
    }

    // Fallback: last point (handles floating-point rounding at total == half).
    let last = n - 1;
    Some((pts[last * 2], pts[last * 2 + 1]))
}

/// Build a filled-triangle arrowhead whose tip sits at `tip`, arriving along the
/// segment from `from_pt` → `tip` (so the head points in the direction of travel
/// into `tip`). Returns a flat `[x0,y0, x1,y1, x2,y2]` (tip, left base, right
/// base), or `None` if the segment is degenerate (endpoints coincide) and the
/// head cannot be oriented. Size scales with `stroke_width`, clamped so thin
/// strokes still get a visible head.
pub(super) fn arrowhead_points(
    tip: (f64, f64),
    from_pt: (f64, f64),
    stroke_width: f64,
) -> Option<Vec<f64>> {
    let vx = tip.0 - from_pt.0;
    let vy = tip.1 - from_pt.1;
    let len = (vx * vx + vy * vy).sqrt();
    if len < 1e-6 {
        return None;
    }
    let (ux, uy) = (vx / len, vy / len);
    let (px, py) = (-uy, ux);
    // head_len: 3.5× stroke; half_w: 2.0× stroke — clamped so hairline strokes
    // still produce a visible 7px × 8px head.
    let head_len = (stroke_width * 3.5).max(7.0);
    let half_w = (stroke_width * 2.0).max(4.0);
    let base_cx = tip.0 - ux * head_len;
    let base_cy = tip.1 - uy * head_len;
    let left_x = base_cx + px * half_w;
    let left_y = base_cy + py * half_w;
    let right_x = base_cx - px * half_w;
    let right_y = base_cy - py * half_w;
    Some(vec![tip.0, tip.1, left_x, left_y, right_x, right_y])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loop_side_parses_edges_and_defaults_to_top() {
        assert_eq!(loop_side(Some("bottom-center")), "bottom");
        assert_eq!(loop_side(Some("center-left")), "left");
        assert_eq!(loop_side(Some("right")), "right");
        assert_eq!(loop_side(Some("top")), "top");
        assert_eq!(loop_side(Some("auto")), "top");
        assert_eq!(loop_side(None), "top");
    }

    #[test]
    fn self_loop_top_bulges_above_the_box() {
        // 120×60 box at (60,110): top edge y=110, center x=120.
        let pts = self_loop_path((60.0, 110.0, 120.0, 60.0), "top");
        // Four points: foot, out, across, foot — all above the top edge.
        assert_eq!(pts.len(), 8);
        let half = (120.0_f64 * 0.3).min(LOOP_HALF_MAX);
        assert_eq!(
            pts,
            vec![
                120.0 - half,
                110.0,
                120.0 - half,
                110.0 - LOOP_DEPTH,
                120.0 + half,
                110.0 - LOOP_DEPTH,
                120.0 + half,
                110.0,
            ]
        );
        // The two feet sit on the top edge; the bulge is strictly above it.
        assert!(pts[3] < 110.0 && pts[5] < 110.0);
    }

    #[test]
    fn self_loop_right_bulges_past_the_right_edge() {
        // 120×60 box at (250,110): right edge x=370, center y=140.
        let pts = self_loop_path((250.0, 110.0, 120.0, 60.0), "right");
        assert_eq!(pts.len(), 8);
        // Feet on the right edge (x=370); bulge extends past it.
        assert_eq!(pts[0], 370.0);
        assert_eq!(pts[6], 370.0);
        assert!(pts[2] > 370.0 && pts[4] > 370.0);
    }

    #[test]
    fn self_loop_is_deterministic() {
        let a = self_loop_path((10.0, 20.0, 80.0, 40.0), "bottom");
        let b = self_loop_path((10.0, 20.0, 80.0, 40.0), "bottom");
        assert_eq!(a, b);
    }
}
