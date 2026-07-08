//! Integration-style DrawImage rasterization tests via the tiny-skia backend.
//! Exercises the public `TinySkiaBackend` directly with raster + SVG assets.

use std::sync::Arc;

mod common;
use common::*;
use zenith_raster::{LinearRgba, blend_pixel, decode_srgb_u8, encode_linear_to_srgb_u8};
use zenith_scene::SvgStyle;

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
        svg_style: None,
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

#[test]
fn draw_image_inside_multiply_layer_uses_raster_blend_math() {
    use tiny_skia::{Pixmap, PremultipliedColorU8};

    let mut pm = Pixmap::new(1, 1).expect("1x1 pixmap");
    let source = (64, 100, 220);
    pm.pixels_mut()[0] =
        PremultipliedColorU8::from_rgba(source.0, source.1, source.2, 255).expect("source pixel");
    let png = pm.encode_png().expect("PNG encode must succeed");

    let mut assets = BytesAssetProvider::new();
    assets.register(
        "asset.layer_source",
        AssetKind::Image,
        Arc::from(png.as_slice()),
    );

    let backdrop = (128, 200, 40);
    let mut scene = Scene::new(1.0, 1.0);
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
    });
    scene.commands.push(SceneCommand::FillRect {
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
        paint: Paint::solid(Color::srgb(backdrop.0, backdrop.1, backdrop.2, 255)),
    });
    let layer_opacity = 0.5;
    scene.commands.push(SceneCommand::PushLayer {
        opacity: layer_opacity,
        blend_mode: Some(BlendMode::Multiply),
    });
    scene.commands.push(SceneCommand::DrawImage {
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
        asset_id: "asset.layer_source".to_string(),
        fit: FitMode::Stretch,
        pos_x: 50.0,
        pos_y: 50.0,
        opacity: 1.0,
        clip_shape: None,
        src_rect: None,
        svg_style: None,
    });
    scene.commands.push(SceneCommand::PopLayer);
    scene.commands.push(SceneCommand::PopClip);

    let backend = TinySkiaBackend;
    let fonts = default_provider();
    let img = backend
        .rasterize(&scene, &fonts, &assets)
        .expect("rasterize must succeed");

    let expected = blend_pixel(
        BlendMode::Multiply,
        opaque_linear(backdrop),
        scaled_alpha(opaque_linear(source), layer_opacity),
    )
    .expect("blend must succeed");
    assert_eq!(
        pixel(&img.rgba, img.width, 0, 0),
        straight_srgb_pixel(expected)
    );
}

#[test]
fn draw_image_multiply_layer_preserves_transparent_layer_area() {
    use tiny_skia::{Pixmap, PremultipliedColorU8};

    let mut pm = Pixmap::new(1, 1).expect("1x1 pixmap");
    pm.pixels_mut()[0] = PremultipliedColorU8::from_rgba(20, 180, 220, 255).expect("source pixel");
    let png = pm.encode_png().expect("PNG encode must succeed");

    let mut assets = BytesAssetProvider::new();
    assets.register(
        "asset.layer_source",
        AssetKind::Image,
        Arc::from(png.as_slice()),
    );

    let backdrop = Color::srgb(128, 32, 200, 128);
    let mut base_scene = Scene::new(2.0, 1.0);
    base_scene.commands.push(SceneCommand::FillRect {
        x: 0.0,
        y: 0.0,
        w: 2.0,
        h: 1.0,
        paint: Paint::solid(backdrop),
    });

    let mut blended_scene = base_scene.clone();
    blended_scene.commands.push(SceneCommand::PushLayer {
        opacity: 1.0,
        blend_mode: Some(BlendMode::Multiply),
    });
    blended_scene.commands.push(SceneCommand::DrawImage {
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
        asset_id: "asset.layer_source".to_string(),
        fit: FitMode::Stretch,
        pos_x: 50.0,
        pos_y: 50.0,
        opacity: 1.0,
        clip_shape: None,
        src_rect: None,
        svg_style: None,
    });
    blended_scene.commands.push(SceneCommand::PopLayer);

    let backend = TinySkiaBackend;
    let fonts = default_provider();
    let base = backend
        .rasterize(&base_scene, &fonts, &assets)
        .expect("base rasterize must succeed");
    let blended = backend
        .rasterize(&blended_scene, &fonts, &assets)
        .expect("blended rasterize must succeed");

    assert_ne!(
        pixel(&base.rgba, base.width, 0, 0),
        pixel(&blended.rgba, blended.width, 0, 0)
    );
    assert_eq!(
        pixel(&base.rgba, base.width, 1, 0),
        pixel(&blended.rgba, blended.width, 1, 0),
        "transparent layer pixels must preserve the existing backdrop bytes"
    );
}

#[test]
fn draw_image_source_over_layers_match_direct_draw_bytes() {
    use tiny_skia::{Pixmap, PremultipliedColorU8};

    let mut pm = Pixmap::new(1, 1).expect("1x1 pixmap");
    pm.pixels_mut()[0] = PremultipliedColorU8::from_rgba(210, 40, 90, 255).expect("source pixel");
    let png = pm.encode_png().expect("PNG encode must succeed");

    let mut assets = BytesAssetProvider::new();
    assets.register(
        "asset.layer_source",
        AssetKind::Image,
        Arc::from(png.as_slice()),
    );

    let direct = image_over_backdrop_scene(None);
    let implicit_source_over = image_over_backdrop_scene(Some(None));
    let explicit_normal = image_over_backdrop_scene(Some(Some(BlendMode::Normal)));

    let backend = TinySkiaBackend;
    let fonts = default_provider();
    let direct_img = backend
        .rasterize(&direct, &fonts, &assets)
        .expect("direct rasterize must succeed");
    let implicit_img = backend
        .rasterize(&implicit_source_over, &fonts, &assets)
        .expect("implicit source-over layer rasterize must succeed");
    let normal_img = backend
        .rasterize(&explicit_normal, &fonts, &assets)
        .expect("normal layer rasterize must succeed");

    assert_eq!(
        direct_img.rgba, implicit_img.rgba,
        "blend_mode=None layer must stay byte-identical to direct drawing"
    );
    assert_eq!(
        direct_img.rgba, normal_img.rgba,
        "blend_mode=Normal layer must stay byte-identical to direct drawing"
    );
}

fn image_over_backdrop_scene(layer: Option<Option<BlendMode>>) -> Scene {
    let mut scene = Scene::new(1.0, 1.0);
    scene.commands.push(SceneCommand::FillRect {
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
        paint: Paint::solid(Color::srgb(30, 120, 200, 255)),
    });
    if let Some(blend_mode) = layer {
        scene.commands.push(SceneCommand::PushLayer {
            opacity: 1.0,
            blend_mode,
        });
    }
    scene.commands.push(SceneCommand::DrawImage {
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
        asset_id: "asset.layer_source".to_string(),
        fit: FitMode::Stretch,
        pos_x: 50.0,
        pos_y: 50.0,
        opacity: 1.0,
        clip_shape: None,
        src_rect: None,
        svg_style: None,
    });
    if layer.is_some() {
        scene.commands.push(SceneCommand::PopLayer);
    }
    scene
}

fn opaque_linear(rgb: (u8, u8, u8)) -> LinearRgba {
    LinearRgba::straight(
        decode_srgb_u8(rgb.0),
        decode_srgb_u8(rgb.1),
        decode_srgb_u8(rgb.2),
        1.0,
    )
    .expect("opaque linear pixel")
}

fn scaled_alpha(pixel: LinearRgba, opacity: f64) -> LinearRgba {
    let opacity = opacity as f32;
    LinearRgba::premultiplied(
        pixel.r() * opacity,
        pixel.g() * opacity,
        pixel.b() * opacity,
        pixel.a() * opacity,
    )
    .expect("scaled linear pixel")
}

fn straight_srgb_pixel(pixel: LinearRgba) -> (u8, u8, u8, u8) {
    let alpha = pixel.a();
    if alpha <= 0.0 {
        return (0, 0, 0, 0);
    }

    (
        encode_linear_to_srgb_u8(pixel.r() / alpha),
        encode_linear_to_srgb_u8(pixel.g() / alpha),
        encode_linear_to_srgb_u8(pixel.b() / alpha),
        quantize_unit_to_u8(alpha),
    )
}

fn quantize_unit_to_u8(channel: f32) -> u8 {
    let scaled = channel.clamp(0.0, 1.0) * 255.0;
    let lower = scaled.floor();
    let fraction = scaled - lower;
    let lower_int = lower as u16;

    let rounded = if fraction < 0.5 {
        lower_int
    } else if fraction > 0.5 {
        lower_int + 1
    } else if lower_int % 2 == 0 {
        lower_int
    } else {
        lower_int + 1
    };

    if rounded >= 255 { 255 } else { rounded as u8 }
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
        svg_style: None,
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

#[test]
fn draw_image_svg_style_recolors_current_color_stroke() {
    const STROKE_SVG: &[u8] = b"<svg xmlns='http://www.w3.org/2000/svg' \
        width='10' height='10' viewBox='0 0 10 10' fill='none' \
        stroke='currentColor' stroke-width='10'>\
        <path d='M5 0 L5 10'/>\
        </svg>";

    let mut assets = BytesAssetProvider::new();
    assets.register("asset.stroke", AssetKind::Svg, Arc::from(STROKE_SVG));

    let render_with = |stroke: Color| {
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
            asset_id: "asset.stroke".to_string(),
            fit: FitMode::Stretch,
            pos_x: 50.0,
            pos_y: 50.0,
            opacity: 1.0,
            clip_shape: None,
            src_rect: None,
            svg_style: Some(SvgStyle {
                stroke: Some(stroke),
                fill: None,
                stroke_width: None,
            }),
        });
        scene.commands.push(SceneCommand::PopClip);
        TinySkiaBackend
            .rasterize(&scene, &default_provider(), &assets)
            .expect("styled SVG rasterize")
    };

    let red = render_with(Color::srgb(255, 0, 0, 255));
    let blue = render_with(Color::srgb(0, 0, 255, 255));
    let (rr, rg, rb, ra) = pixel(&red.rgba, red.width, 5, 5);
    let (br, bg, bb, ba) = pixel(&blue.rgba, blue.width, 5, 5);
    assert!(
        ra > 0 && rr > rg && rr > rb,
        "red pixel: {rr},{rg},{rb},{ra}"
    );
    assert!(
        ba > 0 && bb > br && bb > bg,
        "blue pixel: {br},{bg},{bb},{ba}"
    );
    assert_ne!(red.rgba, blue.rgba, "style color must change raster output");
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
        svg_style: None,
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
        svg_style: None,
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
