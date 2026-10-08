//! Raster source crops and PDF image placement coverage.

use std::sync::Arc;

use miniz_oxide::inflate::decompress_to_vec_zlib;
use tiny_skia::Pixmap;
use zenith_core::{AssetKind, BytesAssetProvider, default_provider};
use zenith_scene::{FitMode, ImageClip, Scene, SceneCommand, SrcRect};

use super::{content::translate, font::build_plan, render_pdf};
use crate::{RasterBackend, TinySkiaBackend};

const PIXELS: [[u8; 4]; 12] = [
    [255, 0, 0, 255],
    [0, 255, 0, 255],
    [0, 0, 255, 255],
    [255, 255, 255, 255],
    [255, 255, 0, 255],
    [255, 0, 255, 128],
    [0, 255, 255, 255],
    [0, 0, 0, 0],
    [0, 0, 255, 64],
    [255, 0, 0, 255],
    [0, 255, 0, 255],
    [255, 255, 255, 255],
];

fn png() -> Vec<u8> {
    let mut pixmap = Pixmap::new(4, 3).unwrap();
    for (pixel, [r, g, b, a]) in pixmap.pixels_mut().iter_mut().zip(PIXELS) {
        *pixel = tiny_skia::ColorU8::from_rgba(r, g, b, a).premultiply();
    }
    pixmap.encode_png().unwrap()
}

fn assets() -> BytesAssetProvider {
    let mut assets = BytesAssetProvider::new();
    assets.register("asset.image", AssetKind::Image, Arc::from(png()));
    assets
}

fn crop(x: f64, y: f64, w: f64, h: f64) -> SrcRect {
    SrcRect { x, y, w, h }
}

fn scene(fit: FitMode, src_rect: Option<SrcRect>, clip_shape: Option<ImageClip>) -> Scene {
    let mut scene = Scene::new(100.0, 80.0);
    scene.commands.push(SceneCommand::DrawImage {
        x: 10.0,
        y: 20.0,
        w: 20.0,
        h: 20.0,
        asset_id: "asset.image".into(),
        fit,
        pos_x: 25.0,
        pos_y: 75.0,
        opacity: 1.0,
        clip_shape,
        src_rect,
        svg_style: None,
    });
    scene
}

fn planes(scene: &Scene, expected_size: (u32, u32), indices: &[usize]) -> String {
    let fonts = default_provider();
    let plan = build_plan(&Default::default(), &fonts, true);
    let (content, resources) = translate(scene, &fonts, &assets(), &plan);
    assert_eq!(resources.images.len(), 1);
    let image = &resources.images[0];
    assert_eq!((image.width, image.height), expected_size);
    let rgb = decompress_to_vec_zlib(&image.rgb_flate).unwrap();
    let expected_rgb: Vec<_> = indices
        .iter()
        .flat_map(|&i| {
            let [r, g, b, a] = PIXELS[i];
            if a == 0 { [0, 0, 0] } else { [r, g, b] }
        })
        .collect();
    assert_eq!(rgb, expected_rgb);
    let expected_alpha: Vec<_> = indices.iter().map(|&i| PIXELS[i][3]).collect();
    if expected_alpha.iter().all(|&a| a == 255) {
        assert!(image.alpha_flate.is_none());
    } else {
        assert_eq!(
            decompress_to_vec_zlib(image.alpha_flate.as_ref().unwrap()).unwrap(),
            expected_alpha
        );
    }
    String::from_utf8(content.finish().into_vec()).unwrap()
}

#[test]
fn source_crop_precedes_every_fit_transform() {
    for (fit, matrix) in [
        (FitMode::Stretch, "20 0 0 -20 10 40 cm"),
        (FitMode::Contain, "20 0 0 -10 10 37.5 cm"),
        (FitMode::Cover, "40 0 0 -20 5 40 cm"),
        (FitMode::None, "2 0 0 -1 14.5 35.25 cm"),
    ] {
        let scene = scene(fit, Some(crop(1.0, 1.0, 2.0, 1.0)), None);
        let content = planes(&scene, (2, 1), &[5, 6]);
        assert!(content.contains(matrix), "{fit:?}: {content}");
    }
}

#[test]
fn source_crop_truncates_clamped_endpoints() {
    for (rect, size, indices) in [
        (crop(0.9, 0.9, 1.9, 1.9), (2, 2), vec![0, 1, 4, 5]),
        (crop(-1.2, -0.4, 3.9, 2.9), (2, 2), vec![0, 1, 4, 5]),
        (crop(2.7, 1.4, 20.0, 20.0), (2, 2), vec![6, 7, 10, 11]),
        (crop(-10.0, -10.0, 100.0, 100.0), (4, 3), (0..12).collect()),
        (crop(0.0, 0.0, 1.0, 1.0), (1, 1), vec![0]),
    ] {
        planes(&scene(FitMode::Stretch, Some(rect), None), size, &indices);
    }
}

#[test]
fn empty_source_crop_emits_no_image_or_clip() {
    for rect in [
        crop(1.0, 1.0, 0.0, 1.0),
        crop(1.0, 1.0, 1.0, 0.0),
        crop(2.0, 1.0, -1.0, 1.0),
        crop(4.0, 0.0, 1.0, 1.0),
        crop(0.0, 3.0, 1.0, 1.0),
        crop(-3.0, 0.0, 1.0, 1.0),
        crop(1.1, 1.1, 0.2, 0.2),
    ] {
        let fonts = default_provider();
        let plan = build_plan(&Default::default(), &fonts, true);
        let (content, resources) = translate(
            &scene(FitMode::Cover, Some(rect), Some(ImageClip::Ellipse)),
            &fonts,
            &assets(),
            &plan,
        );
        assert!(resources.images.is_empty());
        let content = String::from_utf8(content.finish().into_vec()).unwrap();
        assert!(!content.contains(" Do"));
        assert!(!content.contains("W"));
    }
}

#[test]
fn source_crop_keeps_shape_clips() {
    for clip in [ImageClip::Ellipse, ImageClip::RoundedRect { radius: 4.0 }] {
        let content = planes(
            &scene(FitMode::Contain, Some(crop(1.0, 1.0, 2.0, 1.0)), Some(clip)),
            (2, 1),
            &[5, 6],
        );
        assert!(content.contains(" c\n"));
        assert!(content.contains("W\nn"));
        assert!(content.contains("/im0 Do"));
    }
}

#[test]
fn svg_source_crop_keeps_pdf_and_raster_bytes() {
    let mut assets = BytesAssetProvider::new();
    assets.register("asset.image", AssetKind::Svg, Arc::from(b"<svg xmlns='http://www.w3.org/2000/svg' width='4' height='3'><rect width='4' height='3' fill='red'/></svg>".as_slice()));
    let fonts = default_provider();
    let full = scene(FitMode::Contain, None, None);
    let cropped = scene(FitMode::Contain, Some(crop(1.0, 1.0, 0.0, 0.0)), None);
    assert_eq!(
        render_pdf(&full, &fonts, &assets),
        render_pdf(&cropped, &fonts, &assets)
    );
    let backend = TinySkiaBackend;
    assert_eq!(
        backend.rasterize(&full, &fonts, &assets).unwrap().rgba,
        backend.rasterize(&cropped, &fonts, &assets).unwrap().rgba
    );
}

#[test]
fn absent_source_crop_retains_pdf_and_raster_bytes() {
    let scene = scene(FitMode::Contain, None, None);
    planes(&scene, (4, 3), &(0..12).collect::<Vec<_>>());
    let fonts = default_provider();
    let assets = assets();
    let pdf = render_pdf(&scene, &fonts, &assets);
    let raster = TinySkiaBackend.rasterize(&scene, &fonts, &assets).unwrap();
    let fingerprint = |bytes: &[u8]| {
        bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        })
    };
    assert_eq!(pdf.len(), 1113);
    assert_eq!(fingerprint(&pdf), 0xc65e08e7b274929b);
    assert_eq!(fingerprint(&raster.rgba), 0x33037804bbbf17ee);
}

#[test]
fn raster_source_crop_retains_pixels() {
    let fonts = default_provider();
    let assets = assets();
    for (fit, expected) in [
        (FitMode::Stretch, 0xe2c1738d9f108cde),
        (FitMode::Contain, 0xe2c1738d9f108cde),
        (FitMode::Cover, 0xe2c1738d9f108cde),
        (FitMode::None, 0xc3b391434b69f1c5),
    ] {
        let scene = scene(
            fit,
            Some(crop(0.9, 0.9, 1.9, 1.9)),
            Some(ImageClip::RoundedRect { radius: 4.0 }),
        );
        let raster = TinySkiaBackend.rasterize(&scene, &fonts, &assets).unwrap();
        let hash = raster
            .rgba
            .iter()
            .fold(0xcbf29ce484222325u64, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
            });
        assert_eq!(hash, expected, "{fit:?}");
    }
}
