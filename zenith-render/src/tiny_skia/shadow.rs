//! Drop-shadow / outer-glow compositing for shadowed leaf nodes.
//!
//! The node's ink is captured into an offscreen `Pixmap` (premultiplied RGBA8).
//! At EndShadow, each shadow layer is derived from the ink's coverage (alpha),
//! tinted with the layer color, blurred, and composited behind the crisp ink.
//!
//! Blur uses a deterministic three-box approximation of a Gaussian
//! (Ivan Kuchin / "Fastest Gaussian Blur" 3-box method,
//! <http://blog.ivank.net/fastest-gaussian-blur.html>). All arithmetic uses
//! fixed integer/float evaluation order with consistent rounding, so output is
//! byte-identical across runs (no time, randomness, or hashing).

use tiny_skia::{Pixmap, PixmapPaint};
use zenith_scene::ShadowSpec;

use super::crop::{Region, blur_crop, copy_region, draw_at, draw_region, ink_bbox};

/// Local alias for the scene `Color` carried inside a `ShadowSpec`, so this
/// helper does not need to import the scene `Color` name (which would collide
/// with tiny-skia's `Color`). Resolved at call sites via `spec.color`.
type SceneColor = zenith_scene::Color;

/// Paint all shadow layers of one capture onto `canvas`, then the crisp ink.
///
/// Layers are painted in REVERSE declared order so the first-declared layer ends
/// up on top of later layers (all behind the ink). `canvas` and `ink` are
/// page-sized. Each layer works only on the ink's bounding box grown by the
/// layer's blur reach (see `crop`). Empty ink draws nothing.
pub(super) fn composite_shadows(canvas: &mut Pixmap, ink: &Pixmap, shadows: &[ShadowSpec]) {
    let Some(bbox) = ink_bbox(ink) else {
        return;
    };
    let (width, height) = (ink.width(), ink.height());
    let paint = PixmapPaint::default(); // source-over, opacity 1.0
    for spec in shadows.iter().rev() {
        let Some(crop) = blur_crop(bbox, spec.blur, width, height) else {
            continue;
        };
        // Build the tinted shadow coverage from the ink's straight alpha.
        let Some(mut shadow) = Pixmap::new(crop.w, crop.h) else {
            continue;
        };
        tint_coverage(&mut shadow, ink, crop, spec.color);

        // Blur in premultiplied space (correct for source-over compositing).
        gaussian_blur_premul(&mut shadow, spec.blur);

        let (ox, oy) = (round_offset(spec.dx), round_offset(spec.dy));

        // Composite at the crop origin plus the rounded integer offset.
        // `draw_at` places pixels exactly, also for a negative offset.
        let dx = i64::from(crop.x) + i64::from(ox);
        let dy = i64::from(crop.y) + i64::from(oy);
        draw_at(canvas, &shadow, dx, dy, &paint);
    }

    // Crisp ink on top of every shadow.
    draw_region(canvas, ink, bbox, &paint);
}

/// Blur page-sized `ink` by `sigma` and composite it onto `target`.
///
/// The blur runs on the ink's bounding box grown by the blur reach. Empty ink
/// draws nothing. On allocation failure the full page is blurred in place.
pub(super) fn composite_blur(target: &mut Pixmap, mut ink: Pixmap, sigma: f64) {
    let Some(bbox) = ink_bbox(&ink) else {
        return;
    };
    let paint = PixmapPaint::default();
    let crop = blur_crop(bbox, sigma, ink.width(), ink.height());
    if let Some(crop) = crop
        && let Some(mut part) = copy_region(&ink, crop)
    {
        gaussian_blur_premul(&mut part, sigma);
        draw_at(target, &part, i64::from(crop.x), i64::from(crop.y), &paint);
        return;
    }
    gaussian_blur_premul(&mut ink, sigma);
    draw_at(target, &ink, 0, 0, &paint);
}

/// Round a shadow offset to the nearest integer pixel, deterministically and
/// without panicking. `f64::round` ties away from zero; non-finite collapses to 0.
fn round_offset(v: f64) -> i32 {
    if !v.is_finite() {
        return 0;
    }
    let r = v.round();
    if r >= i32::MAX as f64 {
        i32::MAX
    } else if r <= i32::MIN as f64 {
        i32::MIN
    } else {
        r as i32
    }
}

/// Fill `shadow` (premultiplied RGBA8) with the shadow color, modulated by the
/// alpha of `region` of the page-sized `ink`.
///
/// For each pixel: straight alpha = `ink_alpha * (color.a / 255)`, color =
/// `color.rgb`; written PREMULTIPLIED. `shadow` has the size of `region`.
/// Iterates in lockstep via `chunks_exact(4)`, which guarantees exactly 4 bytes
/// per chunk; direct indexing is panic-free. Rows outside `ink` stay zero.
fn tint_coverage(shadow: &mut Pixmap, ink: &Pixmap, region: Region, color: SceneColor) {
    let ca = u32::from(color.a);
    let cr = u32::from(color.r);
    let cg = u32::from(color.g);
    let cb = u32::from(color.b);
    let ink_stride = ink.width() as usize * 4;
    let row_len = region.w as usize * 4;
    let x_off = region.x as usize * 4;
    let src_data = ink.data();
    for (dy, dst_row) in shadow.data_mut().chunks_exact_mut(row_len).enumerate() {
        let start = (region.y as usize + dy) * ink_stride + x_off;
        let Some(src_row) = src_data.get(start..start + row_len) else {
            return;
        };
        for (out, inp) in dst_row.chunks_exact_mut(4).zip(src_row.chunks_exact(4)) {
            // tiny-skia premultiplied RGBA: byte 3 is alpha (coverage). The ink's
            // premultiplied alpha equals its straight alpha (alpha is never scaled).
            // chunks_exact(4) guarantees exactly 4 bytes; direct indexing is safe.
            let ink_a = u32::from(inp[3]);
            // straight shadow alpha = ink_a * ca / 255, rounded.
            let a = ((ink_a * ca) + 127) / 255;
            // Premultiply the (constant) color by this alpha.
            let pr = ((cr * a) + 127) / 255;
            let pg = ((cg * a) + 127) / 255;
            let pb = ((cb * a) + 127) / 255;
            out[0] = pr.min(255) as u8;
            out[1] = pg.min(255) as u8;
            out[2] = pb.min(255) as u8;
            out[3] = a.min(255) as u8;
        }
    }
}

/// True when `sigma` selects a real blur. NaN, infinite and non-positive values
/// leave the pixmap unchanged.
fn blur_active(sigma: f64) -> bool {
    sigma.is_finite() && sigma > 0.0
}

/// Box radius for a box of width `w`.
fn box_radius(w: u32) -> u32 {
    (w.max(1) - 1) / 2
}

/// Total horizontal (and vertical) reach of `gaussian_blur_premul` for `sigma`.
///
/// This is the sum of the three box radii. A pixel further than this from all
/// ink stays zero after the blur. Saturates instead of overflowing.
pub(super) fn blur_reach(sigma: f64) -> u32 {
    if !blur_active(sigma) {
        return 0;
    }
    boxes_for_gauss(sigma)
        .iter()
        .fold(0u32, |acc, &w| acc.saturating_add(box_radius(w)))
}

/// Compute the three box-blur sizes that approximate a Gaussian of the given
/// `sigma` with `n == 3` passes (Kuchin's method).
///
/// Returns three odd box widths; the box radius for a width `w` is `(w - 1) / 2`.
fn boxes_for_gauss(sigma: f64) -> [u32; 3] {
    const N: f64 = 3.0;
    if sigma <= 0.0 {
        return [1, 1, 1];
    }
    let w_ideal = ((12.0 * sigma * sigma / N) + 1.0).sqrt();
    let mut wl = w_ideal.floor() as i64;
    if wl % 2 == 0 {
        wl -= 1;
    }
    if wl < 1 {
        wl = 1;
    }
    let wu = wl + 2;
    let wl_f = wl as f64;
    let m_ideal =
        (12.0 * sigma * sigma - N * wl_f * wl_f - 4.0 * N * wl_f - 3.0 * N) / (-4.0 * wl_f - 4.0);
    let m = m_ideal.round() as i64;
    let mut out = [0u32; 3];
    for (i, slot) in out.iter_mut().enumerate() {
        let w = if (i as i64) < m { wl } else { wu };
        *slot = w.max(1) as u32;
    }
    out
}

/// Apply a deterministic separable Gaussian-approximation blur (three box
/// passes) to a premultiplied RGBA8 `Pixmap`, in place.
///
/// Each pass is a horizontal box blur followed by a vertical box blur, computed
/// with running sums over each of the four premultiplied channels independently
/// (premultiplied blur is correct for source-over compositing). All indexing is
/// bounds-guarded; arithmetic uses `u32` running sums with fixed rounding, so
/// the result is byte-identical across runs.
pub(super) fn gaussian_blur_premul(pm: &mut Pixmap, sigma: f64) {
    // Non-positive or non-finite sigma → no blur (NaN-safe: the `> 0.0` test is
    // false for NaN, so we return early).
    if !blur_active(sigma) {
        return;
    }
    let width = pm.width() as usize;
    let height = pm.height() as usize;
    if width == 0 || height == 0 {
        return;
    }
    let boxes = boxes_for_gauss(sigma);
    let data = pm.data_mut();
    let expected = width * height * 4;
    if data.len() != expected {
        return; // defensive: unexpected layout → leave untouched
    }
    let mut scratch = vec![0u8; expected];
    for w in boxes {
        let radius = box_radius(w) as usize;
        if radius == 0 {
            continue; // a 1-wide box is identity
        }
        // Horizontal: data → scratch.
        box_blur_h(data, &mut scratch, width, height, radius);
        // Vertical: scratch → data.
        box_blur_v(&scratch, data, width, height, radius);
    }
}

/// Horizontal box blur of radius `radius` over premultiplied RGBA8, writing the
/// result into `dst`. Uses a running sum per channel; no panic indexing.
fn box_blur_h(src: &[u8], dst: &mut [u8], width: usize, height: usize, radius: usize) {
    let window = (2 * radius + 1) as u32;
    let last = width.saturating_sub(1);
    for y in 0..height {
        let row = y * width * 4;
        for c in 0..4 {
            // Initialize the running sum for the window CENTERED at x=0, i.e.
            // positions [-radius, radius] each clamped to [0, width-1] (edge
            // extension). Positions < 0 collapse onto column 0.
            let mut sum: u32 = 0;
            for i in 0..=(2 * radius) {
                // Map loop index i∈[0,2r] to signed offset (i - radius), clamped.
                let xx = (i.saturating_sub(radius)).min(last);
                let v = src.get(row + xx * 4 + c).copied().unwrap_or(0);
                sum += u32::from(v);
            }
            for x in 0..width {
                if let Some(o) = dst.get_mut(row + x * 4 + c) {
                    *o = ((sum + window / 2) / window).min(255) as u8;
                }
                // Slide one column right: add pixel at x+radius+1 (clamped),
                // drop the leftmost at x-radius (clamped to 0 via saturating_sub).
                let add_x = (x + radius + 1).min(last);
                let sub_x = x.saturating_sub(radius);
                let add = src.get(row + add_x * 4 + c).copied().unwrap_or(0);
                let sub = src.get(row + sub_x * 4 + c).copied().unwrap_or(0);
                sum = sum + u32::from(add) - u32::from(sub);
            }
        }
    }
}

/// Vertical box blur of radius `radius` over premultiplied RGBA8, writing the
/// result into `dst`. Uses a running sum per channel; no panic indexing.
fn box_blur_v(src: &[u8], dst: &mut [u8], width: usize, height: usize, radius: usize) {
    let window = (2 * radius + 1) as u32;
    let stride = width * 4;
    let last = height.saturating_sub(1);
    for x in 0..width {
        let col = x * 4;
        for c in 0..4 {
            // Window centered at y=0 with edge extension (see box_blur_h).
            let mut sum: u32 = 0;
            for i in 0..=(2 * radius) {
                let yy = (i.saturating_sub(radius)).min(last);
                let v = src.get(col + yy * stride + c).copied().unwrap_or(0);
                sum += u32::from(v);
            }
            for y in 0..height {
                if let Some(o) = dst.get_mut(col + y * stride + c) {
                    *o = ((sum + window / 2) / window).min(255) as u8;
                }
                let add_y = (y + radius + 1).min(last);
                let sub_y = y.saturating_sub(radius);
                let add = src.get(col + add_y * stride + c).copied().unwrap_or(0);
                let sub = src.get(col + sub_y * stride + c).copied().unwrap_or(0);
                sum = sum + u32::from(add) - u32::from(sub);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::{FillRule, Paint, PathBuilder, Rect, Transform};

    const W: u32 = 64;
    const H: u32 = 48;

    /// Page-sized ink with an anti-aliased circle and an opaque rect.
    fn ink_page(cx: f32, cy: f32, r: f32) -> Pixmap {
        let mut pm = Pixmap::new(W, H).expect("alloc");
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        paint.set_color_rgba8(200, 60, 20, 230);
        let circle = PathBuilder::from_circle(cx, cy, r).expect("circle");
        pm.fill_path(
            &circle,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
        paint.set_color_rgba8(10, 90, 160, 255);
        let rect = Rect::from_xywh(cx - 2.0, cy - 1.0, 4.0, 3.0).expect("rect");
        pm.fill_rect(rect, &paint, Transform::identity(), None);
        pm
    }

    /// A page filled with a varied semi-opaque backdrop.
    fn backdrop() -> Pixmap {
        let mut pm = Pixmap::new(W, H).expect("alloc");
        for (i, px) in pm.data_mut().chunks_exact_mut(4).enumerate() {
            let v = (i % 89) as u8;
            px[0] = v;
            px[1] = v / 2;
            px[2] = v / 4;
            px[3] = 100;
        }
        pm
    }

    fn spec(dx: f64, dy: f64, blur: f64) -> ShadowSpec {
        ShadowSpec {
            dx,
            dy,
            blur,
            color: SceneColor::srgb(0, 0, 0, 160),
        }
    }

    /// The full-page tinted and blurred shadow of one layer.
    fn full_page_shadow(ink: &Pixmap, s: &ShadowSpec) -> Pixmap {
        let mut shadow = Pixmap::new(W, H).expect("alloc");
        tint_coverage(
            &mut shadow,
            ink,
            Region::full(W, H).expect("region"),
            s.color,
        );
        gaussian_blur_premul(&mut shadow, s.blur);
        shadow
    }

    /// The old full-page shadow algorithm, kept as the byte reference.
    ///
    /// Valid only for offsets >= 0: tiny-skia mis-places a negative position.
    fn reference_shadows(canvas: &mut Pixmap, ink: &Pixmap, shadows: &[ShadowSpec]) {
        let paint = PixmapPaint::default();
        for s in shadows.iter().rev() {
            assert!(round_offset(s.dx) >= 0 && round_offset(s.dy) >= 0);
            let shadow = full_page_shadow(ink, s);
            canvas.draw_pixmap(
                round_offset(s.dx),
                round_offset(s.dy),
                shadow.as_ref(),
                &paint,
                Transform::identity(),
                None,
            );
        }
        canvas.draw_pixmap(0, 0, ink.as_ref(), &paint, Transform::identity(), None);
    }

    /// The full-page blur algorithm, kept as the byte reference.
    fn reference_blur(canvas: &mut Pixmap, mut ink: Pixmap, sigma: f64) {
        gaussian_blur_premul(&mut ink, sigma);
        canvas.draw_pixmap(
            0,
            0,
            ink.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    }

    /// The exact-placement reference for any offset.
    ///
    /// Each full-page shadow is shifted by hand into a page-sized pixmap, so
    /// pixel (x, y) moves to (x + dx, y + dy) and anything past the page is
    /// dropped. tiny-skia then draws it at (0, 0), which uses the same blend
    /// arithmetic as production.
    fn exact_reference_shadows(canvas: &mut Pixmap, ink: &Pixmap, shadows: &[ShadowSpec]) {
        let paint = PixmapPaint::default();
        for s in shadows.iter().rev() {
            let shadow = full_page_shadow(ink, s);
            let (dx, dy) = (i64::from(round_offset(s.dx)), i64::from(round_offset(s.dy)));
            let mut shifted = Pixmap::new(W, H).expect("alloc");
            for y in 0..i64::from(H) {
                for x in 0..i64::from(W) {
                    let (tx, ty) = (x + dx, y + dy);
                    if (0..i64::from(W)).contains(&tx) && (0..i64::from(H)).contains(&ty) {
                        let src = ((y * i64::from(W) + x) * 4) as usize;
                        let dst = ((ty * i64::from(W) + tx) * 4) as usize;
                        let px = shadow.data()[src..src + 4].to_vec();
                        shifted.data_mut()[dst..dst + 4].copy_from_slice(&px);
                    }
                }
            }
            canvas.draw_pixmap(0, 0, shifted.as_ref(), &paint, Transform::identity(), None);
        }
        canvas.draw_pixmap(0, 0, ink.as_ref(), &paint, Transform::identity(), None);
    }

    fn assert_shadow_exact(ink: &Pixmap, shadows: &[ShadowSpec]) {
        let mut expected = backdrop();
        exact_reference_shadows(&mut expected, ink, shadows);
        let mut cropped = backdrop();
        composite_shadows(&mut cropped, ink, shadows);
        assert_eq!(expected.data(), cropped.data(), "shadow bytes differ");
    }

    fn assert_shadow_matches(ink: &Pixmap, shadows: &[ShadowSpec]) {
        let mut full = backdrop();
        reference_shadows(&mut full, ink, shadows);
        let mut cropped = backdrop();
        composite_shadows(&mut cropped, ink, shadows);
        assert_eq!(full.data(), cropped.data(), "shadow bytes differ");
    }

    fn assert_blur_matches(ink: &Pixmap, sigma: f64) {
        let mut full = backdrop();
        reference_blur(&mut full, ink.clone(), sigma);
        let mut cropped = backdrop();
        composite_blur(&mut cropped, ink.clone(), sigma);
        assert_eq!(
            full.data(),
            cropped.data(),
            "blur bytes differ (sigma {sigma})"
        );
    }

    /// Circle centres that touch the left, right, top and bottom page edge.
    fn edge_centres() -> [(f32, f32); 4] {
        [(1.0, 24.0), (63.0, 20.0), (30.0, 1.0), (34.0, 47.0)]
    }

    #[test]
    fn interior_shadow_matches_full_page() {
        let ink = ink_page(30.0, 22.0, 6.0);
        assert_shadow_matches(&ink, &[spec(2.0, 3.0, 3.0), spec(0.0, 0.0, 1.2)]);
    }

    #[test]
    fn interior_blur_matches_full_page() {
        let ink = ink_page(30.0, 22.0, 6.0);
        for sigma in [0.0, 0.6, 2.0, 4.5] {
            assert_blur_matches(&ink, sigma);
        }
    }

    #[test]
    fn edge_shadow_matches_full_page() {
        for (cx, cy) in edge_centres() {
            let ink = ink_page(cx, cy, 5.0);
            assert_shadow_matches(&ink, &[spec(1.0, 2.0, 2.5), spec(3.0, 0.0, 5.0)]);
        }
    }

    #[test]
    fn edge_shadow_negative_offset_is_exact() {
        for (cx, cy) in edge_centres() {
            let ink = ink_page(cx, cy, 5.0);
            assert_shadow_exact(&ink, &[spec(1.0, -2.0, 2.5), spec(-3.0, 1.0, 5.0)]);
        }
    }

    #[test]
    fn edge_blur_matches_full_page() {
        for (cx, cy) in edge_centres() {
            let ink = ink_page(cx, cy, 5.0);
            for sigma in [1.0, 3.0, 7.5] {
                assert_blur_matches(&ink, sigma);
            }
        }
    }

    #[test]
    fn offset_shadow_partly_off_page_is_exact() {
        let ink = ink_page(8.0, 40.0, 7.0);
        assert_shadow_exact(
            &ink,
            &[
                spec(-12.0, 9.4, 3.0),
                spec(60.0, -45.0, 2.0),
                spec(-70.0, 0.0, 1.0),
                spec(5.5, 4.5, 40.0),
            ],
        );
        assert_shadow_matches(&ink, &[spec(12.0, 9.4, 3.0), spec(5.5, 4.5, 40.0)]);
    }

    #[test]
    fn negative_offset_at_right_and_bottom_edges_is_exact() {
        // Ink touching the right and bottom page edges, with negative offsets.
        let ink = ink_page(62.0, 46.0, 6.0);
        assert_shadow_exact(
            &ink,
            &[
                spec(-3.0, 1.0, 5.0),
                spec(2.0, -4.0, 2.0),
                spec(-5.0, -6.0, 1.5),
                spec(-64.0, 0.0, 1.0),
                spec(0.0, -48.0, 1.0),
                spec(-65.0, -49.0, 1.0),
            ],
        );
        for sp in [spec(-3.0, -2.0, 3.0), spec(-64.0, -48.0, 0.0)] {
            assert_shadow_exact(&ink, &[sp]);
        }
    }

    #[test]
    fn negative_offset_writes_no_smear_column_or_row() {
        // Ink on the right edge, rows 15..=25. The shadow moves up-left by
        // (3, 20). tiny-skia's negative-position bug would smear page column
        // 63 into canvas column W - 3 and page row 47 into canvas row H - 20.
        let ink = ink_page(63.0, 20.0, 5.0);
        let (dx, dy) = (-3i64, -20i64);
        let mut canvas = backdrop();
        composite_shadows(&mut canvas, &ink, &[spec(dx as f64, dy as f64, 0.0)]);
        let clean = backdrop();
        let px = |pm: &Pixmap, x: u32, y: u32| {
            let i = ((y * W + x) * 4) as usize;
            pm.data()[i..i + 4].to_vec()
        };
        let smear_x = (i64::from(W) + dx) as u32;
        let smear_y = (i64::from(H) + dy) as u32;
        // Rows 0..15 hold shadow only; the smear column must stay untouched.
        for y in 0..15 {
            assert_eq!(
                px(&canvas, smear_x, y),
                px(&clean, smear_x, y),
                "column at y {y}"
            );
        }
        // Row H + dy lies below the ink, so the whole row must stay untouched.
        for x in 0..W {
            assert_eq!(
                px(&canvas, x, smear_y),
                px(&clean, x, smear_y),
                "row at x {x}"
            );
        }
        // The column left of the smear does receive shadow.
        assert!((0..6).any(|y| px(&canvas, smear_x - 1, y) != px(&clean, smear_x - 1, y)));
    }

    #[test]
    fn blur_reach_wider_than_page_matches_full_page() {
        let ink = ink_page(30.0, 22.0, 4.0);
        assert_shadow_matches(&ink, &[spec(1.0, 1.0, 50.0)]);
        assert_blur_matches(&ink, 50.0);
    }

    #[test]
    fn empty_ink_matches_full_page() {
        let ink = Pixmap::new(W, H).expect("alloc");
        assert_shadow_matches(&ink, &[spec(4.0, 4.0, 3.0)]);
        assert_blur_matches(&ink, 3.0);
        let mut canvas = backdrop();
        composite_shadows(&mut canvas, &ink, &[spec(4.0, 4.0, 3.0)]);
        assert_eq!(canvas.data(), backdrop().data(), "empty ink draws nothing");
    }

    #[test]
    fn blur_reach_sums_box_radii() {
        assert_eq!(blur_reach(0.0), 0);
        assert_eq!(blur_reach(f64::NAN), 0);
        // sigma 2.0 -> boxes [3, 3, 5] -> radii 1 + 1 + 2.
        assert_eq!(boxes_for_gauss(2.0), [3, 3, 5]);
        assert_eq!(blur_reach(2.0), 4);
    }
}
