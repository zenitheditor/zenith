use super::check_scene;
use crate::{PdfOptions, render_pdf_report, render_pdf_with};
use std::sync::Arc;
use zenith_core::{AssetKind, BytesAssetProvider, default_provider};
use zenith_scene::ir::PathSegment;
use zenith_scene::{
    Color, FillRule, FilterSpec, FitMode, GradientPaint, GradientStop, MaskShape, MaskSpec, Paint,
    Scene, SceneCommand, SceneGlyph, SrcRect, StrokeAlign, SvgStyle,
};

fn shape(value: f64) -> SceneCommand {
    SceneCommand::FillRect {
        x: value,
        y: 0.0,
        w: 2.0,
        h: 2.0,
        paint: Paint::solid(Color::srgb(10, 20, 30, 255)),
    }
}
fn scene(commands: Vec<SceneCommand>) -> Scene {
    let mut scene = Scene::new(16.0, 16.0);
    scene.commands = commands;
    scene
}
fn image() -> SceneCommand {
    SceneCommand::DrawImage {
        x: 0.0,
        y: 0.0,
        w: 8.0,
        h: 8.0,
        asset_id: "asset".into(),
        fit: FitMode::Cover,
        pos_x: 50.0,
        pos_y: 50.0,
        opacity: 1.0,
        clip_shape: None,
        src_rect: None,
        svg_style: None,
    }
}
fn gradient(center: Option<f64>) -> Paint {
    Paint::Gradient(GradientPaint {
        angle_deg: 0.0,
        radial: center.is_some(),
        center_x: center,
        center_y: None,
        radius_frac: None,
        stops: vec![
            GradientStop {
                offset: 0.0,
                color: Color::srgb(0, 0, 0, 255),
            },
            GradientStop {
                offset: 1.0,
                color: Color::srgb(255, 255, 255, 255),
            },
        ],
    })
}

#[test]
fn numeric_families_report_page_command_field_and_value_before_resources() {
    let nan = f64::NAN;
    let mut invalid_image = image();
    if let SceneCommand::DrawImage { opacity, .. } = &mut invalid_image {
        *opacity = nan;
    }
    let glyph = SceneCommand::DrawGlyphRun {
        x: 0.0,
        y: 0.0,
        font_id: "missing".into(),
        font_size: f32::NAN,
        color: Color::srgb(0, 0, 0, 255),
        stroke_color: None,
        stroke_width: None,
        link: None,
        selectable: true,
        source_node_id: None,
        glyphs: vec![],
    };
    let commands = vec![
        shape(nan),
        SceneCommand::FillEllipse {
            x: 0.0,
            y: 0.0,
            w: 2.0,
            h: 2.0,
            rx: Some(f64::INFINITY),
            ry: None,
            paint: Paint::solid(Color::srgb(0, 0, 0, 255)),
        },
        SceneCommand::FillPath {
            segments: vec![PathSegment::CubicTo {
                x1: 0.0,
                y1: nan,
                x2: 1.0,
                y2: 1.0,
                x: 2.0,
                y: 2.0,
            }],
            paint: Paint::solid(Color::srgb(0, 0, 0, 255)),
            fill_rule: FillRule::NonZero,
        },
        SceneCommand::FillPolygon {
            points: vec![0.0, 0.0, 1.0, 1.0, nan],
            paint: Paint::solid(Color::srgb(0, 0, 0, 255)),
            fill_rule: FillRule::NonZero,
        },
        SceneCommand::StrokeLine {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
            color: Color::srgb(0, 0, 0, 255),
            stroke_width: 1e100,
            stroke_dash: None,
            stroke_gap: None,
            stroke_linecap: None,
        },
        SceneCommand::FillRect {
            x: 0.0,
            y: 0.0,
            w: 2.0,
            h: 2.0,
            paint: Paint::solid(Color {
                cmyk: Some([f32::NAN, 0.0, 0.0, 0.0]),
                ..Color::srgb(0, 0, 0, 255)
            }),
        },
        SceneCommand::FillRect {
            x: 0.0,
            y: 0.0,
            w: 2.0,
            h: 2.0,
            paint: gradient(Some(f64::INFINITY)),
        },
        invalid_image,
        glyph,
        SceneCommand::PushClip {
            x: 0.0,
            y: 0.0,
            w: f64::INFINITY,
            h: 2.0,
        },
        SceneCommand::PushTransformMatrix {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 1e100,
            f: 0.0,
        },
        SceneCommand::BeginShadow {
            shadows: vec![zenith_scene::ShadowSpec {
                dx: nan,
                dy: 0.0,
                blur: 0.0,
                color: Color::srgb(0, 0, 0, 255),
            }],
        },
        SceneCommand::BeginFilter {
            filters: vec![FilterSpec::HueRotate(nan)],
        },
        SceneCommand::BeginMask {
            mask: MaskSpec {
                shape: MaskShape::Rect,
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0,
                radius: 0.0,
                feather: nan,
                invert: false,
            },
        },
    ];
    let fonts = default_provider();
    let assets = BytesAssetProvider::new();
    for command in commands {
        for captured in [false, true] {
            let commands = if captured {
                vec![
                    SceneCommand::BeginBlur { radius: 1.0 },
                    command.clone(),
                    SceneCommand::EndBlur,
                ]
            } else {
                vec![command.clone()]
            };
            let error = render_pdf_report(&scene(commands), &fonts, &assets, PdfOptions::default())
                .unwrap_err();
            assert!(
                error.message.contains("PDF page 1 command"),
                "{command:?}: {error}"
            );
            assert!(error.message.contains('='), "{error}");
            assert!(error.message.contains("Supply"), "{error}");
        }
    }
    let mut trim = scene(vec![]);
    trim.trim = Some(zenith_scene::Rect {
        x: 0.0,
        y: 0.0,
        w: f64::INFINITY,
        h: 1.0,
    });
    assert!(
        check_scene(&trim, 2)
            .unwrap_err()
            .message
            .contains("PDF page 2 command 0")
    );
}

#[test]
fn derived_endpoints_glyph_positions_gradients_and_composed_matrices_return_errors() {
    let max = f64::from(f32::MAX);
    let mut rect = shape(max);
    if let SceneCommand::FillRect { w, .. } = &mut rect {
        *w = max;
    }
    let glyph = SceneCommand::DrawGlyphRun {
        x: max,
        y: 0.0,
        font_id: "missing".into(),
        font_size: 1.0,
        color: Color::srgb(0, 0, 0, 255),
        stroke_color: None,
        stroke_width: None,
        link: None,
        selectable: true,
        source_node_id: None,
        glyphs: vec![SceneGlyph {
            glyph_id: 0,
            dx: f32::MAX,
            dy: 0.0,
            text: String::new(),
        }],
    };
    for commands in [
        vec![rect],
        vec![glyph],
        vec![SceneCommand::FillRect {
            x: 0.0,
            y: 0.0,
            w: 2.0,
            h: 2.0,
            paint: gradient(Some(max)),
        }],
        vec![
            SceneCommand::PushScaleTranslate {
                sx: max,
                sy: 1.0,
                tx: 0.0,
                ty: 0.0,
            },
            SceneCommand::PushScaleTranslate {
                sx: 2.0,
                sy: 1.0,
                tx: 0.0,
                ty: 0.0,
            },
        ],
        vec![
            SceneCommand::PushScaleTranslate {
                sx: max,
                sy: 1.0,
                tx: 0.0,
                ty: 0.0,
            },
            shape(2.0),
        ],
    ] {
        assert!(check_scene(&scene(commands), 1).is_err());
    }
    let mut outside = scene(vec![SceneCommand::StrokePolyline {
        points: vec![0.0, 0.0, 1.0, 1.0],
        color: Color::srgb(0, 0, 0, 255),
        stroke_width: 1.0,
        closed: true,
        align: StrokeAlign::Outside,
        clip_fill_rule: FillRule::NonZero,
    }]);
    outside.width = 3e38;
    assert!(
        check_scene(&outside, 1)
            .unwrap_err()
            .message
            .contains("outside_clip")
    );
}

#[test]
fn finite_degeneracies_and_clamped_or_ignored_fields_keep_legacy_bytes() {
    let fonts = default_provider();
    let assets = BytesAssetProvider::new();
    let mut paint = gradient(None);
    if let Paint::Gradient(ref mut gradient) = paint {
        gradient.stops.first_mut().unwrap().offset = -1e100;
        gradient.stops.last_mut().unwrap().offset = 1e100;
    }
    let scenes = vec![
        scene(vec![shape(-2.0)]),
        scene(vec![
            SceneCommand::PushScaleTranslate {
                sx: -1.0,
                sy: 0.0,
                tx: 0.0,
                ty: 0.0,
            },
            shape(0.0),
            SceneCommand::PopTransform,
        ]),
        scene(vec![
            SceneCommand::PushClip {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            },
            shape(0.0),
            SceneCommand::PopClip,
        ]),
        scene(vec![SceneCommand::FillPolygon {
            points: vec![],
            paint: Paint::solid(Color::srgb(0, 0, 0, 0)),
            fill_rule: FillRule::NonZero,
        }]),
        scene(vec![SceneCommand::FillRect {
            x: 0.0,
            y: 0.0,
            w: 8.0,
            h: 8.0,
            paint,
        }]),
        scene(vec![
            SceneCommand::BeginBlur { radius: 0.0 },
            shape(0.0),
            SceneCommand::EndBlur,
        ]),
        scene(vec![
            SceneCommand::BeginMask {
                mask: MaskSpec {
                    shape: MaskShape::Rect,
                    x: 0.0,
                    y: 0.0,
                    w: 1.0,
                    h: 1.0,
                    radius: f64::NAN,
                    feather: 0.0,
                    invert: false,
                },
            },
            shape(0.0),
            SceneCommand::EndMask,
        ]),
    ];
    for scene in scenes {
        assert_eq!(
            render_pdf_report(&scene, &fonts, &assets, PdfOptions::default())
                .unwrap()
                .bytes,
            render_pdf_with(&scene, &fonts, &assets, PdfOptions::default())
        );
    }
}

#[test]
fn image_fit_checks_effective_dimensions_and_respects_ignored_fields_and_empty_crops() {
    let fonts = default_provider();
    let mut raster = BytesAssetProvider::new();
    let mut svg = BytesAssetProvider::new();
    let mut pixels = tiny_skia::Pixmap::new(1, 2).unwrap();
    pixels.fill(tiny_skia::Color::WHITE);
    raster.register(
        "asset",
        AssetKind::Image,
        Arc::from(pixels.encode_png().unwrap()),
    );
    svg.register("asset",AssetKind::Svg,Arc::from(&b"<svg xmlns='http://www.w3.org/2000/svg' width='1' height='2'><rect width='1' height='2'/></svg>"[..]));
    let mut fit = image();
    if let SceneCommand::DrawImage { w, h, .. } = &mut fit {
        *w = 3e38;
        *h = 1.0;
    }
    let error =
        render_pdf_report(&scene(vec![fit]), &fonts, &raster, PdfOptions::default()).unwrap_err();
    assert!(error.message.contains("image.fit.height"), "{error}");
    let mut ignored = image();
    if let SceneCommand::DrawImage { svg_style, .. } = &mut ignored {
        *svg_style = Some(SvgStyle {
            stroke: None,
            fill: None,
            stroke_width: Some(f64::NAN),
        });
    }
    let accepted = scene(vec![ignored]);
    assert_eq!(
        render_pdf_report(&accepted, &fonts, &raster, PdfOptions::default())
            .unwrap()
            .bytes,
        render_pdf_with(&accepted, &fonts, &raster, PdfOptions::default())
    );
    let mut ignored = image();
    if let SceneCommand::DrawImage { src_rect, .. } = &mut ignored {
        *src_rect = Some(SrcRect {
            x: f64::NAN,
            y: 0.0,
            w: 1.0,
            h: 1.0,
        });
    }
    let accepted = scene(vec![ignored]);
    assert_eq!(
        render_pdf_report(&accepted, &fonts, &svg, PdfOptions::default())
            .unwrap()
            .bytes,
        render_pdf_with(&accepted, &fonts, &svg, PdfOptions::default())
    );
    let mut empty = image();
    if let SceneCommand::DrawImage { src_rect, .. } = &mut empty {
        *src_rect = Some(SrcRect {
            x: 100.0,
            y: 100.0,
            w: 1.0,
            h: 1.0,
        });
    }
    let accepted = scene(vec![empty]);
    assert_eq!(
        render_pdf_report(&accepted, &fonts, &raster, PdfOptions::default())
            .unwrap()
            .bytes,
        render_pdf_with(&accepted, &fonts, &raster, PdfOptions::default())
    );
}

#[test]
fn captured_geometry_checks_positions_under_the_requested_device_scale() {
    let fonts = default_provider();
    let assets = BytesAssetProvider::new();
    let scene = scene(vec![
        SceneCommand::BeginBlur { radius: 1.0 },
        shape(1e38),
        SceneCommand::EndBlur,
    ]);
    assert!(render_pdf_report(&scene, &fonts, &assets, PdfOptions::default()).is_ok());
    let error = crate::render_pdf_report_with_options(
        &scene,
        &fonts,
        &assets,
        crate::PdfExportOptions {
            raster_scale: 4.0,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(error.message.contains("transformed.x"), "{error}");
}
