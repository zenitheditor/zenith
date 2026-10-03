//! Box paint shared by `rect` and `frame`: the background fill and the border
//! stroke of a (rounded) rectangle.

use std::collections::BTreeMap;

use zenith_core::{Diagnostic, PropertyValue, ResolvedToken};

use crate::ir::{Color, LineCap, Paint, SceneCommand};

use super::super::paint::{
    apply_gradient_opacity, resolve_property_color, resolve_property_gradient,
};

/// A box in page pixels with its uniform and optional per-corner radii.
#[derive(Clone, Copy)]
pub(in crate::compile) struct BoxGeom {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub radius: f64,
    /// Per-corner radii `[tl, tr, br, bl]`. `None` uses `radius` for all.
    pub radii: Option<[f64; 4]>,
}

impl BoxGeom {
    /// True when the uniform radius or any corner radius is above 0.
    pub(in crate::compile) fn is_rounded(&self) -> bool {
        self.radius > 0.0 || self.radii.is_some_and(|a| a.iter().any(|&v| v > 0.0))
    }
}

/// Token and opacity environment for one node's box paint.
#[derive(Clone, Copy)]
pub(in crate::compile) struct BoxPaintEnv<'a> {
    pub resolved: &'a BTreeMap<String, ResolvedToken>,
    pub node_id: &'a str,
    /// Opacity multiplied into every paint alpha.
    pub color_op: f64,
}

/// Border stroke attributes, already resolved to pixels.
#[derive(Clone, Copy)]
pub(in crate::compile) struct BoxStroke<'a> {
    pub color: &'a PropertyValue,
    pub width: f64,
    pub dash: Option<f64>,
    pub gap: Option<f64>,
    pub linecap: Option<LineCap>,
    /// `inside`, `outside`, or `center` (the default for any other value).
    pub alignment: Option<&'a str>,
}

/// Push the box fill: a gradient token or a color token. Nothing is pushed
/// when `fill` resolves to neither.
pub(in crate::compile) fn push_box_fill(
    draws: &mut Vec<SceneCommand>,
    geom: BoxGeom,
    fill: &PropertyValue,
    env: BoxPaintEnv<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let paint = if let Some(mut gradient) =
        resolve_property_gradient(fill, env.resolved, env.node_id)
    {
        apply_gradient_opacity(&mut gradient, env.color_op, 1.0);
        Paint::Gradient(gradient)
    } else if let Some(color) = resolve_property_color(fill, env.resolved, diagnostics, env.node_id)
    {
        Paint::solid(scaled_alpha(color, env.color_op))
    } else {
        return;
    };
    let BoxGeom {
        x,
        y,
        w,
        h,
        radius,
        radii,
    } = geom;
    if geom.is_rounded() {
        draws.push(SceneCommand::FillRoundedRect {
            x,
            y,
            w,
            h,
            radius,
            radii,
            paint,
        });
    } else {
        draws.push(SceneCommand::FillRect { x, y, w, h, paint });
    }
}

/// Push the box border stroke, offset for `inside` / `outside` alignment.
/// Nothing is pushed when the color does not resolve or an inside stroke
/// leaves no area.
pub(in crate::compile) fn push_box_stroke(
    draws: &mut Vec<SceneCommand>,
    geom: BoxGeom,
    stroke: BoxStroke<'_>,
    env: BoxPaintEnv<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(color) = resolve_property_color(stroke.color, env.resolved, diagnostics, env.node_id)
    else {
        return;
    };
    let color = scaled_alpha(color, env.color_op);
    let stroke_width = stroke.width;
    let BoxGeom {
        x,
        y,
        w,
        h,
        radius,
        radii,
    } = geom;

    // Alignment moves the stroke path by half the stroke width. A corner with
    // radius 0 stays sharp.
    let half = stroke_width / 2.0;
    let adjust_inside = |v: f64| if v > 0.0 { (v - half).max(0.0) } else { 0.0 };
    let adjust_outside = |v: f64| if v > 0.0 { v + half } else { 0.0 };
    let shifted = match stroke.alignment {
        Some("inside") => BoxGeom {
            x: x + half,
            y: y + half,
            w: w - stroke_width,
            h: h - stroke_width,
            radius: adjust_inside(radius),
            radii: radii.map(|r| r.map(adjust_inside)),
        },
        Some("outside") => BoxGeom {
            x: x - half,
            y: y - half,
            w: w + stroke_width,
            h: h + stroke_width,
            radius: adjust_outside(radius),
            radii: radii.map(|r| r.map(adjust_outside)),
        },
        // "center" (default) and any unrecognized value.
        _ => geom,
    };

    // An inside stroke can shrink the box to nothing: skip it.
    if shifted.w <= 0.0 || shifted.h <= 0.0 {
        return;
    }
    if shifted.is_rounded() {
        draws.push(SceneCommand::StrokeRoundedRect {
            x: shifted.x,
            y: shifted.y,
            w: shifted.w,
            h: shifted.h,
            radius: shifted.radius,
            radii: shifted.radii,
            color,
            stroke_width,
            stroke_dash: stroke.dash,
            stroke_gap: stroke.gap,
            stroke_linecap: stroke.linecap,
        });
    } else {
        draws.push(SceneCommand::StrokeRect {
            x: shifted.x,
            y: shifted.y,
            w: shifted.w,
            h: shifted.h,
            color,
            stroke_width,
            stroke_dash: stroke.dash,
            stroke_gap: stroke.gap,
            stroke_linecap: stroke.linecap,
        });
    }
}

/// Multiply a color's alpha by `op`, rounding to the nearest step.
fn scaled_alpha(mut color: Color, op: f64) -> Color {
    color.a = (color.a as f64 * op).round() as u8;
    color
}
