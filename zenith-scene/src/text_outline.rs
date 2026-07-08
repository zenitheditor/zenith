//! Materialize compiled text glyph runs into editable compound path nodes.
//!
//! This module operates on [`SceneCommand::DrawGlyphRun`] instead of authored
//! text nodes so callers reuse the same shaping, fallback, feature, and layout
//! decisions that scene compilation already made.

use std::collections::BTreeMap;

use zenith_core::{Dimension, FontProvider, PathNode, PropertyValue, Unit};
use zenith_geometry::Point2;
use zenith_layout::{
    GlyphRunOutlineRequest, LayoutError, PositionedGlyph, ZenithGlyphRun,
    glyph_outline_path_subpaths, glyph_run_outline,
};

use crate::ir::{Color, SceneCommand, SceneGlyph};

/// Convert every glyph-run command in `commands` into one compound [`PathNode`]
/// per run.
///
/// `id_prefix` is combined with a zero-based glyph-run index. Non-text commands
/// and runs whose glyphs have no outline are skipped. The returned nodes carry
/// resolved literal paint values, no source span, and ordered `subpath`
/// contours, making them suitable for later transaction-layer insertion policy.
pub fn outline_glyph_run_commands(
    id_prefix: &str,
    commands: &[SceneCommand],
    fonts: &dyn FontProvider,
) -> Result<Vec<PathNode>, LayoutError> {
    let mut nodes = Vec::new();
    let mut glyph_run_index = 0usize;

    for command in commands {
        if let SceneCommand::DrawGlyphRun { .. } = command {
            let id = format!("{id_prefix}-{glyph_run_index}");
            if let Some(node) = outline_glyph_run_command(id, command, fonts)? {
                nodes.push(node);
            }
            glyph_run_index = glyph_run_index.saturating_add(1);
        }
    }

    Ok(nodes)
}

/// Convert one [`SceneCommand::DrawGlyphRun`] into a compound [`PathNode`].
///
/// Returns `Ok(None)` for non-glyph-run commands and for glyph runs that do not
/// produce any outline contours, such as whitespace-only runs.
pub fn outline_glyph_run_command(
    id: impl Into<String>,
    command: &SceneCommand,
    fonts: &dyn FontProvider,
) -> Result<Option<PathNode>, LayoutError> {
    let SceneCommand::DrawGlyphRun {
        x,
        y,
        font_id,
        font_size,
        color,
        stroke_color,
        stroke_width,
        glyphs,
        ..
    } = command
    else {
        return Ok(None);
    };

    let origin = Point2::new(*x, *y)
        .map_err(|_| LayoutError::new("glyph run outline command requires finite origin"))?;
    let run = scene_glyph_run(font_id, *font_size, glyphs);
    let Some(outline) = glyph_run_outline(GlyphRunOutlineRequest { run: &run, origin }, fonts)?
    else {
        return Ok(None);
    };

    let mut subpaths = Vec::new();
    for glyph in &outline.glyphs {
        subpaths.extend(glyph_outline_path_subpaths(&glyph.outline)?);
    }
    if subpaths.is_empty() {
        return Ok(None);
    }

    let visible_stroke = (*stroke_color)
        .zip(*stroke_width)
        .filter(|(_, width)| width.is_finite() && *width > 0.0);
    let stroke = visible_stroke.map(|(value, _)| PropertyValue::Literal(color_literal(value)));
    let stroke_width = visible_stroke.map(|(_, value)| {
        PropertyValue::Dimension(Dimension {
            value,
            unit: Unit::Px,
        })
    });

    Ok(Some(PathNode {
        id: id.into(),
        name: None,
        role: None,
        closed: None,
        fill: Some(PropertyValue::Literal(color_literal(*color))),
        stroke,
        stroke_width,
        stroke_alignment: None,
        stroke_linejoin: None,
        stroke_miter_limit: None,
        fill_rule: None,
        opacity: None,
        visible: None,
        locked: None,
        rotate: None,
        style: None,
        anchors: Vec::new(),
        subpaths,
        source_span: None,
        unknown_props: BTreeMap::new(),
    }))
}

fn scene_glyph_run(font_id: &str, font_size: f32, glyphs: &[SceneGlyph]) -> ZenithGlyphRun {
    ZenithGlyphRun {
        font_id: font_id.to_owned(),
        font_size,
        ascent: 0.0,
        descent: 0.0,
        line_height: 0.0,
        advance_width: 0.0,
        glyphs: glyphs
            .iter()
            .map(|glyph| PositionedGlyph {
                glyph_id: glyph.glyph_id,
                x: glyph.dx,
                y: glyph.dy,
                text: glyph.text.clone(),
            })
            .collect(),
    }
}

fn color_literal(color: Color) -> String {
    if let Some([c, m, y, k]) = color.cmyk {
        return format!("cmyk({c},{m},{y},{k})");
    }
    if color.a == 255 {
        format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
    } else {
        format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            color.r, color.g, color.b, color.a
        )
    }
}

#[cfg(test)]
mod tests {
    use zenith_core::{FontStyle, default_provider};
    use zenith_layout::{RustybuzzEngine, ShapeRequest, TextDirection, TextLayoutEngine};

    use super::*;

    #[test]
    fn materializes_glyph_run_as_compound_path_node() {
        let provider = default_provider();
        let run = shape_sample_run(&provider, "A");
        let command = command_from_run(run, Color::srgb(17, 34, 51, 255), None, None);

        let node = outline_glyph_run_command("outlined-a", &command, &provider)
            .expect("outline command should not fail")
            .expect("A should have an outline");

        assert_eq!(node.id, "outlined-a");
        assert_eq!(
            node.fill,
            Some(PropertyValue::Literal("#112233".to_owned()))
        );
        assert_eq!(node.stroke, None);
        assert_eq!(node.stroke_width, None);
        assert!(node.anchors.is_empty());
        assert!(!node.subpaths.is_empty());
        assert!(node.source_span.is_none());
    }

    #[test]
    fn preserves_resolved_stroke_paint_and_width() {
        let provider = default_provider();
        let run = shape_sample_run(&provider, "B");
        let command = command_from_run(
            run,
            Color::srgb(1, 2, 3, 128),
            Some(Color::srgb(200, 201, 202, 255)),
            Some(2.5),
        );

        let node = outline_glyph_run_command("outlined-b", &command, &provider)
            .expect("outline command should not fail")
            .expect("B should have an outline");

        assert_eq!(
            node.fill,
            Some(PropertyValue::Literal("#01020380".to_owned()))
        );
        assert_eq!(
            node.stroke,
            Some(PropertyValue::Literal("#c8c9ca".to_owned()))
        );
        assert_eq!(
            node.stroke_width,
            Some(PropertyValue::Dimension(Dimension {
                value: 2.5,
                unit: Unit::Px,
            }))
        );
    }

    #[test]
    fn ignores_non_visible_stroke_data() {
        let provider = default_provider();
        let run = shape_sample_run(&provider, "C");

        let stroke_only = command_from_run(
            run.clone(),
            Color::srgb(4, 5, 6, 255),
            Some(Color::srgb(7, 8, 9, 255)),
            None,
        );
        let width_only = command_from_run(run, Color::srgb(4, 5, 6, 255), None, Some(3.0));

        let stroke_node = outline_glyph_run_command("stroke-only", &stroke_only, &provider)
            .expect("outline command should not fail")
            .expect("C should have an outline");
        let width_node = outline_glyph_run_command("width-only", &width_only, &provider)
            .expect("outline command should not fail")
            .expect("C should have an outline");

        assert_eq!(stroke_node.stroke, None);
        assert_eq!(stroke_node.stroke_width, None);
        assert_eq!(width_node.stroke, None);
        assert_eq!(width_node.stroke_width, None);
    }

    #[test]
    fn whitespace_run_returns_none() {
        let provider = default_provider();
        let run = shape_sample_run(&provider, " ");
        let command = command_from_run(run, Color::srgb(0, 0, 0, 255), None, None);

        let node = outline_glyph_run_command("space", &command, &provider)
            .expect("outline command should not fail");

        assert!(node.is_none());
    }

    #[test]
    fn preserves_cmyk_color_literals() {
        let provider = default_provider();
        let run = shape_sample_run(&provider, "C");
        let command = command_from_run(
            run,
            Color::cmyk(59.0, 85.0, 0.0, 7.0, 97, 36, 237),
            None,
            None,
        );

        let node = outline_glyph_run_command("outlined-c", &command, &provider)
            .expect("outline command should not fail")
            .expect("C should have an outline");

        assert_eq!(
            node.fill,
            Some(PropertyValue::Literal("cmyk(59,85,0,7)".to_owned()))
        );
    }

    fn shape_sample_run(provider: &dyn FontProvider, text: &str) -> ZenithGlyphRun {
        let engine = RustybuzzEngine::new();
        let families = [String::from("Noto Sans")];
        engine
            .shape(
                &ShapeRequest {
                    text,
                    families: &families,
                    weight: 400,
                    style: FontStyle::Normal,
                    font_size: 32.0,
                    direction: TextDirection::Ltr,
                    features: &[],
                },
                provider,
            )
            .expect("sample text should shape")
    }

    fn command_from_run(
        run: ZenithGlyphRun,
        color: Color,
        stroke_color: Option<Color>,
        stroke_width: Option<f64>,
    ) -> SceneCommand {
        SceneCommand::DrawGlyphRun {
            x: 10.0,
            y: 40.0,
            font_id: run.font_id,
            font_size: run.font_size,
            color,
            stroke_color,
            stroke_width,
            link: None,
            selectable: true,
            glyphs: run
                .glyphs
                .into_iter()
                .map(|glyph| SceneGlyph {
                    glyph_id: glyph.glyph_id,
                    dx: glyph.x,
                    dy: glyph.y,
                    text: glyph.text,
                })
                .collect(),
        }
    }
}
