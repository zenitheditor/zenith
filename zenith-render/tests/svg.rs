mod svg_support;

#[path = "common/no_assets.rs"]
mod no_assets;
use no_assets::no_assets;
#[path = "common/red.rs"]
mod red;
use red::red;
use svg_support::{assert_pixels_close, data_url_bytes, embedded_png, rasterize};
use zenith_core::default_provider;
use zenith_render::render_image;
use zenith_render::{SvgRasterizationReason, render_svg, render_svg_with};
use zenith_scene::ir::{FillRule, FilterSpec, LineCap, MaskShape, MaskSpec, PathSegment};
use zenith_scene::{
    BlendMode, Color, GradientPaint, GradientStop, Paint, Scene, SceneCommand, ShadowSpec,
    StrokeAlign,
};

fn rect(x: f64, y: f64, w: f64, h: f64) -> SceneCommand {
    SceneCommand::FillRect {
        x,
        y,
        w,
        h,
        paint: Paint::solid(red()),
    }
}

fn check_native(scene: &Scene, mean_limit: f64) -> String {
    let fonts = default_provider();
    let output = render_svg_with(scene, &fonts, &no_assets()).expect("native SVG render");
    assert!(output.rasterized_regions.is_empty());
    let image = rasterize(&output.bytes);
    let reference = render_image(scene, &fonts, &no_assets()).expect("PNG reference");
    if reference
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .any(|pixel| pixel[3] != 0)
    {
        assert!(
            image
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] != 0),
            "native SVG contains ink"
        );
    }
    assert_pixels_close(&image, &reference, mean_limit);
    assert_eq!(
        output.bytes,
        render_svg(scene, &fonts, &no_assets()).expect("repeat render")
    );
    let svg = String::from_utf8(output.bytes).expect("UTF-8 SVG");
    assert!(!svg.contains("<image"), "native geometry stays vector");
    svg
}

#[test]
fn native_geometry_preserves_dimensions_and_pixels() {
    let mut scene = Scene::new(128.0, 96.0);
    scene.commands = vec![
        rect(2.0, 2.0, 20.0, 16.0),
        SceneCommand::FillRoundedRect {
            x: 30.0,
            y: 2.0,
            w: 24.0,
            h: 20.0,
            radius: 6.0,
            radii: Some([2.0, 4.0, 6.0, 8.0]),
            paint: Paint::solid(red()),
        },
        SceneCommand::FillEllipse {
            x: 60.0,
            y: 2.0,
            w: 24.0,
            h: 20.0,
            rx: Some(8.0),
            ry: Some(6.0),
            paint: Paint::solid(red()),
        },
        SceneCommand::FillPolygon {
            points: vec![90.0, 2.0, 116.0, 2.0, 104.0, 22.0],
            paint: Paint::solid(red()),
            fill_rule: FillRule::EvenOdd,
        },
        SceneCommand::FillPath {
            segments: vec![
                PathSegment::MoveTo { x: 4.0, y: 36.0 },
                PathSegment::CubicTo {
                    x1: 8.0,
                    y1: 24.0,
                    x2: 24.0,
                    y2: 24.0,
                    x: 28.0,
                    y: 36.0,
                },
                PathSegment::LineTo { x: 4.0, y: 36.0 },
                PathSegment::Close,
            ],
            paint: Paint::solid(red()),
            fill_rule: FillRule::NonZero,
        },
        SceneCommand::StrokeRect {
            x: 34.0,
            y: 30.0,
            w: 20.0,
            h: 18.0,
            color: red(),
            stroke_width: 4.0,
            stroke_dash: None,
            stroke_gap: None,
            stroke_linecap: None,
        },
        SceneCommand::StrokeRoundedRect {
            x: 62.0,
            y: 30.0,
            w: 24.0,
            h: 20.0,
            radius: 6.0,
            radii: None,
            color: red(),
            stroke_width: 3.0,
            stroke_dash: None,
            stroke_gap: None,
            stroke_linecap: None,
        },
        SceneCommand::StrokeEllipse {
            x: 96.0,
            y: 30.0,
            w: 24.0,
            h: 20.0,
            rx: None,
            ry: None,
            color: red(),
            stroke_width: 2.0,
            stroke_dash: None,
            stroke_gap: None,
            stroke_linecap: None,
        },
        SceneCommand::StrokeLine {
            x1: 4.0,
            y1: 64.0,
            x2: 48.0,
            y2: 64.0,
            color: red(),
            stroke_width: 4.0,
            stroke_dash: Some(6.0),
            stroke_gap: Some(4.0),
            stroke_linecap: Some(LineCap::Round),
        },
    ];
    let svg = check_native(&scene, 1.0);
    assert!(svg.contains("viewBox=\"0 0 128 96\""));
    assert!(svg.contains("stroke-dasharray"));
    assert!(svg.contains("evenodd"));
}

#[test]
fn aligned_path_and_polyline_strokes_match_reference() {
    for align in [
        StrokeAlign::Inside,
        StrokeAlign::Outside,
        StrokeAlign::Center,
    ] {
        let mut scene = Scene::new(80.0, 40.0);
        scene.commands.push(SceneCommand::StrokePolyline {
            points: vec![8.0, 8.0, 28.0, 8.0, 28.0, 28.0, 8.0, 28.0],
            color: Color::srgb(255, 0, 0, 128),
            stroke_width: 4.0,
            closed: true,
            align,
            clip_fill_rule: FillRule::EvenOdd,
        });
        scene.commands.push(SceneCommand::StrokePath {
            segments: vec![
                PathSegment::MoveTo { x: 48.0, y: 8.0 },
                PathSegment::LineTo { x: 68.0, y: 8.0 },
                PathSegment::LineTo { x: 68.0, y: 28.0 },
                PathSegment::LineTo { x: 48.0, y: 28.0 },
                PathSegment::Close,
            ],
            color: Color::srgb(255, 0, 0, 128),
            stroke_width: 4.0,
            closed: true,
            align,
            clip_fill_rule: FillRule::NonZero,
            stroke_linejoin: None,
            stroke_linecap: None,
            stroke_miter_limit: None,
        });
        check_native(&scene, 0.5);
    }
}

#[test]
fn linear_and_radial_gradients_match_reference() {
    for radial in [false, true] {
        let mut scene = Scene::new(40.0, 40.0);
        scene.commands.push(SceneCommand::FillRect {
            x: 0.0,
            y: 0.0,
            w: 40.0,
            h: 40.0,
            paint: Paint::Gradient(GradientPaint {
                angle_deg: 35.0,
                radial,
                center_x: Some(0.3),
                center_y: Some(0.6),
                radius_frac: Some(0.7),
                stops: vec![
                    GradientStop {
                        offset: 0.0,
                        color: Color::srgb(20, 80, 200, 255),
                    },
                    GradientStop {
                        offset: 1.0,
                        color: Color::srgb(220, 140, 30, 128),
                    },
                ],
            }),
        });
        let svg = check_native(&scene, 1.0);
        assert!(svg.contains(if radial {
            "<radialGradient"
        } else {
            "<linearGradient"
        }));
    }
}

#[test]
fn nested_clips_layers_and_transforms_match_reference() {
    let mut scene = Scene::new(80.0, 80.0);
    scene.commands = vec![
        SceneCommand::PushLayer {
            opacity: 0.5,
            blend_mode: Some(BlendMode::Normal),
        },
        SceneCommand::PushTransform {
            angle_deg: 25.0,
            cx: 40.0,
            cy: 40.0,
        },
        SceneCommand::PushClipRoundedRect {
            x: 10.0,
            y: 10.0,
            w: 60.0,
            h: 60.0,
            radius: 12.0,
        },
        SceneCommand::PushScaleTranslate {
            sx: 1.0,
            sy: 1.0,
            tx: 4.0,
            ty: 3.0,
        },
        SceneCommand::PushTransformMatrix {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 2.0,
            f: 1.0,
        },
        rect(8.0, 8.0, 70.0, 70.0),
        SceneCommand::PopTransform,
        SceneCommand::PopTransform,
        SceneCommand::PopClip,
        SceneCommand::PopTransform,
        SceneCommand::PopLayer,
    ];
    check_native(&scene, 1.0);
}

#[test]
fn effect_segments_embed_exact_reference_pixels_and_keep_siblings_vector() {
    let effects = vec![
        (
            SceneCommand::BeginBlur { radius: 2.0 },
            SceneCommand::EndBlur,
        ),
        (
            SceneCommand::BeginShadow {
                shadows: vec![ShadowSpec {
                    dx: 2.0,
                    dy: 3.0,
                    blur: 2.0,
                    color: Color::srgb(0, 0, 0, 180),
                }],
            },
            SceneCommand::EndShadow,
        ),
        (
            SceneCommand::BeginFilter {
                filters: vec![FilterSpec::Invert(1.0)],
            },
            SceneCommand::EndFilter,
        ),
        (
            SceneCommand::BeginMask {
                mask: MaskSpec {
                    shape: MaskShape::Ellipse,
                    radius: 0.0,
                    feather: 2.0,
                    invert: false,
                    x: 8.0,
                    y: 8.0,
                    w: 16.0,
                    h: 16.0,
                },
            },
            SceneCommand::EndMask,
        ),
    ];
    for (begin, end) in effects {
        let mut scene = Scene::new(48.0, 48.0);
        scene.commands = vec![
            SceneCommand::PushLayer {
                opacity: 0.7,
                blend_mode: None,
            },
            SceneCommand::PushTransform {
                angle_deg: 20.0,
                cx: 16.0,
                cy: 16.0,
            },
            SceneCommand::PushClipRoundedRect {
                x: 5.0,
                y: 5.0,
                w: 28.0,
                h: 28.0,
                radius: 4.0,
            },
            begin,
            rect(8.0, 8.0, 16.0, 16.0),
            end,
            SceneCommand::PopClip,
            SceneCommand::PopTransform,
            SceneCommand::PopLayer,
            rect(34.0, 34.0, 8.0, 8.0),
        ];
        let fonts = default_provider();
        let output = render_svg_with(&scene, &fonts, &no_assets()).expect("effect SVG");
        assert_eq!(output.rasterized_regions.len(), 1);
        let region = &output.rasterized_regions[0];
        assert_eq!((region.command_start, region.command_end), (0, 9));
        assert_eq!(region.reason, SvgRasterizationReason::Effects);
        let mut segment = Scene::new(48.0, 48.0);
        segment.commands = scene.commands[..9].to_vec();
        let reference = render_image(&segment, &fonts, &no_assets()).expect("segment reference");
        assert_eq!(embedded_png(&output.bytes).rgba, reference.rgba);
        assert!(data_url_bytes(&output.bytes, "image/png").starts_with(b"\x89PNG"));
        let svg = std::str::from_utf8(&output.bytes).expect("SVG text");
        assert!(svg.contains("<path") || svg.contains("<rect"));
        assert_pixels_close(
            &rasterize(&output.bytes),
            &render_image(&scene, &fonts, &no_assets()).expect("page reference"),
            0.1,
        );
    }
}

#[test]
fn empty_effects_keep_geometry_vector() {
    let mut scene = Scene::new(32.0, 32.0);
    scene.commands = vec![
        SceneCommand::BeginBlur { radius: 0.0 },
        SceneCommand::BeginShadow { shadows: vec![] },
        SceneCommand::BeginFilter { filters: vec![] },
        rect(4.0, 4.0, 24.0, 24.0),
        SceneCommand::EndFilter,
        SceneCommand::EndShadow,
        SceneCommand::EndBlur,
    ];
    check_native(&scene, 0.0);
}

#[test]
fn crossed_scopes_rasterize_and_non_normal_blends_capture_page_backdrop() {
    let mut crossed = Scene::new(32.0, 32.0);
    crossed.commands = vec![
        SceneCommand::PushClip {
            x: 4.0,
            y: 4.0,
            w: 24.0,
            h: 24.0,
        },
        SceneCommand::PushLayer {
            opacity: 0.5,
            blend_mode: None,
        },
        rect(0.0, 0.0, 32.0, 32.0),
        SceneCommand::PopClip,
        SceneCommand::PopLayer,
    ];
    let mut blend = Scene::new(32.0, 32.0);
    blend.commands = vec![
        rect(0.0, 0.0, 32.0, 32.0),
        SceneCommand::PushLayer {
            opacity: 0.7,
            blend_mode: Some(BlendMode::Multiply),
        },
        rect(4.0, 4.0, 24.0, 24.0),
        SceneCommand::PopLayer,
    ];
    for (scene, reason) in [
        (crossed, SvgRasterizationReason::CrossedScopes),
        (blend, SvgRasterizationReason::NonNormalBlend),
    ] {
        let fonts = default_provider();
        let output = render_svg_with(&scene, &fonts, &no_assets()).expect("fallback SVG");
        assert_eq!(output.rasterized_regions.len(), 1);
        assert_eq!(output.rasterized_regions[0].reason, reason);
        assert_eq!(output.rasterized_regions[0].command_start, 0);
        assert_eq!(
            output.rasterized_regions[0].command_end,
            scene.commands.len()
        );
        let reference = render_image(&scene, &fonts, &no_assets()).expect("reference");
        assert_eq!(embedded_png(&output.bytes).rgba, reference.rgba);
        assert!(data_url_bytes(&output.bytes, "image/png").starts_with(b"\x89PNG"));
    }
}

#[test]
fn invalid_dimensions_and_unbalanced_scopes_return_errors() {
    let fonts = default_provider();
    for (w, h) in [
        (0.0, 4.0),
        (-1.0, 4.0),
        (f64::NAN, 4.0),
        (4.0, f64::INFINITY),
    ] {
        assert!(render_svg(&Scene::new(w, h), &fonts, &no_assets()).is_err());
    }
    let commands = vec![
        SceneCommand::PopClip,
        SceneCommand::PopLayer,
        SceneCommand::PopTransform,
        SceneCommand::EndBlur,
        SceneCommand::EndShadow,
        SceneCommand::EndFilter,
        SceneCommand::EndMask,
        SceneCommand::PushClip {
            x: 0.0,
            y: 0.0,
            w: 4.0,
            h: 4.0,
        },
        SceneCommand::PushLayer {
            opacity: 1.0,
            blend_mode: None,
        },
        SceneCommand::PushTransform {
            angle_deg: 10.0,
            cx: 2.0,
            cy: 2.0,
        },
        SceneCommand::BeginBlur { radius: 2.0 },
        SceneCommand::BeginShadow { shadows: vec![] },
        SceneCommand::BeginFilter { filters: vec![] },
    ];
    for command in commands {
        let mut scene = Scene::new(4.0, 4.0);
        scene.commands.push(command.clone());
        assert!(
            render_svg(&scene, &fonts, &no_assets()).is_err(),
            "unbalanced {command:?}"
        );
    }
}
