//! Integration-style DrawImage rasterization tests via the tiny-skia backend.
//! Exercises the public `TinySkiaBackend` directly with raster + SVG assets.

use std::sync::Arc;

mod common;
use common::*;

// ── image: stretch renders + determinism ──────────────────────────────

#[test]
fn draw_image_stretch_renders() {
    let backend = TinySkiaBackend;
    let fonts = default_provider();
    let assets = swatch_provider();
    let scene = swatch_scene();

    let img1 = backend
        .rasterize(&scene, &fonts, &assets)
        .expect("rasterize 1");
    let img2 = backend
        .rasterize(&scene, &fonts, &assets)
        .expect("rasterize 2");

    // (i) determinism: byte-identical pixels across two rasterizes.
    assert_eq!(
        img1.rgba, img2.rgba,
        "two rasterizes of the same image scene must be byte-identical"
    );

    // (ii) at least one pixel inside the box is non-transparent.
    let any_ink = (0..img1.height).any(|py| {
        (0..img1.width).any(|px| {
            let (_, _, _, a) = pixel(&img1.rgba, img1.width, px, py);
            a > 0
        })
    });
    assert!(
        any_ink,
        "DrawImage stretch must rasterize at least one non-transparent pixel"
    );
}

// ── image clip="ellipse": clip takes effect + determinism ─────────────

#[test]
fn draw_image_ellipse_clip_takes_effect() {
    let backend = TinySkiaBackend;
    let fonts = default_provider();
    let assets = swatch_provider();
    let scene = swatch_ellipse_scene();

    let img1 = backend
        .rasterize(&scene, &fonts, &assets)
        .expect("rasterize 1");
    let img2 = backend
        .rasterize(&scene, &fonts, &assets)
        .expect("rasterize 2");

    // (i) determinism: byte-identical across two rasterizes.
    assert_eq!(
        img1.rgba, img2.rgba,
        "two rasterizes of the ellipse-clipped scene must be byte-identical"
    );

    // (ii) the center pixel (inside the inscribed ellipse) is painted, while a
    // corner pixel (outside the ellipse) is fully transparent — proving the
    // ellipse clip mask took effect (a plain box-clip would paint the corner).
    let (_, _, _, center_a) = pixel(&img1.rgba, img1.width, 20, 20);
    let (_, _, _, corner_a) = pixel(&img1.rgba, img1.width, 0, 0);
    assert!(
        center_a > 0,
        "center pixel must be painted inside the ellipse clip"
    );
    assert_eq!(
        corner_a, 0,
        "corner pixel must be clipped out by the ellipse mask"
    );
}

#[test]
fn draw_image_missing_asset_is_skipped() {
    let backend = TinySkiaBackend;
    let fonts = default_provider();
    // Empty provider: the asset id is not registered.
    let assets = BytesAssetProvider::new();

    let mut scene = Scene::new(20.0, 20.0);
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 20.0,
        h: 20.0,
    });
    scene.commands.push(SceneCommand::DrawImage {
        x: 0.0,
        y: 0.0,
        w: 20.0,
        h: 20.0,
        asset_id: "asset.missing".to_string(),
        fit: FitMode::Stretch,
        pos_x: 50.0,
        pos_y: 50.0,
        opacity: 1.0,
        clip_shape: None,
        src_rect: None,
    });
    scene.commands.push(SceneCommand::PopClip);

    // Must not panic; renders without any image pixels.
    let img = backend
        .rasterize(&scene, &fonts, &assets)
        .expect("rasterize must succeed even with a missing asset");
    let any_opaque = (0..img.height).any(|py| {
        (0..img.width).any(|px| {
            let (_, _, _, a) = pixel(&img.rgba, img.width, px, py);
            a > 0
        })
    });
    assert!(
        !any_opaque,
        "no pixels should be drawn when the asset is missing"
    );
}

// ── SVG asset: rasterizes and draws red pixels ────────────────────────

/// An inline 10×10 SVG filled solid red is registered as `AssetKind::Svg`,
/// drawn stretched into a 10×10 box on a 10×10 page, and the center pixel
/// must be red (proving the SVG was rasterized and composited).
#[test]
fn draw_image_svg_asset_renders_red_pixels() {
    const RED_SVG: &[u8] = b"<svg xmlns='http://www.w3.org/2000/svg' \
        width='10' height='10'>\
        <rect width='10' height='10' fill='#ff0000'/>\
        </svg>";

    let mut assets = BytesAssetProvider::new();
    assets.register("asset.red", AssetKind::Svg, Arc::from(RED_SVG));

    let mut scene = Scene::new(10.0, 10.0);
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 10.0,
        h: 10.0,
    });
    scene.commands.push(SceneCommand::DrawImage {
        x: 0.0,
        y: 0.0,
        w: 10.0,
        h: 10.0,
        asset_id: "asset.red".to_string(),
        fit: FitMode::Stretch,
        pos_x: 50.0,
        pos_y: 50.0,
        opacity: 1.0,
        clip_shape: None,
        src_rect: None,
    });
    scene.commands.push(SceneCommand::PopClip);

    let backend = TinySkiaBackend;
    let fonts = default_provider();
    let img = backend
        .rasterize(&scene, &fonts, &assets)
        .expect("SVG rasterize must succeed");

    // Center pixel must be red (r dominant, a > 0).
    let (r, g, b, a) = pixel(&img.rgba, img.width, 5, 5);
    assert!(a > 0, "center pixel must be opaque after SVG rasterize");
    assert!(
        r > g && r > b,
        "center pixel must be red-dominant; got r={r} g={g} b={b}"
    );
}

// ── SVG <text>: text element converts to paths and rasterizes ─────────

/// An inline SVG containing a red `<text>` element is registered as
/// `AssetKind::Svg`, drawn via DrawImage, and the output pixmap must contain
/// at least one RED pixel — proving the text was converted to paths and
/// rasterized (not silently dropped due to an empty fontdb).
#[test]
fn draw_image_svg_text_renders_red_pixels() {
    const TEXT_SVG: &[u8] = b"<svg xmlns='http://www.w3.org/2000/svg' \
        width='200' height='60'>\
        <text x='0' y='40' font-size='40' fill='#ff0000'>Hi</text>\
        </svg>";

    let mut assets = BytesAssetProvider::new();
    assets.register("asset.text_svg", AssetKind::Svg, Arc::from(TEXT_SVG));

    let mut scene = Scene::new(200.0, 60.0);
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 200.0,
        h: 60.0,
    });
    scene.commands.push(SceneCommand::DrawImage {
        x: 0.0,
        y: 0.0,
        w: 200.0,
        h: 60.0,
        asset_id: "asset.text_svg".to_string(),
        fit: FitMode::Stretch,
        pos_x: 50.0,
        pos_y: 50.0,
        opacity: 1.0,
        clip_shape: None,
        src_rect: None,
    });
    scene.commands.push(SceneCommand::PopClip);

    let backend = TinySkiaBackend;
    let fonts = default_provider();
    let img = backend
        .rasterize(&scene, &fonts, &assets)
        .expect("SVG text rasterize must succeed");

    // At least one pixel must be red-dominant — text paths were rasterized.
    let any_red = (0..img.height).any(|py| {
        (0..img.width).any(|px| {
            let (r, g, b, a) = pixel(&img.rgba, img.width, px, py);
            a > 0 && r > g && r > b
        })
    });
    assert!(
        any_red,
        "SVG <text> must produce at least one red pixel after convert_text + rasterize"
    );
}

// ── src-rect raster crop ──────────────────────────────────────────────────

/// A DrawImage with `src_rect=Some(SrcRect{x:2,y:0,w:1,h:3})` selecting the
/// BLUE column of a red/green/blue 3×3 image, stretched into a 4×4 box, must
/// render entirely blue pixels (the crop replaces the source).
#[test]
fn draw_image_src_rect_crops_to_blue_column() {
    let png = three_column_rgb_png();
    let mut assets = BytesAssetProvider::new();
    assets.register("asset.cols", AssetKind::Image, png);

    // 4×4 canvas; the image fills the entire page.
    let mut scene = Scene::new(4.0, 4.0);
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 4.0,
        h: 4.0,
    });
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 4.0,
        h: 4.0,
    });
    scene.commands.push(SceneCommand::DrawImage {
        x: 0.0,
        y: 0.0,
        w: 4.0,
        h: 4.0,
        asset_id: "asset.cols".to_string(),
        fit: FitMode::Stretch,
        pos_x: 50.0,
        pos_y: 50.0,
        opacity: 1.0,
        clip_shape: None,
        // Crop to the blue column (x=2, w=1, full height).
        src_rect: Some(SrcRect {
            x: 2.0,
            y: 0.0,
            w: 1.0,
            h: 3.0,
        }),
    });
    scene.commands.push(SceneCommand::PopClip);
    scene.commands.push(SceneCommand::PopClip);

    let backend = TinySkiaBackend;
    let fonts = default_provider();
    let img = backend
        .rasterize(&scene, &fonts, &assets)
        .expect("rasterize must succeed");

    // Every pixel in the 4×4 output must be blue-dominant and opaque.
    for py in 0..img.height {
        for px_x in 0..img.width {
            let (r, g, b, a) = pixel(&img.rgba, img.width, px_x, py);
            assert!(a > 0, "pixel ({px_x},{py}) must be opaque (alpha={a})");
            assert!(
                b > r && b > g,
                "pixel ({px_x},{py}) must be blue-dominant after src-rect crop; got r={r} g={g} b={b}"
            );
        }
    }
}
