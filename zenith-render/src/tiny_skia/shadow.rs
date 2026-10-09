//! Drop-shadow / outer-glow compositing for shadowed leaf nodes.
//!
//! The node's ink is captured into an offscreen `Pixmap` (premultiplied RGBA8).
//! At EndShadow, each shadow layer is derived from the ink's coverage (alpha),
//! tinted with the layer color, blurred, and composited behind the crisp ink.
//!
//! The blur kernel lives in `blur`.

use tiny_skia::{Pixmap, PixmapPaint};
use zenith_scene::ShadowSpec;

use super::blur::{BlurScratch, gaussian_blur_premul};
use super::crop::{Region, blur_crop, copy_region, draw_at, draw_region, ink_bbox};
use super::pool::Dirty;

/// Local alias for the scene `Color` carried inside a `ShadowSpec`, so this
/// helper does not need to import the scene `Color` name (which would collide
/// with tiny-skia's `Color`). Resolved at call sites via `spec.color`.
type SceneColor = zenith_scene::Color;

/// Paint all shadow layers of one capture onto `canvas`, then the crisp ink.
///
/// Layers are painted in REVERSE declared order so the first-declared layer ends
/// up on top of later layers (all behind the ink). `canvas` and `ink` are
/// page-sized. Each layer works only on the ink's bounding box grown by the
/// layer's blur reach (see `crop`). Empty ink draws nothing. Returns where
/// `ink` can hold non-zero bytes (its bounding box), for reuse.
pub(super) fn composite_shadows(
    canvas: &mut Pixmap,
    ink: &Pixmap,
    shadows: &[ShadowSpec],
) -> Dirty {
    let Some(bbox) = ink_bbox(ink) else {
        return Dirty::Clean;
    };
    let (width, height) = (ink.width(), ink.height());
    let paint = PixmapPaint::default(); // source-over, opacity 1.0
    let mut scratch = BlurScratch::default();
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
        gaussian_blur_premul(&mut shadow, spec.blur, &mut scratch);

        let (ox, oy) = (round_offset(spec.dx), round_offset(spec.dy));

        // Composite at the crop origin plus the rounded integer offset.
        // `draw_at` places pixels exactly, also for a negative offset.
        let dx = i64::from(crop.x) + i64::from(ox);
        let dy = i64::from(crop.y) + i64::from(oy);
        draw_at(canvas, &shadow, dx, dy, &paint);
    }

    // Crisp ink on top of every shadow.
    draw_region(canvas, ink, bbox, &paint);
    Dirty::Region(bbox)
}

/// Blur page-sized `ink` by `sigma` and composite it onto `target`.
///
/// The blur runs on the ink's bounding box grown by the blur reach. Empty ink
/// draws nothing. On allocation failure the full page is blurred in place.
/// Returns where `ink` can hold non-zero bytes afterwards, for reuse.
pub(super) fn composite_blur(target: &mut Pixmap, ink: &mut Pixmap, sigma: f64) -> Dirty {
    let Some(bbox) = ink_bbox(ink) else {
        return Dirty::Clean;
    };
    let paint = PixmapPaint::default();
    let mut scratch = BlurScratch::default();
    let crop = blur_crop(bbox, sigma, ink.width(), ink.height());
    if let Some(crop) = crop
        && let Some(mut part) = copy_region(ink, crop)
    {
        gaussian_blur_premul(&mut part, sigma, &mut scratch);
        draw_at(target, &part, i64::from(crop.x), i64::from(crop.y), &paint);
        return Dirty::Region(bbox);
    }
    gaussian_blur_premul(ink, sigma, &mut scratch);
    draw_at(target, ink, 0, 0, &paint);
    Dirty::All
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
/// `color.rgb`; written PREMULTIPLIED. The output depends on the ink alpha
/// alone, so the 256 possible pixels come from a table built once per call.
/// `shadow` has the size of `region`. Rows outside `ink` stay zero.
fn tint_coverage(shadow: &mut Pixmap, ink: &Pixmap, region: Region, color: SceneColor) {
    let table = tint_table(color);
    let ink_stride = ink.width() as usize * 4;
    let row_len = region.w as usize * 4;
    let x_off = region.x as usize * 4;
    let src_data = ink.data();
    for (dy, dst_row) in shadow.data_mut().chunks_exact_mut(row_len).enumerate() {
        let start = (region.y as usize + dy) * ink_stride + x_off;
        let Some(src_row) = src_data.get(start..start + row_len) else {
            return;
        };
        for (out, inp) in dst_row
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(src_row.as_chunks::<4>().0.iter())
        {
            // tiny-skia premultiplied RGBA: byte 3 is alpha (coverage). The ink's
            // premultiplied alpha equals its straight alpha (alpha is never scaled).
            if let Some(px) = table.get(usize::from(inp[3])) {
                *out = *px;
            }
        }
    }
}

/// The premultiplied shadow pixel for each ink alpha: straight alpha
/// `a = (ink_a * color.a + 127) / 255`, then each color channel
/// `(c * a + 127) / 255`.
fn tint_table(color: SceneColor) -> [[u8; 4]; 256] {
    let ca = u32::from(color.a);
    let (cr, cg, cb) = (u32::from(color.r), u32::from(color.g), u32::from(color.b));
    let mut table = [[0u8; 4]; 256];
    for (ink_a, px) in (0u32..).zip(table.iter_mut()) {
        let a = ((ink_a * ca) + 127) / 255;
        let premul = |c: u32| (((c * a) + 127) / 255).min(255) as u8;
        *px = [premul(cr), premul(cg), premul(cb), a.min(255) as u8];
    }
    table
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
        for (i, px) in pm.data_mut().as_chunks_mut::<4>().0.iter_mut().enumerate() {
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
        gaussian_blur_premul(&mut shadow, s.blur, &mut BlurScratch::default());
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
        gaussian_blur_premul(&mut ink, sigma, &mut BlurScratch::default());
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
        let mut ink = ink.clone();
        composite_blur(&mut cropped, &mut ink, sigma);
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

    /// The former per-pixel tint, kept as the byte reference.
    fn reference_tint(ink_a: u8, color: SceneColor) -> [u8; 4] {
        let ca = u32::from(color.a);
        let a = ((u32::from(ink_a) * ca) + 127) / 255;
        let premul = |c: u8| (((u32::from(c) * a) + 127) / 255).min(255) as u8;
        [
            premul(color.r),
            premul(color.g),
            premul(color.b),
            a.min(255) as u8,
        ]
    }

    #[test]
    fn tint_table_matches_the_per_pixel_formula() {
        for (r, g, b, a) in [
            (0, 0, 0, 160),
            (255, 255, 255, 255),
            (8, 32, 63, 43),
            (37, 194, 228, 102),
            (1, 2, 3, 0),
        ] {
            let color = SceneColor::srgb(r, g, b, a);
            let table = tint_table(color);
            for ink_a in 0..=255u8 {
                assert_eq!(
                    table[usize::from(ink_a)],
                    reference_tint(ink_a, color),
                    "{color:?} at {ink_a}"
                );
            }
        }
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
}
