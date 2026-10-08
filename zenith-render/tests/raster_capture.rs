mod svg_support;

use std::sync::Arc;
use svg_support::{assert_pixels_close, data_url_bytes, embedded_png, rasterize};
use zenith_core::{AssetKind, BytesAssetProvider, default_provider};
use zenith_render::{
    PdfExportOptions, PdfOptions, SvgOptions, render_image, render_image_scaled, render_pdf_report,
    render_pdf_report_with_options, render_svg_with, render_svg_with_options,
};
use zenith_scene::{BlendMode, Color, FitMode, Paint, Scene, SceneCommand};

fn shape(w: f64, h: f64) -> SceneCommand {
    SceneCommand::FillRect {
        x: 0.0,
        y: 0.0,
        w,
        h,
        paint: Paint::solid(Color::srgb(180, 40, 90, 255)),
    }
}
fn scene(w: f64, h: f64, capture: bool) -> Scene {
    let mut scene = Scene::new(w, h);
    scene.commands = if capture {
        vec![
            SceneCommand::BeginBlur { radius: 1.0 },
            shape(w, h),
            SceneCommand::EndBlur,
        ]
    } else {
        vec![shape(w, h)]
    };
    scene
}
#[test]
fn scale_one_retains_default_bytes_and_vector_scales_retain_native_bytes() {
    let fonts = default_provider();
    let assets = BytesAssetProvider::new();
    for capture in [false, true] {
        let scene = scene(15.0, 11.0, capture);
        assert_eq!(
            render_svg_with(&scene, &fonts, &assets).unwrap(),
            render_svg_with_options(&scene, &fonts, &assets, SvgOptions::default()).unwrap()
        );
        assert_eq!(
            render_pdf_report(&scene, &fonts, &assets, PdfOptions::default()).unwrap(),
            render_pdf_report_with_options(&scene, &fonts, &assets, PdfExportOptions::default())
                .unwrap()
        );
    }
    let scene = scene(15.0, 11.0, false);
    for scale in [0.25, 1.5, 2.0, 4.0, f64::MIN_POSITIVE] {
        assert_eq!(
            render_svg_with(&scene, &fonts, &assets).unwrap().bytes,
            render_svg_with_options(
                &scene,
                &fonts,
                &assets,
                SvgOptions {
                    raster_scale: scale
                }
            )
            .unwrap()
            .bytes
        );
        assert_eq!(
            render_pdf_report(&scene, &fonts, &assets, PdfOptions::default())
                .unwrap()
                .bytes,
            render_pdf_report_with_options(
                &scene,
                &fonts,
                &assets,
                PdfExportOptions {
                    raster_scale: scale,
                    ..Default::default()
                }
            )
            .unwrap()
            .bytes
        );
    }
}
#[test]
fn captures_use_requested_scale_for_pixels_and_placement() {
    let fonts = default_provider();
    let assets = BytesAssetProvider::new();
    for (w, h, scale) in [(15.0, 11.0, 2.0), (5.0, 3.0, 1.5), (1.0, 1.0, 0.25)] {
        let scene = scene(w, h, true);
        let expected = render_image_scaled(&scene, scale, &fonts, &assets).unwrap();
        if scale == 0.25 {
            assert_eq!((expected.width, expected.height), (1, 1));
        }
        let svg = render_svg_with_options(
            &scene,
            &fonts,
            &assets,
            SvgOptions {
                raster_scale: scale,
            },
        )
        .unwrap();
        let png = embedded_png(&svg.bytes);
        assert_eq!((png.width, png.height), (expected.width, expected.height));
        let png_bytes = data_url_bytes(&svg.bytes, "image/png");
        assert!(
            png_bytes == zenith_render::encode_png(&expected).unwrap(),
            "embedded PNG differs from the scaled capture encoder"
        );
        assert!(!png_bytes.is_empty());
        let text = String::from_utf8(svg.bytes).unwrap();
        assert!(
            text.contains(&format!(
                "width=\"{}\" height=\"{}\" preserveAspectRatio",
                f64::from(expected.width) / scale,
                f64::from(expected.height) / scale
            )),
            "{text}"
        );
        let pdf = render_pdf_report_with_options(
            &scene,
            &fonts,
            &assets,
            PdfExportOptions {
                raster_scale: scale,
                ..Default::default()
            },
        )
        .unwrap();
        let text = String::from_utf8_lossy(&pdf.bytes);
        if expected
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[3] > 0)
        {
            assert!(
                text.contains(&format!("/Width {}", expected.width)),
                "{text}"
            );
            assert!(
                text.contains(&format!("/Height {}", expected.height)),
                "{text}"
            );
            assert!(
                text.contains(&format!(
                    "{} 0 0 -{} 0 {} cm",
                    (f64::from(expected.width) / scale) as f32,
                    (f64::from(expected.height) / scale) as f32,
                    (f64::from(expected.height) / scale) as f32
                )),
                "{text}"
            );
        }
    }
}
#[test]
fn transformed_clipped_blur_and_blend_capture_scaled_pixels() {
    let fonts = default_provider();
    let assets = BytesAssetProvider::new();
    for blend_mode in [None, Some(BlendMode::Multiply)] {
        let mut scene = scene(31.0, 23.0, false);
        scene.commands = vec![
            SceneCommand::PushLayer {
                opacity: 1.0,
                blend_mode,
            },
            SceneCommand::PushScaleTranslate {
                sx: 1.2,
                sy: 1.1,
                tx: 3.0,
                ty: 2.0,
            },
            SceneCommand::PushClip {
                x: 0.0,
                y: 0.0,
                w: 17.0,
                h: 15.0,
            },
            SceneCommand::BeginBlur { radius: 1.0 },
            shape(20.0, 18.0),
            SceneCommand::EndBlur,
            SceneCommand::PopClip,
            SceneCommand::PopTransform,
            SceneCommand::PopLayer,
        ];
        let svg =
            render_svg_with_options(&scene, &fonts, &assets, SvgOptions { raster_scale: 2.0 })
                .unwrap();
        let png = embedded_png(&svg.bytes);
        let expected = render_image_scaled(&scene, 2.0, &fonts, &assets).unwrap();
        assert_eq!((png.width, png.height), (expected.width, expected.height));
        assert!(
            data_url_bytes(&svg.bytes, "image/png")
                == zenith_render::encode_png(&expected).unwrap(),
            "embedded PNG differs from the scaled capture encoder"
        );
        assert_pixels_close(
            &rasterize(&svg.bytes),
            &render_image(&scene, &fonts, &assets).unwrap(),
            10.0,
        );
        assert_eq!(
            render_pdf_report_with_options(
                &scene,
                &fonts,
                &assets,
                PdfExportOptions {
                    raster_scale: 2.0,
                    ..Default::default()
                }
            )
            .unwrap()
            .rasterized_regions
            .len(),
            1
        );
    }
}
#[test]
fn invalid_scales_and_capture_coordinate_overflow_return_errors() {
    let fonts = default_provider();
    let assets = BytesAssetProvider::new();
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY, 4.1] {
        for capture in [false, true] {
            let scene = scene(15.0, 11.0, capture);
            assert!(
                render_svg_with_options(
                    &scene,
                    &fonts,
                    &assets,
                    SvgOptions {
                        raster_scale: scale
                    }
                )
                .unwrap_err()
                .message
                .contains("invalid raster capture scale")
            );
            assert!(
                render_pdf_report_with_options(
                    &scene,
                    &fonts,
                    &assets,
                    PdfExportOptions {
                        raster_scale: scale,
                        ..Default::default()
                    }
                )
                .unwrap_err()
                .message
                .contains("invalid raster capture scale")
            );
        }
    }
    let scene = scene(15.0, 11.0, true);
    let scale = f64::from_bits(1);
    assert!(
        render_svg_with_options(
            &scene,
            &fonts,
            &assets,
            SvgOptions {
                raster_scale: scale
            }
        )
        .unwrap_err()
        .message
        .contains("placement")
    );
    assert!(
        render_pdf_report_with_options(
            &scene,
            &fonts,
            &assets,
            PdfExportOptions {
                raster_scale: scale,
                ..Default::default()
            }
        )
        .unwrap_err()
        .message
        .contains("placement")
    );
}
#[test]
fn oversized_svg_intermediates_error_in_both_scaled_captures() {
    let fonts = default_provider();
    let mut assets = BytesAssetProvider::new();
    assets.register("asset",AssetKind::Svg,Arc::from(&b"<svg xmlns='http://www.w3.org/2000/svg' width='1000000000000' height='1000000000000'><rect width='1000000000000' height='1000000000000' fill='red'/></svg>"[..]));
    let image = SceneCommand::DrawImage {
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
    };
    let mut scene = Scene::new(24.0, 24.0);
    scene.commands = vec![image.clone()];
    assert!(
        render_svg_with_options(&scene, &fonts, &assets, SvgOptions { raster_scale: 2.0 }).is_ok()
    );
    assert!(
        render_pdf_report_with_options(
            &scene,
            &fonts,
            &assets,
            PdfExportOptions {
                raster_scale: 2.0,
                ..Default::default()
            }
        )
        .is_ok()
    );
    scene.commands = vec![
        SceneCommand::BeginBlur { radius: 1.0 },
        image,
        SceneCommand::EndBlur,
    ];
    assert!(
        render_svg_with_options(&scene, &fonts, &assets, SvgOptions { raster_scale: 2.0 })
            .unwrap_err()
            .message
            .contains("unsupported intermediate dimensions")
    );
    assert!(
        render_pdf_report_with_options(
            &scene,
            &fonts,
            &assets,
            PdfExportOptions {
                raster_scale: 2.0,
                ..Default::default()
            }
        )
        .unwrap_err()
        .message
        .contains("unsupported intermediate dimensions")
    );
}
