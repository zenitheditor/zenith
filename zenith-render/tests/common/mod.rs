//! Shared helpers for zenith-render integration tests.
//!
//! Every helper that was in the old inline `#[cfg(test)] mod tests` block in
//! `tiny_skia/tests.rs` and is not itself a `#[test]` lives here.
//!
//! `tests/common/mod.rs` is compiled into EVERY integration-test binary, but
//! each binary only exercises a subset of these helpers — so the unused ones
//! trip `dead_code`/`unused_imports` in the binaries that don't call them. This
//! is the canonical shared-test-helper situation (see the Rust book, "Submodules
//! in Integration Tests"): the helpers are genuinely used across the suite, so
//! the per-binary false positives are suppressed here rather than fragmenting the
//! helpers across files.
#![allow(dead_code, unused_imports)]

use std::sync::Arc;

pub use zenith_core::{
    AssetKind, BytesAssetProvider, BytesFontProvider, FontStyle, default_provider,
};
pub use zenith_layout::{RustybuzzEngine, ShapeRequest, TextDirection, TextLayoutEngine};
pub use zenith_render::{RasterBackend, RasterImage, TinySkiaBackend, render_image, render_png};
pub use zenith_scene::{
    BlendMode, Color, FitMode, GradientPaint, GradientStop, ImageClip, Scene, SceneCommand,
    SceneGlyph, ShadowSpec, SrcRect, StrokeAlign,
};

/// A shared empty asset provider for tests that draw no images.
pub fn no_assets() -> BytesAssetProvider {
    BytesAssetProvider::new()
}

pub fn red() -> Color {
    Color::srgb(255, 0, 0, 255)
}

pub fn make_solid_red_scene(page: f64) -> Scene {
    let mut s = Scene::new(page, page);
    s.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: page,
        h: page,
    });
    s.commands.push(SceneCommand::FillRect {
        x: 0.0,
        y: 0.0,
        w: page,
        h: page,
        color: red(),
    });
    s.commands.push(SceneCommand::PopClip);
    s
}

/// Index into a straight-alpha RGBA8 buffer for pixel (px, py) in an image
/// of the given `width`.
pub fn pixel(rgba: &[u8], width: u32, px: u32, py: u32) -> (u8, u8, u8, u8) {
    let base = ((py * width + px) * 4) as usize;
    (rgba[base], rgba[base + 1], rgba[base + 2], rgba[base + 3])
}

/// The committed 2×2 RGBA test PNG.
pub const SWATCH_PNG: &[u8] = include_bytes!("../../../examples/assets/swatch.png");

pub fn swatch_provider() -> BytesAssetProvider {
    let mut p = BytesAssetProvider::new();
    p.register("asset.swatch", AssetKind::Image, Arc::from(SWATCH_PNG));
    p
}

/// Build a scene that draws the swatch stretched into a box, clipped to it.
pub fn swatch_scene() -> Scene {
    let mut scene = Scene::new(40.0, 40.0);
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 40.0,
        h: 40.0,
    });
    scene.commands.push(SceneCommand::PushClip {
        x: 8.0,
        y: 8.0,
        w: 24.0,
        h: 24.0,
    });
    scene.commands.push(SceneCommand::DrawImage {
        x: 8.0,
        y: 8.0,
        w: 24.0,
        h: 24.0,
        asset_id: "asset.swatch".to_string(),
        fit: FitMode::Stretch,
        pos_x: 50.0,
        pos_y: 50.0,
        opacity: 1.0,
        clip_shape: None,
        src_rect: None,
    });
    scene.commands.push(SceneCommand::PopClip);
    scene.commands.push(SceneCommand::PopClip);
    scene
}

/// Build a scene that draws the swatch stretched to fill the whole page,
/// clipped to the inscribed ellipse (circle, since the box is square).
pub fn swatch_ellipse_scene() -> Scene {
    let mut scene = Scene::new(40.0, 40.0);
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 40.0,
        h: 40.0,
    });
    // Box-clip the compiler always emits before DrawImage.
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 40.0,
        h: 40.0,
    });
    scene.commands.push(SceneCommand::DrawImage {
        x: 0.0,
        y: 0.0,
        w: 40.0,
        h: 40.0,
        asset_id: "asset.swatch".to_string(),
        fit: FitMode::Stretch,
        pos_x: 50.0,
        pos_y: 50.0,
        opacity: 1.0,
        clip_shape: Some(ImageClip::Ellipse),
        src_rect: None,
    });
    scene.commands.push(SceneCommand::PopClip);
    scene.commands.push(SceneCommand::PopClip);
    scene
}

/// Build a 3×3 PNG whose columns are pure RED (x=0), GREEN (x=1), BLUE (x=2).
///
/// Uses tiny-skia to compose the image rather than pulling in the `image`
/// crate or embedding a hand-crafted PNG byte string.
pub fn three_column_rgb_png() -> Arc<[u8]> {
    use tiny_skia::{Pixmap, PremultipliedColorU8};

    let mut pm = Pixmap::new(3, 3).expect("3x3 pixmap");
    // Fill each pixel using PremultipliedColorU8 (alpha=255 → premult == straight).
    let pixels = pm.pixels_mut();
    for row in 0..3_usize {
        for col in 0..3_usize {
            let idx = row * 3 + col;
            pixels[idx] = match col {
                0 => PremultipliedColorU8::from_rgba(255, 0, 0, 255).expect("red"),
                1 => PremultipliedColorU8::from_rgba(0, 255, 0, 255).expect("green"),
                _ => PremultipliedColorU8::from_rgba(0, 0, 255, 255).expect("blue"),
            };
        }
    }
    let png = pm.encode_png().expect("PNG encode must succeed");
    Arc::from(png.as_slice())
}

pub fn backend_render(scene: &Scene, provider: &BytesFontProvider) -> Vec<u8> {
    let backend = TinySkiaBackend;
    backend
        .rasterize(scene, provider, &no_assets())
        .expect("rasterize must succeed")
        .rgba
}
