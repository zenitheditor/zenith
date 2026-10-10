//! Gradient shader construction for the tiny-skia backend.
//!
//! Linear gradients use a CSS-style angle through the box center.
//! Radial gradients use a center point (as box fractions) and a radius
//! (as a fraction of `hypot(w, h) / 2`), painted via tiny-skia's
//! `RadialGradient` (start == end == center, standard radial).

use tiny_skia::{
    Color as TsColor, GradientStop as TsGradientStop, LinearGradient, Point, RadialGradient,
    Shader, SpreadMode, Transform,
};
use zenith_geometry::math;
use zenith_scene::GradientPaint;

/// Build a tiny-skia gradient [`Shader`] for a fill box.
///
/// When `gradient.radial` is false (the default), this builds a linear gradient:
/// the gradient line runs through the box center at `gradient.angle_deg`
/// (clockwise from +x in screen coordinates, so `90°` = top-to-bottom) with
/// CSS gradient-line length `|w·cosθ| + |h·sinθ|`.
///
/// When `gradient.radial` is true, this builds a radial gradient:
/// the center defaults to `(0.5, 0.5)` (box center) and the radius defaults
/// to `hypot(w, h) / 2` (corner-reaching radius), scaled by `radius_frac`.
///
/// Returns `None` when tiny-skia rejects the stops (e.g. fewer than two).
pub(super) fn gradient_shader(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    gradient: &GradientPaint,
) -> Option<Shader<'static>> {
    let stops: Vec<TsGradientStop> = gradient
        .stops
        .iter()
        .map(|s| {
            TsGradientStop::new(
                s.offset as f32,
                TsColor::from_rgba8(s.color.r, s.color.g, s.color.b, s.color.a),
            )
        })
        .collect();

    if gradient.radial {
        let cx = x + w * gradient.center_x.unwrap_or(0.5);
        let cy = y + h * gradient.center_y.unwrap_or(0.5);
        let default_radius = math::hypot(w / 2.0, h / 2.0);
        let radius = (gradient.radius_frac.unwrap_or(1.0) * default_radius) as f32;
        let center = Point::from_xy(cx as f32, cy as f32);
        RadialGradient::new(
            center,
            center,
            radius,
            stops,
            SpreadMode::Pad,
            Transform::identity(),
        )
    } else {
        let theta = gradient.angle_deg.to_radians();
        let (dir_x, dir_y) = (math::cos(theta), math::sin(theta));
        let center = (x + w / 2.0, y + h / 2.0);
        // CSS gradient-line length, then half-extent on each side of center.
        let line_len = (w * dir_x).abs() + (h * dir_y).abs();
        let half = line_len / 2.0;
        let start = Point::from_xy(
            (center.0 - dir_x * half) as f32,
            (center.1 - dir_y * half) as f32,
        );
        let end = Point::from_xy(
            (center.0 + dir_x * half) as f32,
            (center.1 + dir_y * half) as f32,
        );
        LinearGradient::new(start, end, stops, SpreadMode::Pad, Transform::identity())
    }
}
