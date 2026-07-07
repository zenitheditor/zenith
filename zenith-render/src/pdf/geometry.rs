//! Geometry helpers for the PDF backend: rounded-rect and ellipse bezier path
//! emission into a `pdf_writer::Content`, plus the glyph outline pen.
//!
//! All emitters append path-construction operators (`m`, `l`, `c`, `h`) to the
//! content stream but never paint — the caller chooses the paint operator
//! (`f`, `S`, `W n`, …) afterwards. Coordinates are in scene space; the page's
//! initial flip CTM maps them to PDF user space, so no per-point flip is done
//! here.

use pdf_writer::Content;
use zenith_scene::ir::PathSegment;

/// Circle-approximation constant κ for a 90° cubic arc (matches the raster
/// backend's `build_rounded_rect_path`).
const KAPPA: f64 = 0.552_284_8;

/// Append a rounded-rectangle subpath with per-corner radii `[tl, tr, br, bl]`
/// (index 0=top-left, 1=top-right, 2=bottom-right, 3=bottom-left) to `content`.
/// Each corner radius is clamped independently to `min(w, h) / 2`. A radius of 0
/// produces a sharp corner. Does nothing for a degenerate box.
///
/// Path order matches the raster backend: move-to top-left-start → right along
/// top → top-right arc → down right → bottom-right arc → left along bottom →
/// bottom-left arc → up left → top-left arc → close.
pub(super) fn rounded_rect_path(
    content: &mut Content,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    radii: [f64; 4],
) {
    if !(w > 0.0 && h > 0.0) {
        return;
    }
    let half_min = (w / 2.0).min(h / 2.0);
    let tl = radii[0].max(0.0).min(half_min);
    let tr = radii[1].max(0.0).min(half_min);
    let br = radii[2].max(0.0).min(half_min);
    let bl = radii[3].max(0.0).min(half_min);

    let ktl = (KAPPA * tl) as f32;
    let ktr = (KAPPA * tr) as f32;
    let kbr = (KAPPA * br) as f32;
    let kbl = (KAPPA * bl) as f32;
    let (tl, tr, br, bl) = (tl as f32, tr as f32, br as f32, bl as f32);
    let (x, y, w, h) = (x as f32, y as f32, w as f32, h as f32);

    content.move_to(x + tl, y);
    content.line_to(x + w - tr, y);
    if tr > 0.0 {
        content.cubic_to(x + w - tr + ktr, y, x + w, y + tr - ktr, x + w, y + tr);
    }
    content.line_to(x + w, y + h - br);
    if br > 0.0 {
        content.cubic_to(
            x + w,
            y + h - br + kbr,
            x + w - br + kbr,
            y + h,
            x + w - br,
            y + h,
        );
    }
    content.line_to(x + bl, y + h);
    if bl > 0.0 {
        content.cubic_to(x + bl - kbl, y + h, x, y + h - bl + kbl, x, y + h - bl);
    }
    content.line_to(x, y + tl);
    if tl > 0.0 {
        content.cubic_to(x, y + tl - ktl, x + tl - ktl, y, x + tl, y);
    }
    content.close_path();
}

/// Append a full ellipse subpath to `content` as four cubic bezier arcs.
///
/// `rx_override`/`ry_override`: when `Some`, use the given semi-axis length;
/// when `None`, the semi-axis is derived from `w`/`h` (inscribed ellipse,
/// byte-identical to the prior behavior). The oval is centered in the node
/// bbox `[x, y, w, h]`. Does nothing for a degenerate box.
pub(super) fn ellipse_path(
    content: &mut Content,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    rx_override: Option<f64>,
    ry_override: Option<f64>,
) {
    if !(w > 0.0 && h > 0.0) {
        return;
    }
    let rx = rx_override.unwrap_or(w / 2.0);
    let ry = ry_override.unwrap_or(h / 2.0);
    // Center the oval in the node bbox.
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let kx = (KAPPA * rx) as f32;
    let ky = (KAPPA * ry) as f32;
    let (cx, cy, rx, ry) = (cx as f32, cy as f32, rx as f32, ry as f32);
    // Start at the rightmost point, go clockwise (in scene/y-down space).
    content.move_to(cx + rx, cy);
    content.cubic_to(cx + rx, cy + ky, cx + kx, cy + ry, cx, cy + ry); // → bottom
    content.cubic_to(cx - kx, cy + ry, cx - rx, cy + ky, cx - rx, cy); // → left
    content.cubic_to(cx - rx, cy - ky, cx - kx, cy - ry, cx, cy - ry); // → top
    content.cubic_to(cx + kx, cy - ry, cx + rx, cy - ky, cx + rx, cy); // → right
    content.close_path();
}

/// Append a flat `[x0, y0, x1, y1, …]` polygon/polyline subpath to `content`.
///
/// `closed` closes the subpath (polygon outline / fill). Returns `false` and
/// emits nothing when fewer than two vertices are present.
pub(super) fn poly_path(content: &mut Content, points: &[f64], closed: bool) -> bool {
    let (Some(&x0), Some(&y0)) = (points.first(), points.get(1)) else {
        return false;
    };
    content.move_to(x0 as f32, y0 as f32);
    let mut i = 2;
    while i + 1 < points.len() {
        let (Some(&px), Some(&py)) = (points.get(i), points.get(i + 1)) else {
            break;
        };
        content.line_to(px as f32, py as f32);
        i += 2;
    }
    if closed {
        content.close_path();
    }
    true
}

/// Axis-aligned bounding box `(x, y, w, h)` of a flat `[x0,y0,x1,y1,...]`
/// point list.
pub(super) fn poly_bbox(points: &[f64]) -> (f64, f64, f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for pair in points.chunks_exact(2) {
        let &[x, y] = pair else { continue };
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    (min_x, min_y, max_x - min_x, max_y - min_y)
}

/// Append structured scene path segments to `content`.
pub(super) fn scene_path(content: &mut Content, segments: &[PathSegment]) -> bool {
    let mut subpath_open = false;
    let mut produced = false;
    for segment in segments {
        match segment {
            PathSegment::MoveTo { x, y } => {
                content.move_to(*x as f32, *y as f32);
                subpath_open = true;
                produced = true;
            }
            PathSegment::LineTo { x, y } if subpath_open => {
                content.line_to(*x as f32, *y as f32);
            }
            PathSegment::CubicTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } if subpath_open => {
                content.cubic_to(
                    *x1 as f32, *y1 as f32, *x2 as f32, *y2 as f32, *x as f32, *y as f32,
                );
            }
            PathSegment::Close if subpath_open => {
                content.close_path();
                subpath_open = false;
            }
            PathSegment::LineTo { .. } | PathSegment::CubicTo { .. } | PathSegment::Close => {
                return false;
            }
        }
    }
    produced
}

// ── Glyph outline pen ─────────────────────────────────────────────────────────

/// A `ttf_parser::OutlineBuilder` that emits glyph outline segments as PDF
/// path-construction operators into a `pdf_writer::Content`.
///
/// Mirrors the raster backend's `GlyphOutlinePen`, but targets a PDF content
/// buffer. Font coordinates are y-UP with the origin at the glyph origin; the
/// transform applied per point is `px = origin_x + fx*scale`,
/// `py = baseline_y - fy*scale`, matching the raster pen exactly so PDF text
/// outlines align with the rasterized reference. (The page-level flip CTM then
/// maps these y-down scene coordinates back to y-up PDF space.) Quadratic
/// segments are promoted to cubics because PDF has no quadratic operator.
pub(super) struct GlyphPen<'a> {
    content: &'a mut Content,
    origin_x: f32,
    baseline_y: f32,
    scale: f32,
    /// Current pen position in scene coordinates, needed to elevate a TrueType
    /// quadratic to a cubic.
    cur: (f32, f32),
}

impl<'a> GlyphPen<'a> {
    pub(super) fn new(
        content: &'a mut Content,
        origin_x: f32,
        baseline_y: f32,
        scale: f32,
    ) -> Self {
        Self {
            content,
            origin_x,
            baseline_y,
            scale,
            cur: (0.0, 0.0),
        }
    }

    #[inline]
    fn map(&self, fx: f32, fy: f32) -> (f32, f32) {
        (
            self.origin_x + fx * self.scale,
            self.baseline_y - fy * self.scale,
        )
    }
}

impl ttf_parser::OutlineBuilder for GlyphPen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let (px, py) = self.map(x, y);
        self.content.move_to(px, py);
        self.cur = (px, py);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let (px, py) = self.map(x, y);
        self.content.line_to(px, py);
        self.cur = (px, py);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        // Elevate the quadratic (p0, c, p1) to a cubic with control points
        // c1 = p0 + 2/3 (c - p0), c2 = p1 + 2/3 (c - p1).
        let (cx, cy) = self.map(x1, y1);
        let (px, py) = self.map(x, y);
        let (p0x, p0y) = self.cur;
        let c1x = p0x + 2.0 / 3.0 * (cx - p0x);
        let c1y = p0y + 2.0 / 3.0 * (cy - p0y);
        let c2x = px + 2.0 / 3.0 * (cx - px);
        let c2y = py + 2.0 / 3.0 * (cy - py);
        self.content.cubic_to(c1x, c1y, c2x, c2y, px, py);
        self.cur = (px, py);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (c1x, c1y) = self.map(x1, y1);
        let (c2x, c2y) = self.map(x2, y2);
        let (px, py) = self.map(x, y);
        self.content.cubic_to(c1x, c1y, c2x, c2y, px, py);
        self.cur = (px, py);
    }

    fn close(&mut self) {
        self.content.close_path();
    }
}
