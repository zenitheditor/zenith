//! Every command family checks numeric operands before PDF planning or resource lookup.

use super::{transform::TransformStack, values::Check};
use crate::RenderError;
use zenith_scene::ir::path_segments_bbox;
use zenith_scene::{FilterSpec, ImageClip, Scene, SceneCommand, StrokeAlign};

pub(in crate::pdf) fn check_scene(scene: &Scene, page: usize) -> Result<(), RenderError> {
    let initial = Check { page, command: 0 };
    initial.coordinate("scene.width", scene.width)?;
    initial.coordinate("scene.height", scene.height)?;
    let mut transforms = TransformStack::new(scene.height);
    if let Some(trim) = scene.trim {
        initial.rect(&transforms, "trim", (trim.x, trim.y, trim.w, trim.h))?;
    }
    for (command, value) in scene.commands.iter().enumerate() {
        let check = Check { page, command };
        let outside = if let SceneCommand::StrokePolyline {
            closed: true,
            align: StrokeAlign::Outside,
            points,
            ..
        } = value
        {
            points.len() >= 4
        } else if let SceneCommand::StrokePath {
            closed: true,
            align: StrokeAlign::Outside,
            segments,
            ..
        } = value
        {
            segments.len() >= 2
        } else {
            false
        };
        if outside {
            let margin = scene.width.max(scene.height).max(1.0);
            check.coordinate("outside_clip.end.x", scene.width + margin)?;
            check.coordinate("outside_clip.end.y", scene.height + margin)?;
        }
        check_command(check, &mut transforms, value)?;
    }
    Ok(())
}
pub(in crate::pdf) fn check_capture_commands(
    commands: &[SceneCommand],
    page: usize,
    start: usize,
    scale: f64,
) -> Result<(), RenderError> {
    let mut transforms = TransformStack::capture(scale);
    for (index, command) in commands.iter().enumerate() {
        check_command(
            Check {
                page,
                command: start + index,
            },
            &mut transforms,
            command,
        )?;
    }
    Ok(())
}

fn check_command(
    check: Check,
    transforms: &mut TransformStack,
    command: &SceneCommand,
) -> Result<(), RenderError> {
    match command {
        SceneCommand::FillRect { x, y, w, h, paint } => {
            check.rect(transforms, "rect", (*x, *y, *w, *h))?;
            check.paint(transforms, paint, Some((*x, *y, *w, *h)))?;
        }
        SceneCommand::FillRoundedRect {
            x,
            y,
            w,
            h,
            radius,
            radii,
            paint,
        } => {
            check.rect(transforms, "rect", (*x, *y, *w, *h))?;
            check.finite("radius", *radius)?;
            if let Some(radii) = radii {
                for (i, r) in radii.iter().enumerate() {
                    check.finite(&format!("radii[{i}]"), *r)?;
                }
            }
            check.paint(transforms, paint, Some((*x, *y, *w, *h)))?;
        }
        SceneCommand::FillEllipse {
            x,
            y,
            w,
            h,
            rx,
            ry,
            paint,
        } => {
            ellipse(check, transforms, (*x, *y, *w, *h), *rx, *ry)?;
            check.paint(transforms, paint, Some((*x, *y, *w, *h)))?;
        }
        SceneCommand::StrokeRect {
            x,
            y,
            w,
            h,
            color,
            stroke_width,
            stroke_dash,
            stroke_gap,
            stroke_linecap: _,
        } => {
            check.rect(transforms, "rect", (*x, *y, *w, *h))?;
            stroke(
                check,
                transforms,
                color,
                *stroke_width,
                *stroke_dash,
                *stroke_gap,
            )?;
            check.point(
                transforms,
                "stroke.inset",
                *x + *stroke_width / 2.0,
                *y + *stroke_width / 2.0,
            )?;
        }
        SceneCommand::StrokeRoundedRect {
            x,
            y,
            w,
            h,
            radius,
            radii,
            color,
            stroke_width,
            stroke_dash,
            stroke_gap,
            stroke_linecap: _,
        } => {
            check.rect(transforms, "rect", (*x, *y, *w, *h))?;
            check.finite("radius", *radius)?;
            if let Some(radii) = radii {
                for (i, r) in radii.iter().enumerate() {
                    check.finite(&format!("radii[{i}]"), *r)?;
                }
            }
            stroke(
                check,
                transforms,
                color,
                *stroke_width,
                *stroke_dash,
                *stroke_gap,
            )?;
            check.point(
                transforms,
                "stroke.inset",
                *x + *stroke_width / 2.0,
                *y + *stroke_width / 2.0,
            )?;
        }
        SceneCommand::StrokeEllipse {
            x,
            y,
            w,
            h,
            rx,
            ry,
            color,
            stroke_width,
            stroke_dash,
            stroke_gap,
            stroke_linecap: _,
        } => {
            ellipse(check, transforms, (*x, *y, *w, *h), *rx, *ry)?;
            stroke(
                check,
                transforms,
                color,
                *stroke_width,
                *stroke_dash,
                *stroke_gap,
            )?;
        }
        SceneCommand::StrokeLine {
            x1,
            y1,
            x2,
            y2,
            color,
            stroke_width,
            stroke_dash,
            stroke_gap,
            stroke_linecap: _,
        } => {
            check.point(transforms, "line.start", *x1, *y1)?;
            check.point(transforms, "line.end", *x2, *y2)?;
            stroke(
                check,
                transforms,
                color,
                *stroke_width,
                *stroke_dash,
                *stroke_gap,
            )?;
        }
        SceneCommand::FillPolygon {
            points,
            paint,
            fill_rule: _,
        } => {
            points_check(check, transforms, points)?;
            let bbox = (points.len() >= 2).then(|| crate::pdf::geometry::poly_bbox(points));
            check.paint(transforms, paint, bbox)?;
        }
        SceneCommand::StrokePolyline {
            points,
            color,
            stroke_width,
            closed,
            align,
            clip_fill_rule: _,
        } => {
            points_check(check, transforms, points)?;
            check.color("color", color)?;
            check.stroke(
                transforms,
                *stroke_width,
                *closed && aligned(*align) && points.len() >= 4,
            )?;
        }
        SceneCommand::FillPath {
            segments,
            paint,
            fill_rule: _,
        } => {
            check.path(transforms, segments)?;
            check.paint(transforms, paint, path_segments_bbox(segments))?;
        }
        SceneCommand::StrokePath {
            segments,
            color,
            stroke_width,
            closed,
            align,
            clip_fill_rule: _,
            stroke_linejoin: _,
            stroke_linecap: _,
            stroke_miter_limit,
        } => {
            check.path(transforms, segments)?;
            check.color("color", color)?;
            check.stroke(
                transforms,
                *stroke_width,
                *closed && aligned(*align) && segments.len() >= 2,
            )?;
            check.option("stroke_miter_limit", *stroke_miter_limit)?;
        }
        SceneCommand::DrawImage {
            x,
            y,
            w,
            h,
            asset_id: _,
            fit: _,
            pos_x,
            pos_y,
            opacity,
            clip_shape,
            src_rect: _,
            svg_style: _,
        } => {
            check.rect(transforms, "image", (*x, *y, *w, *h))?;
            check.coordinate("image.pos_x", *pos_x)?;
            check.coordinate("image.pos_y", *pos_y)?;
            check.finite("image.opacity", *opacity)?;
            if let Some(shape) = clip_shape {
                match shape {
                    ImageClip::Ellipse => {}
                    ImageClip::RoundedRect { radius } => {
                        check.finite("image.clip.radius", *radius)?
                    }
                }
            }
        }
        SceneCommand::DrawSvgAsset {
            x,
            y,
            w,
            h,
            asset: _,
        } => check.rect(transforms, "svg_asset", (*x, *y, *w, *h))?,
        SceneCommand::DrawGlyphRun {
            x,
            y,
            font_id: _,
            font_size,
            color,
            stroke_color,
            stroke_width,
            link: _,
            selectable: _,
            source_node_id: _,
            glyphs,
        } => {
            check.point(transforms, "glyph_origin", *x, *y)?;
            check.coordinate("font_size", f64::from(*font_size))?;
            check.color("color", color)?;
            if let Some(color) = stroke_color {
                check.color("stroke_color", color)?;
                if let Some(width) = stroke_width {
                    check.stroke(transforms, *width, false)?;
                }
            }
            for (i, glyph) in glyphs.iter().enumerate() {
                check.coordinate(&format!("glyphs[{i}].dx"), f64::from(glyph.dx))?;
                check.coordinate(&format!("glyphs[{i}].dy"), f64::from(glyph.dy))?;
                check.point(
                    transforms,
                    &format!("glyphs[{i}].position"),
                    *x + f64::from(glyph.dx),
                    *y + f64::from(glyph.dy),
                )?;
                check.coordinate("glyph.position.x.f32", f64::from(*x as f32 + glyph.dx))?;
                check.coordinate("glyph.position.y.f32", f64::from(*y as f32 + glyph.dy))?;
            }
        }
        SceneCommand::PushClip { x, y, w, h } => {
            check.rect(transforms, "clip", (*x, *y, *w, *h))?
        }
        SceneCommand::PushClipRoundedRect { x, y, w, h, radius } => {
            check.rect(transforms, "clip", (*x, *y, *w, *h))?;
            check.finite("clip.radius", *radius)?;
        }
        SceneCommand::PushLayer {
            opacity,
            blend_mode: _,
        } => check.finite("layer.opacity", *opacity)?,
        SceneCommand::PushTransform { angle_deg, cx, cy } => {
            transforms.rotation(check, *angle_deg, *cx, *cy)?
        }
        SceneCommand::PushScaleTranslate { sx, sy, tx, ty } => {
            for (field, value) in [("sx", *sx), ("sy", *sy), ("tx", *tx), ("ty", *ty)] {
                check.coordinate(field, value)?;
            }
            transforms.push(
                check,
                tiny_skia::Transform::from_row(
                    *sx as f32, 0.0, 0.0, *sy as f32, *tx as f32, *ty as f32,
                ),
            )?;
        }
        SceneCommand::PushTransformMatrix { a, b, c, d, e, f } => {
            for (field, value) in [
                ("a", *a),
                ("b", *b),
                ("c", *c),
                ("d", *d),
                ("e", *e),
                ("f", *f),
            ] {
                check.coordinate(field, value)?;
            }
            transforms.push(
                check,
                tiny_skia::Transform::from_row(
                    *a as f32, *b as f32, *c as f32, *d as f32, *e as f32, *f as f32,
                ),
            )?;
        }
        SceneCommand::PopTransform => transforms.pop(),
        SceneCommand::BeginShadow { shadows } => {
            for (i, shadow) in shadows.iter().enumerate() {
                for (field, value) in [("dx", shadow.dx), ("dy", shadow.dy), ("blur", shadow.blur)]
                {
                    check.coordinate(&format!("shadows[{i}].{field}"), value)?;
                }
                check.color("shadow.color", &shadow.color)?;
            }
        }
        SceneCommand::BeginBlur { radius } => check.coordinate("blur.radius", *radius)?,
        SceneCommand::BeginFilter { filters } => {
            for (i, filter) in filters.iter().enumerate() {
                check_filter(check, i, filter)?;
            }
        }
        SceneCommand::BeginMask { mask } => {
            check.rect(transforms, "mask", (mask.x, mask.y, mask.w, mask.h))?;
            if matches!(mask.shape, zenith_scene::MaskShape::RoundedRect) {
                check.finite("mask.radius", mask.radius)?;
            }
            check.coordinate("mask.feather", mask.feather)?;
        }
        SceneCommand::PopClip
        | SceneCommand::PopLayer
        | SceneCommand::EndShadow
        | SceneCommand::EndBlur
        | SceneCommand::EndFilter
        | SceneCommand::EndMask => {}
    }
    Ok(())
}
fn aligned(align: StrokeAlign) -> bool {
    match align {
        StrokeAlign::Center => false,
        StrokeAlign::Inside | StrokeAlign::Outside => true,
    }
}
fn points_check(
    check: Check,
    transforms: &TransformStack,
    points: &[f64],
) -> Result<(), RenderError> {
    for (i, value) in points.iter().enumerate() {
        check.coordinate(&format!("points[{i}]"), *value)?;
    }
    if !points.len().is_multiple_of(2) {
        return Err(RenderError::new(format!(
            "PDF page {} command {} polygon coordinate count error: points.len()={}. Supply complete coordinate pairs",
            check.page,
            check.command,
            points.len()
        )));
    }
    for (i, &[x, y]) in points.as_chunks::<2>().0.iter().enumerate() {
        check.point(transforms, &format!("points[{i}]"), x, y)?;
    }
    Ok(())
}
fn stroke(
    check: Check,
    transforms: &TransformStack,
    color: &zenith_scene::Color,
    width: f64,
    dash: Option<f64>,
    gap: Option<f64>,
) -> Result<(), RenderError> {
    check.color("color", color)?;
    check.stroke(transforms, width, false)?;
    check.option("stroke_dash", dash)?;
    check.option("stroke_gap", gap)
}
fn ellipse(
    check: Check,
    transforms: &TransformStack,
    rect: (f64, f64, f64, f64),
    rx: Option<f64>,
    ry: Option<f64>,
) -> Result<(), RenderError> {
    check.rect(transforms, "ellipse", rect)?;
    let (x, y, w, h) = rect;
    let rx = rx.unwrap_or(w / 2.0);
    let ry = ry.unwrap_or(h / 2.0);
    check.coordinate("ellipse.rx", rx)?;
    check.coordinate("ellipse.ry", ry)?;
    check.point(
        transforms,
        "ellipse.extent.start",
        x + w / 2.0 - rx,
        y + h / 2.0 - ry,
    )?;
    check.point(
        transforms,
        "ellipse.extent.end",
        x + w / 2.0 + rx,
        y + h / 2.0 + ry,
    )
}
fn check_filter(check: Check, index: usize, filter: &FilterSpec) -> Result<(), RenderError> {
    let field = format!("filters[{index}]");
    match filter {
        FilterSpec::Grayscale(value)
        | FilterSpec::Invert(value)
        | FilterSpec::Sepia(value)
        | FilterSpec::Saturate(value)
        | FilterSpec::Brightness(value)
        | FilterSpec::Contrast(value)
        | FilterSpec::HueRotate(value) => check.finite(&field, *value),
        FilterSpec::Duotone {
            amount,
            shadow,
            highlight,
        } => {
            check.finite(&format!("{field}.amount"), *amount)?;
            check.color(&format!("{field}.shadow"), shadow)?;
            check.color(&format!("{field}.highlight"), highlight)
        }
        FilterSpec::Noise {
            amount,
            seed: _,
            scale,
        } => {
            check.finite(&format!("{field}.amount"), *amount)?;
            check.finite(&format!("{field}.scale"), *scale)
        }
    }
}
