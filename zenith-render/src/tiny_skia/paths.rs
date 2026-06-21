//! Geometry / path helpers for the tiny-skia backend: rect intersection, clip
//! mask construction, polygon and rounded-rect path builders, and the glyph
//! outline pen.

use tiny_skia::{FillRule, Mask, Path, PathBuilder, Rect, Transform};
use zenith_scene::StrokeAlign;

/// Build a clip `Mask` from the current effective clip rectangle.
///
/// Returns:
/// - `None` — the effective clip is empty or fully off-canvas; the caller
///   should skip the draw entirely (`continue`).
/// - `Some(None)` — the clip covers the whole pixmap; no masking needed,
///   draw with `mask = None` (the common, no-frame case — avoids allocating
///   a full-size mask on every top-level draw).
/// - `Some(Some(mask))` — a real sub-page clip; draw with `mask = Some(&mask)`.
pub(super) fn clip_mask(
    effective_clip: (f64, f64, f64, f64),
    width: u32,
    height: u32,
) -> Option<Option<Mask>> {
    let pixmap_bounds = (0.0, 0.0, f64::from(width), f64::from(height));
    let (cx, cy, cx2, cy2) = intersect_rects(effective_clip, pixmap_bounds)?; // empty → None (skip)
    // If the clip covers the entire pixmap, no mask is needed.
    if cx <= 0.0 && cy <= 0.0 && cx2 >= f64::from(width) && cy2 >= f64::from(height) {
        return Some(None);
    }
    let mut mask = Mask::new(width, height)?;
    let rect = Rect::from_xywh(cx as f32, cy as f32, (cx2 - cx) as f32, (cy2 - cy) as f32)?;
    let clip_path = PathBuilder::from_rect(rect);
    // AA off: the clip is an axis-aligned rect and must be exact.
    mask.fill_path(&clip_path, FillRule::Winding, false, Transform::identity());
    Some(Some(mask))
}

/// Build the clip `Mask` for an `Inside`/`Outside`-aligned polygon stroke.
///
/// The stroke is drawn at 2× width centered on the path; this mask keeps only
/// the half that lies inside (`Inside`) or outside (`Outside`) the polygon's
/// fill region, yielding a full-width stroke flush against the boundary. The
/// fill region is rasterized using the polygon's fill rule (`fill_even_odd`),
/// anti-aliased, under `device_ts` so it lands in the same device space as the
/// stroke (rotation handled). For `Outside`, the mask is inverted.
///
/// When the effective frame clip is a real sub-page rect (not the whole pixmap),
/// the alignment mask is additionally intersected with that rect so frame
/// clipping still applies.
///
/// Returns `None` on any degenerate input (mask allocation failure, empty path).
/// The caller must fall back to centered stroking in that case — never panic.
pub(super) fn build_align_mask(
    points: &[f64],
    align: StrokeAlign,
    fill_even_odd: bool,
    effective_clip: (f64, f64, f64, f64),
    width: u32,
    height: u32,
    device_ts: Transform,
) -> Option<Mask> {
    // Inside/Outside only — Center never reaches here.
    let invert = match align {
        StrokeAlign::Inside => false,
        StrokeAlign::Outside => true,
        StrokeAlign::Center => return None,
    };

    let fill_path = build_poly_path(points, true)?;
    let mut mask = Mask::new(width, height)?;
    let fill_rule = if fill_even_odd {
        FillRule::EvenOdd
    } else {
        FillRule::Winding
    };
    mask.fill_path(&fill_path, fill_rule, true, device_ts);
    if invert {
        mask.invert();
    }

    // Intersect with the frame clip rect when it is a real sub-page clip.
    let pixmap_bounds = (0.0, 0.0, f64::from(width), f64::from(height));
    if let Some((cx, cy, cx2, cy2)) = intersect_rects(effective_clip, pixmap_bounds) {
        let full_page =
            cx <= 0.0 && cy <= 0.0 && cx2 >= f64::from(width) && cy2 >= f64::from(height);
        if !full_page
            && let Some(rect) =
                Rect::from_xywh(cx as f32, cy as f32, (cx2 - cx) as f32, (cy2 - cy) as f32)
        {
            let clip_path = PathBuilder::from_rect(rect);
            // effective_clip is device-space → identity transform, AA off (exact rect).
            mask.intersect_path(&clip_path, FillRule::Winding, false, Transform::identity());
        }
    }

    Some(mask)
}

/// Intersect two axis-aligned rectangles expressed as `(x, y, x2, y2)`.
///
/// Returns `None` when the intersection is empty.
pub(super) fn intersect_rects(
    (ax, ay, ax2, ay2): (f64, f64, f64, f64),
    (bx, by, bx2, by2): (f64, f64, f64, f64),
) -> Option<(f64, f64, f64, f64)> {
    let ix = ax.max(bx);
    let iy = ay.max(by);
    let ix2 = ax2.min(bx2);
    let iy2 = ay2.min(by2);
    if ix < ix2 && iy < iy2 {
        Some((ix, iy, ix2, iy2))
    } else {
        None
    }
}

/// Build a `tiny_skia::Path` from a flat `[x0, y0, x1, y1, …]` point list.
///
/// `closed` — when `true` the path is closed after the final vertex (polygon);
/// when `false` the path is left open (polyline stroke).
///
/// Returns `None` when the path is degenerate (e.g. zero-length). The caller
/// must have already verified that `points` contains at least 4 elements (2
/// vertices) and that all values are finite before calling this function.
pub(super) fn build_poly_path(points: &[f64], closed: bool) -> Option<Path> {
    let mut pb = PathBuilder::new();
    // Safety: caller guarantees points.len() >= 4; first() / get(1) always Some.
    let (x0, y0) = match (points.first(), points.get(1)) {
        (Some(&x), Some(&y)) => (x as f32, y as f32),
        _ => return None,
    };
    pb.move_to(x0, y0);
    let mut i = 2;
    while i + 1 < points.len() {
        let (px, py) = match (points.get(i), points.get(i + 1)) {
            (Some(&x), Some(&y)) => (x as f32, y as f32),
            _ => break,
        };
        pb.line_to(px, py);
        i += 2;
    }
    if closed {
        pb.close();
    }
    pb.finish()
}

/// Build a closed rounded-rectangle path with per-corner radii
/// `[tl, tr, br, bl]` (index 0=top-left, 1=top-right, 2=bottom-right,
/// 3=bottom-left). Each corner radius is clamped independently to
/// `min(w, h) / 2`. A radius of 0 produces a sharp corner (no cubic arc).
/// Corners with radius > 0 use cubic Béziers with κ ≈ 0.5522848.
///
/// Path order (same as the former uniform variant):
/// move-to top-left-start → right along top → top-right arc →
/// down right side → bottom-right arc → left along bottom →
/// bottom-left arc → up left side → top-left arc → close.
pub(super) fn build_rounded_rect_path(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radii: [f32; 4],
) -> Option<Path> {
    if !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 {
        return None;
    }
    // Clamp each corner independently.
    let half_min = (w / 2.0).min(h / 2.0);
    let tl = radii[0].max(0.0).min(half_min);
    let tr = radii[1].max(0.0).min(half_min);
    let br = radii[2].max(0.0).min(half_min);
    let bl = radii[3].max(0.0).min(half_min);

    const K: f32 = 0.552_284_8_f32; // κ: cubic control-point ratio for 90° arc
    let ktl = K * tl;
    let ktr = K * tr;
    let kbr = K * br;
    let kbl = K * bl;

    let mut pb = PathBuilder::new();

    // Start at the top-left corner's top-edge departure point.
    pb.move_to(x + tl, y);
    // → top edge to top-right corner
    pb.line_to(x + w - tr, y);
    // top-right arc
    if tr > 0.0 {
        pb.cubic_to(x + w - tr + ktr, y, x + w, y + tr - ktr, x + w, y + tr);
    }
    // → right edge down to bottom-right corner
    pb.line_to(x + w, y + h - br);
    // bottom-right arc
    if br > 0.0 {
        pb.cubic_to(
            x + w,
            y + h - br + kbr,
            x + w - br + kbr,
            y + h,
            x + w - br,
            y + h,
        );
    }
    // → bottom edge to bottom-left corner
    pb.line_to(x + bl, y + h);
    // bottom-left arc
    if bl > 0.0 {
        pb.cubic_to(x + bl - kbl, y + h, x, y + h - bl + kbl, x, y + h - bl);
    }
    // → left edge up to top-left corner
    pb.line_to(x, y + tl);
    // top-left arc
    if tl > 0.0 {
        pb.cubic_to(x, y + tl - ktl, x + tl - ktl, y, x + tl, y);
    }
    pb.close();
    pb.finish()
}

// ── Glyph outline pen ─────────────────────────────────────────────────────────

/// An `OutlineBuilder` that feeds ttf-parser outline commands into a
/// `tiny_skia::PathBuilder`, applying the Y-flip and scale transform needed to
/// map from font-units (Y-up) to pixmap coordinates (Y-down).
///
/// Font coordinate system: Y increases upward, origin at glyph origin.
/// Pixmap coordinate system: Y increases downward, origin at top-left.
///
/// Transform applied per point: `px = origin_x + fx * scale`,
///                               `py = baseline_y - fy * scale`.
pub(super) struct GlyphOutlinePen {
    pub(super) builder: PathBuilder,
    origin_x: f32,
    baseline_y: f32,
    scale: f32,
}

impl GlyphOutlinePen {
    pub(super) fn new(origin_x: f32, baseline_y: f32, scale: f32) -> Self {
        Self {
            builder: PathBuilder::new(),
            origin_x,
            baseline_y,
            scale,
        }
    }

    /// Map a font-unit point `(fx, fy)` to pixmap coordinates.
    #[inline]
    fn to_px(&self, fx: f32, fy: f32) -> (f32, f32) {
        let px = self.origin_x + fx * self.scale;
        let py = self.baseline_y - fy * self.scale;
        (px, py)
    }
}

impl ttf_parser::OutlineBuilder for GlyphOutlinePen {
    fn move_to(&mut self, x: f32, y: f32) {
        let (px, py) = self.to_px(x, y);
        self.builder.move_to(px, py);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let (px, py) = self.to_px(x, y);
        self.builder.line_to(px, py);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (px1, py1) = self.to_px(x1, y1);
        let (px, py) = self.to_px(x, y);
        self.builder.quad_to(px1, py1, px, py);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (px1, py1) = self.to_px(x1, y1);
        let (px2, py2) = self.to_px(x2, y2);
        let (px, py) = self.to_px(x, y);
        self.builder.cubic_to(px1, py1, px2, py2, px, py);
    }

    fn close(&mut self) {
        self.builder.close();
    }
}
