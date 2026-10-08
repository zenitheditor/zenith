use super::{
    PdfOptions, PdfRasterizationReason as Reason, render_pdf_multi_report, render_pdf_report,
    render_pdf_with,
};
use std::sync::Arc;
use zenith_core::{AssetKind, BytesAssetProvider, default_provider};
use zenith_scene::{BlendMode, Color, FitMode, Paint, Scene, SceneCommand};

fn shape() -> SceneCommand {
    SceneCommand::FillRect {
        x: 2.0,
        y: 3.0,
        w: 10.0,
        h: 8.0,
        paint: Paint::solid(Color::srgb(200, 40, 60, 255)),
    }
}
fn scene(commands: Vec<SceneCommand>) -> Scene {
    let mut scene = Scene::new(24.0, 24.0);
    scene.commands = commands;
    scene
}
fn image() -> SceneCommand {
    SceneCommand::DrawImage {
        x: 0.0,
        y: 0.0,
        w: 20.0,
        h: 20.0,
        asset_id: "asset".into(),
        fit: FitMode::Stretch,
        pos_x: 0.0,
        pos_y: 0.0,
        opacity: 1.0,
        clip_shape: None,
        src_rect: None,
        svg_style: None,
    }
}
#[test]
fn native_and_captured_documents_retain_legacy_bytes() {
    let fonts = default_provider();
    let assets = BytesAssetProvider::new();
    for commands in [
        vec![shape()],
        vec![
            shape(),
            SceneCommand::BeginBlur { radius: 1.0 },
            shape(),
            SceneCommand::EndBlur,
            shape(),
        ],
    ] {
        let scene = scene(commands);
        for subset in [true, false] {
            let options = PdfOptions { subset };
            let output = render_pdf_report(&scene, &fonts, &assets, options).unwrap();
            assert_eq!(
                output.bytes,
                render_pdf_with(&scene, &fonts, &assets, options)
            );
            if scene.commands.len() == 1 {
                assert!(output.rasterized_regions.is_empty());
            } else {
                let region = &output.rasterized_regions[0];
                assert_eq!(
                    (
                        region.page,
                        region.command_start,
                        region.command_end,
                        region.reason
                    ),
                    (1, 1, 4, Reason::Effects)
                );
            }
        }
    }
}
#[test]
fn reports_preserve_page_order_and_reason_precedence() {
    let scenes = [
        scene(vec![
            SceneCommand::PushLayer {
                opacity: 0.5,
                blend_mode: None,
            },
            SceneCommand::BeginBlur { radius: 0.0 },
            shape(),
            SceneCommand::EndBlur,
            SceneCommand::PopLayer,
        ]),
        scene(vec![
            SceneCommand::PushLayer {
                opacity: 0.5,
                blend_mode: None,
            },
            shape(),
            SceneCommand::PopLayer,
        ]),
        scene(vec![
            shape(),
            SceneCommand::PushLayer {
                opacity: 1.0,
                blend_mode: Some(BlendMode::Multiply),
            },
            shape(),
            SceneCommand::PopLayer,
            shape(),
        ]),
        scene(vec![
            SceneCommand::PushScaleTranslate {
                sx: 1.0,
                sy: 1.0,
                tx: 1.0,
                ty: 1.0,
            },
            SceneCommand::PushClip {
                x: 0.0,
                y: 0.0,
                w: 20.0,
                h: 20.0,
            },
            shape(),
            SceneCommand::PopTransform,
            SceneCommand::PopClip,
        ]),
    ];
    let fonts = default_provider();
    let assets = BytesAssetProvider::new();
    let output = render_pdf_multi_report(&scenes, &fonts, &assets, PdfOptions::default()).unwrap();
    assert_eq!(
        output.bytes,
        super::render_pdf_multi(&scenes, &fonts, &assets)
    );
    let regions: Vec<_> = output
        .rasterized_regions
        .iter()
        .map(|r| (r.page, r.command_start, r.command_end, r.reason))
        .collect();
    assert_eq!(
        regions,
        vec![
            (1, 0, 5, Reason::Effects),
            (2, 0, 3, Reason::GroupOpacity),
            (3, 0, 5, Reason::NonNormalBlend),
            (4, 0, 5, Reason::CrossedScopes)
        ]
    );
}
#[test]
fn malformed_scopes_and_dimensions_return_context() {
    let fonts = default_provider();
    let assets = BytesAssetProvider::new();
    for commands in [
        vec![SceneCommand::PopLayer],
        vec![SceneCommand::BeginBlur { radius: 1.0 }, shape()],
    ] {
        let error = render_pdf_report(&scene(commands), &fonts, &assets, PdfOptions::default())
            .unwrap_err();
        assert!(
            error.message.contains("PDF page 1 malformed scopes"),
            "{error}"
        );
    }
    let mut scene = scene(vec![
        SceneCommand::BeginBlur { radius: 1.0 },
        shape(),
        SceneCommand::EndBlur,
    ]);
    scene.width = -1.0;
    assert!(
        render_pdf_report(&scene, &fonts, &assets, PdfOptions::default())
            .unwrap_err()
            .message
            .contains("invalid dimensions")
    );
    scene.width = f64::from(u32::MAX) + 1.0;
    let error = render_pdf_report(&scene, &fonts, &assets, PdfOptions::default()).unwrap_err();
    assert!(
        error.message.contains("PDF page 1")
            && error.message.contains("commands 0..3 capture error"),
        "{error}"
    );
}
#[test]
fn unresolved_and_corrupt_assets_error_inside_and_outside_captures() {
    let fonts = default_provider();
    let mut corrupt_png = BytesAssetProvider::new();
    corrupt_png.register("asset", AssetKind::Image, Arc::from(&b"bad PNG"[..]));
    let mut corrupt_svg = BytesAssetProvider::new();
    corrupt_svg.register("asset", AssetKind::Svg, Arc::from(&b"<broken"[..]));
    let mut external_svg = BytesAssetProvider::new();
    external_svg.register("asset", AssetKind::Svg, Arc::from(&b"<svg xmlns='http://www.w3.org/2000/svg' xmlns:xlink='http://www.w3.org/1999/xlink' width='20' height='20'><image width='20' height='20' xlink:href='missing.png'/></svg>"[..]));
    for assets in [
        BytesAssetProvider::new(),
        corrupt_png,
        corrupt_svg,
        external_svg,
    ] {
        for commands in [
            vec![image()],
            vec![
                SceneCommand::BeginBlur { radius: 1.0 },
                image(),
                SceneCommand::EndBlur,
            ],
        ] {
            let error = render_pdf_report(&scene(commands), &fonts, &assets, PdfOptions::default())
                .unwrap_err();
            assert!(
                error.message.contains("asset") && error.message.contains("command"),
                "{error}"
            );
        }
    }
}
#[test]
fn unsupported_svg_command_errors_before_capture_emission() {
    let fonts = default_provider();
    let assets = BytesAssetProvider::new();
    let svg = SceneCommand::DrawSvgAsset {
        x: 0.0,
        y: 0.0,
        w: 20.0,
        h: 20.0,
        asset: "legacy.svg".into(),
    };
    for commands in [
        vec![svg.clone()],
        vec![
            SceneCommand::BeginBlur { radius: 1.0 },
            svg,
            SceneCommand::EndBlur,
        ],
    ] {
        let error = render_pdf_report(&scene(commands), &fonts, &assets, PdfOptions::default())
            .unwrap_err();
        assert!(
            error.message.contains("legacy.svg")
                && error.message.contains("DrawImage")
                && error.message.contains("command"),
            "{error}"
        );
    }
}
#[test]
fn complex_svg_report_matches_legacy_capture() {
    let fonts = default_provider();
    let mut assets = BytesAssetProvider::new();
    assets.register("asset", AssetKind::Svg, Arc::from(&b"<svg xmlns='http://www.w3.org/2000/svg' width='20' height='20'><g opacity='.5'><rect width='15' height='15' fill='red'/><rect x='5' y='5' width='15' height='15' fill='blue'/></g></svg>"[..]));
    let scene = scene(vec![image()]);
    let output = render_pdf_report(&scene, &fonts, &assets, PdfOptions::default()).unwrap();
    assert_eq!(output.bytes, super::render_pdf(&scene, &fonts, &assets));
    assert_eq!(output.rasterized_regions[0].reason, Reason::SvgAsset);
}

#[test]
fn gradient_capture_reason_retains_legacy_bytes() {
    use zenith_scene::{GradientPaint, GradientStop};
    let paint = GradientPaint {
        angle_deg: 0.0,
        radial: false,
        center_x: None,
        center_y: None,
        radius_frac: None,
        stops: vec![
            GradientStop {
                offset: 0.0,
                color: Color::srgb(255, 0, 0, 128),
            },
            GradientStop {
                offset: 1.0,
                color: Color::srgb(0, 0, 255, 255),
            },
        ],
    };
    let scene = scene(vec![SceneCommand::FillRect {
        x: 2.0,
        y: 2.0,
        w: 20.0,
        h: 20.0,
        paint: Paint::Gradient(paint),
    }]);
    let fonts = default_provider();
    let assets = BytesAssetProvider::new();
    let output = render_pdf_report(&scene, &fonts, &assets, PdfOptions::default()).unwrap();
    assert_eq!(output.rasterized_regions[0].reason, Reason::Gradient);
    assert_eq!(output.bytes, super::render_pdf(&scene, &fonts, &assets));
}
#[test]
fn missing_fonts_and_out_of_range_glyphs_return_context() {
    use zenith_core::FontProvider;
    use zenith_scene::SceneGlyph;
    let fonts = default_provider();
    let data = fonts.all_faces();
    let font = data.first().unwrap();
    let run = SceneCommand::DrawGlyphRun {
        x: 0.0,
        y: 12.0,
        font_id: font.id.clone(),
        font_size: 12.0,
        color: Color::srgb(0, 0, 0, 255),
        stroke_color: None,
        stroke_width: None,
        link: None,
        selectable: false,
        source_node_id: None,
        glyphs: vec![SceneGlyph {
            glyph_id: u16::MAX,
            dx: 0.0,
            dy: 0.0,
            text: "A".into(),
        }],
    };
    let assets = BytesAssetProvider::new();
    let error = render_pdf_report(
        &scene(vec![run.clone()]),
        &fonts,
        &assets,
        PdfOptions::default(),
    )
    .unwrap_err();
    assert!(
        error.message.contains("command 0") && error.message.contains("glyph 65535"),
        "{error}"
    );
    let mut run = run;
    if let SceneCommand::DrawGlyphRun { font_id, .. } = &mut run {
        *font_id = "unregistered".into();
    }
    let error = render_pdf_report(
        &scene(vec![
            SceneCommand::BeginBlur { radius: 1.0 },
            run,
            SceneCommand::EndBlur,
        ]),
        &fonts,
        &assets,
        PdfOptions::default(),
    )
    .unwrap_err();
    assert!(
        error.message.contains("command 1") && error.message.contains("unregistered"),
        "{error}"
    );
}

#[test]
fn oversized_svg_intermediate_errors_only_inside_captures() {
    let fonts = default_provider();
    let mut assets = BytesAssetProvider::new();
    assets.register("asset", AssetKind::Svg, Arc::from(&b"<svg xmlns='http://www.w3.org/2000/svg' width='1000000000000' height='1000000000000'><rect width='1000000000000' height='1000000000000' fill='red'/></svg>"[..]));
    let native = scene(vec![image()]);
    let output = render_pdf_report(&native, &fonts, &assets, PdfOptions::default()).unwrap();
    assert!(output.rasterized_regions.is_empty());
    assert_eq!(output.bytes, super::render_pdf(&native, &fonts, &assets));
    let captured = scene(vec![
        SceneCommand::BeginBlur { radius: 1.0 },
        image(),
        SceneCommand::EndBlur,
    ]);
    let error = render_pdf_report(&captured, &fonts, &assets, PdfOptions::default()).unwrap_err();
    assert!(
        error.message.contains("PDF page 1")
            && error
                .message
                .contains("commands 0..3 capture command 1 SVG asset asset")
            && error
                .message
                .contains("unsupported intermediate dimensions"),
        "{error}"
    );
}
