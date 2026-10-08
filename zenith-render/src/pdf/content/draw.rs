//! Shared drawing primitives: alpha ExtGState application, the fill-region
//! (solid/gradient) helper, gradient interning, and small line-style/geometry
//! guards reused by the command and image emitters.

use pdf_writer::{Content, types::LineCapStyle, types::LineJoinStyle};
use zenith_scene::{Color, FillRule, LineCap, LineJoin, Paint as ScenePaint};

use crate::pdf::color;
use crate::pdf::content::resources::{ALPHA_PREFIX, PageResources, SHADING_PREFIX, name};
use crate::pdf::gradient::{PdfGradient, resolve as resolve_gradient};

/// Apply the fill-alpha ExtGState for `color` if it is non-opaque (interning the
/// alpha into `res`). Returns nothing; emits `/ga<i> gs` when needed.
pub(in crate::pdf) fn apply_alpha(content: &mut Content, res: &mut PageResources, color: &Color) {
    if color.a == 255 {
        return;
    }
    let idx = res.intern_alpha(color.a);
    content.set_parameters(name(ALPHA_PREFIX, idx).as_name());
}

/// Fill a region with a scene [`ScenePaint`] (solid or gradient), where
/// `build_path` emits the path operators for the geometry and returns whether a
/// path was produced. `bbox` is the geometry's bounding box, used to resolve a
/// gradient's native shading geometry.
///
/// - **Solid** → set the fill color and fill the path.
/// - **Gradient** → clip to the path and paint native shading.
pub(in crate::pdf::content) fn fill_region<F: Fn(&mut Content) -> bool>(
    content: &mut Content,
    res: &mut PageResources,
    paint: &ScenePaint,
    bbox: (f64, f64, f64, f64),
    fill_rule: FillRule,
    build_path: F,
) {
    let fill = |content: &mut Content, produced: bool| {
        if produced {
            apply_fill_rule(
                content,
                fill_rule,
                |content| {
                    content.fill_nonzero();
                },
                |content| {
                    content.fill_even_odd();
                },
            );
        } else {
            content.end_path();
        }
    };

    match paint {
        ScenePaint::Solid { color } => {
            content.save_state();
            apply_alpha(content, res, color);
            color::set_fill(content, color);
            let produced = build_path(content);
            fill(content, produced);
            content.restore_state();
        }
        ScenePaint::Gradient(gradient) => {
            let (x, y, w, h) = bbox;
            if let Some(g) = resolve_gradient(x, y, w, h, gradient) {
                let id = push_gradient(res, g);
                content.save_state();
                if build_path(content) {
                    apply_fill_rule(
                        content,
                        fill_rule,
                        |content| {
                            content.clip_nonzero();
                        },
                        |content| {
                            content.clip_even_odd();
                        },
                    );
                    content.end_path();
                    content.shading(name(SHADING_PREFIX, id).as_name());
                } else {
                    content.end_path();
                }
                content.restore_state();
            }
        }
    }
}

/// Push a gradient and return its resource index.
pub(in crate::pdf) fn push_gradient(res: &mut PageResources, g: PdfGradient) -> usize {
    let id = res.gradients.len();
    res.gradients.push(g);
    id
}

// ── Small helpers ──────────────────────────────────────────────────────────

#[inline]
pub(in crate::pdf::content) fn finite(v: f64) -> bool {
    v.is_finite()
}

pub(in crate::pdf::content) fn apply_fill_rule<F, G>(
    content: &mut Content,
    fill_rule: FillRule,
    nonzero: F,
    even_odd: G,
) where
    F: FnOnce(&mut Content),
    G: FnOnce(&mut Content),
{
    match fill_rule {
        FillRule::NonZero => nonzero(content),
        FillRule::EvenOdd => even_odd(content),
    }
}

pub(in crate::pdf::content) fn set_line_join(content: &mut Content, line_join: Option<LineJoin>) {
    let style = match line_join {
        Some(LineJoin::Round) => LineJoinStyle::RoundJoin,
        Some(LineJoin::Bevel) => LineJoinStyle::BevelJoin,
        Some(LineJoin::Miter) | None => LineJoinStyle::MiterJoin,
    };
    content.set_line_join(style);
}

pub(in crate::pdf::content) fn set_line_cap(content: &mut Content, line_cap: Option<LineCap>) {
    let style = match line_cap {
        Some(LineCap::Round) => LineCapStyle::RoundCap,
        Some(LineCap::Square) => LineCapStyle::ProjectingSquareCap,
        Some(LineCap::Butt) | None => LineCapStyle::ButtCap,
    };
    content.set_line_cap(style);
}

pub(in crate::pdf::content) fn set_miter_limit(
    content: &mut Content,
    miter_limit: Option<f64>,
) -> bool {
    let Some(limit) = miter_limit else {
        return true;
    };
    if !finite(limit) || limit <= 0.0 || limit > f64::from(f32::MAX) {
        return false;
    }
    content.set_miter_limit(limit as f32);
    true
}

#[inline]
pub(in crate::pdf::content) fn rect_ok(x: f64, y: f64, w: f64, h: f64) -> bool {
    finite(x) && finite(y) && finite(w) && finite(h) && w > 0.0 && h > 0.0
}

/// Match raster dash intervals after conversion to scene float precision.
pub(in crate::pdf::content) fn set_dash(
    content: &mut Content,
    dash: Option<f64>,
    gap: Option<f64>,
) {
    let Some(dash) = dash else {
        return;
    };
    if dash <= 0.0 {
        return;
    }
    let intervals = [dash as f32, gap.unwrap_or(dash).max(0.0) as f32];
    if tiny_skia::StrokeDash::new(intervals.to_vec(), 0.0).is_some() {
        content.set_dash_pattern(intervals, 0.0);
    }
}
