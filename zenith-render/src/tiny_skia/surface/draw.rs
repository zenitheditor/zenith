//! Pixmap draws on a [`Surface`]: path fills and strokes, device rects, and
//! images. Each has the contract of the tiny-skia call it wraps.

use tiny_skia::{
    FillRule, Mask, Paint, Path, PathBuilder, PathStroker, Pattern, Pixmap, PixmapPaint, PixmapRef,
    Point, Rect, Shader, SpreadMode, Stroke, Transform,
};

use super::scratch::{mask_from, pixmap_back, pixmap_from};
use super::window::{DeviceBox, Placement, Surface};

/// Device pixels a filled path can touch past its bounds: the anti-aliased
/// edge pixel plus tiny-skia's floor/ceil of the bounds.
pub(super) const FILL_MARGIN: f32 = 2.0;

/// Device pixels a hairline can touch past its bounds: tiny-skia outsets
/// hairline bounds by up to 2 px for caps and insets its clip by 1 px.
const HAIRLINE_MARGIN: f32 = 4.0;

/// `path` mapped by `ts` into full-page device space, with the same `f32`
/// operations tiny-skia uses.
pub(super) fn device_path(path: &Path, ts: Transform) -> Option<Path> {
    if ts.is_identity() {
        Some(path.clone())
    } else {
        path.clone().transform(ts)
    }
}

/// True when `paint` samples a shader by pixel position.
fn samples_pixels(paint: &Paint) -> bool {
    match paint.shader {
        Shader::SolidColor(_) => false,
        Shader::LinearGradient(_) | Shader::RadialGradient(_) | Shader::Pattern(_) => true,
    }
}

/// One draw of a device-space path onto a buffer covering `area`.
type DrawOn<'p> = dyn Fn(&mut Pixmap, &Path, &Paint, Option<&Mask>) + 'p;

/// Run `draw` for the device-space `device` path with the device-space
/// `paint` where `place` puts it, with `mask` (surface-sized).
fn run_placed(
    target: &mut Pixmap,
    surface: Surface,
    device: &Path,
    paint: &Paint,
    mask: Option<&Mask>,
    margin: f32,
    draw: &DrawOn<'_>,
) {
    let Some(placement) = surface.place(device.bounds(), margin, samples_pixels(paint)) else {
        return;
    };
    if let Placement::Scratch(area) = placement
        && let Some(mut scratch) = pixmap_from(target, surface, area)
    {
        let scratch_mask = match mask {
            Some(m) => match mask_from(m, surface, area) {
                Some(sm) => Some(sm),
                None => {
                    draw_at(target, surface.device_box(), device, paint, mask, draw);
                    return;
                }
            },
            None => None,
        };
        draw_at(
            &mut scratch,
            area,
            device,
            paint,
            scratch_mask.as_ref(),
            draw,
        );
        pixmap_back(target, surface, &scratch, area);
        return;
    }
    // Direct, or a scratch that could not be allocated.
    draw_at(target, surface.device_box(), device, paint, mask, draw);
}

/// Shift `device` and `paint` onto the buffer covering `area` and draw.
fn draw_at(
    pm: &mut Pixmap,
    area: DeviceBox,
    device: &Path,
    paint: &Paint,
    mask: Option<&Mask>,
    draw: &DrawOn<'_>,
) {
    if area.x0 == 0 && area.y0 == 0 {
        draw(pm, device, paint, mask);
        return;
    }
    let Some(local) = device.clone().transform(area.shift()) else {
        return;
    };
    let mut paint = paint.clone();
    paint.shader.transform(area.shift());
    draw(pm, &local, &paint, mask);
}

/// Fill `path` under the full-page transform `ts`.
///
/// Same contract as `Pixmap::fill_path`.
pub(in crate::tiny_skia) fn fill_path(
    target: &mut Pixmap,
    surface: Surface,
    path: &Path,
    paint: &Paint,
    fill_rule: FillRule,
    ts: Transform,
    mask: Option<&Mask>,
) {
    if surface.is_page() {
        target.fill_path(path, paint, fill_rule, ts, mask);
        return;
    }
    // tiny-skia's own order for a non-identity transform: map the path and
    // the shader, then fill in device space.
    let Some(device) = device_path(path, ts) else {
        return;
    };
    let mut paint = paint.clone();
    if !ts.is_identity() {
        paint.shader.transform(ts);
    }
    run_placed(
        target,
        surface,
        &device,
        &paint,
        mask,
        FILL_MARGIN,
        &|pm, p, paint, mask| pm.fill_path(p, paint, fill_rule, Transform::identity(), mask),
    );
}

/// Stroke `path` under the full-page transform `ts`.
///
/// Same contract as `Pixmap::stroke_path`. Off the page this repeats
/// tiny-skia's own steps (dash, hairline test, stroke) with the full-page
/// transform. A thick stroke becomes a filled path. A hairline goes back to
/// tiny-skia as a device-space path with a stroke width equal to its
/// coverage, which selects the same hairline branch and coverage.
pub(in crate::tiny_skia) fn stroke_path(
    target: &mut Pixmap,
    surface: Surface,
    path: &Path,
    paint: &Paint,
    stroke: &Stroke,
    ts: Transform,
    mask: Option<&Mask>,
) {
    if surface.is_page() {
        target.stroke_path(path, paint, stroke, ts, mask);
        return;
    }
    if stroke.width < 0.0 {
        return;
    }
    let res_scale = PathStroker::compute_resolution_scale(&ts);
    let dashed;
    let path = match &stroke.dash {
        Some(dash) => {
            let Some(d) = path.dash(dash, res_scale) else {
                return;
            };
            dashed = d;
            &dashed
        }
        None => path,
    };
    let Some(coverage) = hairline_coverage(paint, stroke, ts) else {
        if let Some(stroked) = path.stroke(stroke, res_scale) {
            fill_path(
                target,
                surface,
                &stroked,
                paint,
                FillRule::Winding,
                ts,
                mask,
            );
        }
        return;
    };
    let Some(device) = device_path(path, ts) else {
        return;
    };
    let mut paint = paint.clone();
    if !ts.is_identity() {
        paint.shader.transform(ts);
    }
    let hairline = Stroke {
        width: if stroke.width == 0.0 { 0.0 } else { coverage },
        dash: None,
        ..stroke.clone()
    };
    run_placed(
        target,
        surface,
        &device,
        &paint,
        mask,
        HAIRLINE_MARGIN,
        &|pm, p, paint, mask| pm.stroke_path(p, paint, &hairline, Transform::identity(), mask),
    );
}

/// tiny-skia's hairline test (`treat_as_hairline`, private in 0.11): the
/// coverage when the stroke is drawn as a hairline, else `None`.
///
/// Matches tiny-skia 0.11 operation for operation, so a pre-mapped hairline
/// picks the same branch and coverage as the full render.
fn hairline_coverage(paint: &Paint, stroke: &Stroke, mut ts: Transform) -> Option<f32> {
    fn fast_len(p: Point) -> f32 {
        let mut x = p.x.abs();
        let mut y = p.y.abs();
        if x < y {
            core::mem::swap(&mut x, &mut y);
        }
        x + y * 0.5
    }
    if stroke.width == 0.0 {
        return Some(1.0);
    }
    if !paint.anti_alias {
        return None;
    }
    ts.tx = 0.0;
    ts.ty = 0.0;
    let mut points = [
        Point::from_xy(stroke.width, 0.0),
        Point::from_xy(0.0, stroke.width),
    ];
    ts.map_points(&mut points);
    let [p0, p1] = points;
    let (len0, len1) = (fast_len(p0), fast_len(p1));
    if len0 <= 1.0 && len1 <= 1.0 {
        return Some((len0 + len1) * 0.5);
    }
    None
}

/// Fill the full-page device rect `rect` with no transform.
///
/// Same contract as `Pixmap::fill_rect` with an identity transform. A rect
/// fill never clips curves and samples a pattern at `x + 0.5 + origin`, which
/// is exact in `f32`, so it always runs on the surface.
///
/// Without anti-aliasing tiny-skia snaps the rect with `Rect::round`:
/// `trunc(floor(v) + 0.5)` for the left edge and the width. That truncates
/// toward zero, so a negative left edge lands one pixel right. Off the page
/// the rect is therefore snapped in full-page space (where the full render
/// snaps it), shifted by the integer origin, and clipped to the surface, so
/// every edge tiny-skia sees is a non-negative integer.
pub(in crate::tiny_skia) fn fill_device_rect(
    target: &mut Pixmap,
    surface: Surface,
    rect: Rect,
    paint: &Paint,
    mask: Option<&Mask>,
) {
    if surface.is_page() {
        target.fill_rect(rect, paint, Transform::identity(), mask);
        return;
    }
    let local = if paint.anti_alias {
        surface.local_rect(rect)
    } else {
        snapped_local_rect(surface, rect)
    };
    let Some(local) = local else {
        return;
    };
    let mut paint = paint.clone();
    paint.shader.transform(surface.shift());
    target.fill_rect(local, &paint, Transform::identity(), mask);
}

/// `rect` snapped as tiny-skia's non-AA rect fill snaps it, shifted onto the
/// surface and clipped to it. `None` when nothing is left.
fn snapped_local_rect(surface: Surface, rect: Rect) -> Option<Rect> {
    let snapped = rect.round()?;
    let (ox, oy) = (i64::from(surface.x), i64::from(surface.y));
    let x0 = (i64::from(snapped.left()) - ox).max(0);
    let y0 = (i64::from(snapped.top()) - oy).max(0);
    let x1 = (i64::from(snapped.right()) - ox).min(i64::from(surface.w));
    let y1 = (i64::from(snapped.bottom()) - oy).min(i64::from(surface.h));
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Rect::from_ltrb(x0 as f32, y0 as f32, x1 as f32, y1 as f32)
}

/// Draw `src` with its pixel `(0, 0)` at user-space `(0, 0)` under `ts`.
///
/// Same contract as `Pixmap::draw_pixmap(0, 0, src, paint, ts, mask)`: a
/// `Pad` pattern over the source rect, filled without anti-aliasing.
pub(in crate::tiny_skia) fn draw_pixmap(
    target: &mut Pixmap,
    surface: Surface,
    src: PixmapRef,
    paint: &PixmapPaint,
    ts: Transform,
    mask: Option<&Mask>,
) {
    if surface.is_page() {
        target.draw_pixmap(0, 0, src, paint, ts, mask);
        return;
    }
    let Some(rect) = Rect::from_xywh(0.0, 0.0, src.width() as f32, src.height() as f32) else {
        return;
    };
    let pattern_paint = Paint {
        shader: Pattern::new(
            src,
            SpreadMode::Pad,
            paint.quality,
            paint.opacity,
            Transform::identity(),
        ),
        blend_mode: paint.blend_mode,
        anti_alias: false,
        force_hq_pipeline: false,
    };
    if ts.is_identity() {
        fill_device_rect(target, surface, rect, &pattern_paint, mask);
    } else {
        // tiny-skia's `fill_rect` under a transform fills the rect path.
        let path = PathBuilder::from_rect(rect);
        fill_path(
            target,
            surface,
            &path,
            &pattern_paint,
            FillRule::Winding,
            ts,
            mask,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::{Color, GradientStop, LinearGradient};

    const PAGE: (u32, u32) = (64, 48);

    fn surface(x: u32, y: u32, w: u32, h: u32) -> Surface {
        Surface {
            x,
            y,
            w,
            h,
            page_w: PAGE.0,
            page_h: PAGE.1,
            exact: true,
        }
    }

    fn window(full: &Pixmap, s: Surface) -> Vec<u8> {
        let mut out = Vec::new();
        for row in s.y..s.y + s.h {
            let start = ((row * full.width() + s.x) * 4) as usize;
            out.extend_from_slice(&full.data()[start..start + (s.w * 4) as usize]);
        }
        out
    }

    fn solid() -> Paint<'static> {
        let mut p = Paint::default();
        p.set_color_rgba8(20, 120, 200, 230);
        p.anti_alias = true;
        p
    }

    fn gradient() -> Paint<'static> {
        let shader = LinearGradient::new(
            Point::from_xy(3.0, 5.0),
            Point::from_xy(57.0, 41.0),
            vec![
                GradientStop::new(0.0, Color::from_rgba8(250, 10, 40, 255)),
                GradientStop::new(1.0, Color::from_rgba8(10, 90, 240, 200)),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        )
        .expect("gradient");
        Paint {
            shader,
            anti_alias: true,
            ..Paint::default()
        }
    }

    /// Every window of the page equals the same window of the full render.
    fn check(draw: &dyn Fn(&mut Pixmap, Surface)) {
        let page = Surface::page(PAGE.0, PAGE.1);
        let mut full = Pixmap::new(PAGE.0, PAGE.1).expect("alloc");
        draw(&mut full, page);
        for s in [
            surface(0, 0, 20, 15),
            surface(13, 7, 30, 29),
            surface(40, 30, 24, 18),
            surface(31, 0, 1, 48),
        ] {
            let mut part = Pixmap::new(s.w, s.h).expect("alloc");
            draw(&mut part, s);
            assert_eq!(part.data(), &window(&full, s)[..], "window {s:?}");
        }
    }

    #[test]
    fn crossing_curves_match_the_full_render() {
        let path = PathBuilder::from_circle(30.3, 20.7, 16.4).expect("circle");
        let ts = Transform::from_row(1.2, 0.3, -0.2, 1.1, 2.25, 1.5);
        check(&|pm, s| fill_path(pm, s, &path, &solid(), FillRule::Winding, ts, None));
    }

    #[test]
    fn gradients_match_the_full_render() {
        let path = PathBuilder::from_circle(31.0, 22.0, 19.0).expect("circle");
        let ts = Transform::from_rotate_at(17.0, 30.0, 20.0);
        check(&|pm, s| fill_path(pm, s, &path, &gradient(), FillRule::Winding, ts, None));
    }

    #[test]
    fn strokes_and_hairlines_match_the_full_render() {
        let mut pb = PathBuilder::new();
        pb.move_to(2.2, 18.1);
        pb.quad_to(30.0, -10.0, 61.7, 40.3);
        let path = pb.finish().expect("curve");
        for width in [0.0, 0.4, 3.5] {
            let stroke = Stroke {
                width,
                ..Stroke::default()
            };
            let ts = Transform::from_scale(1.0, 1.1);
            check(&|pm, s| stroke_path(pm, s, &path, &solid(), &stroke, ts, None));
        }
    }

    /// Without scratch buffers (pages too large to render whole) a crossing
    /// curve or a gradient differs from the full render by a few
    /// anti-aliasing steps at most: no seam.
    #[test]
    fn inexact_mode_stays_within_a_few_steps() {
        let path = PathBuilder::from_circle(30.3, 20.7, 16.4).expect("circle");
        let ts = Transform::from_row(1.2, 0.3, -0.2, 1.1, 2.25, 1.5);
        let page = Surface::page(PAGE.0, PAGE.1);
        for paint in [solid(), gradient()] {
            let mut full = Pixmap::new(PAGE.0, PAGE.1).expect("alloc");
            fill_path(&mut full, page, &path, &paint, FillRule::Winding, ts, None);
            for s in [surface(13, 7, 30, 29), surface(40, 30, 24, 18)] {
                let s = Surface { exact: false, ..s };
                let mut part = Pixmap::new(s.w, s.h).expect("alloc");
                fill_path(&mut part, s, &path, &paint, FillRule::Winding, ts, None);
                let worst = part
                    .data()
                    .iter()
                    .zip(&window(&full, s))
                    .map(|(a, b)| a.abs_diff(*b))
                    .max()
                    .unwrap_or(0);
                assert!(worst <= 64, "window {s:?} differs by {worst}");
            }
        }
    }

    #[test]
    fn rects_match_the_full_render() {
        let rect = Rect::from_xywh(5.4, 3.6, 40.3, 30.9).expect("rect");
        let mut paint = solid();
        paint.anti_alias = false;
        check(&|pm, s| fill_device_rect(pm, s, rect, &paint, None));
    }
}
