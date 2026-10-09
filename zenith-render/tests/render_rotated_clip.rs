//! A `PushClip` under a rotation clips to the rotated quad, not to its
//! axis-aligned bounding box. Axis-aligned clips keep the exact rect path.

mod common;
use common::pixel;
#[path = "common/no_assets.rs"]
mod no_assets;
use no_assets::no_assets;
#[path = "common/red.rs"]
mod red;
use red::red;
#[path = "common/swatch_png.rs"]
mod swatch_png;
#[path = "common/swatch_provider.rs"]
mod swatch_provider;
use swatch_provider::swatch_provider;
use zenith_core::default_provider;
use zenith_render::{render_image, render_png};
use zenith_scene::{FitMode, Paint, Scene, SceneCommand};

/// A 100×100 transparent page with a 40×20 box centered at (50, 50), rotated
/// 45° about its center, holding the swatch with `fit="cover"` (2×2 source
/// into 40×20 → a 40×40 overflowing square), clipped to the box — the scene
/// shape the compiler emits for a rotated cover image.
fn rotated_cover_scene() -> Scene {
    let mut s = Scene::new(100.0, 100.0);
    s.commands.push(SceneCommand::PushTransform {
        angle_deg: 45.0,
        cx: 50.0,
        cy: 50.0,
    });
    s.commands.push(SceneCommand::PushClip {
        x: 30.0,
        y: 40.0,
        w: 40.0,
        h: 20.0,
    });
    s.commands.push(SceneCommand::DrawImage {
        x: 30.0,
        y: 40.0,
        w: 40.0,
        h: 20.0,
        asset_id: "asset.swatch".to_string(),
        fit: FitMode::Cover,
        pos_x: 50.0,
        pos_y: 50.0,
        opacity: 1.0,
        clip_shape: None,
        src_rect: None,
        svg_style: None,
    });
    s.commands.push(SceneCommand::PopClip);
    s.commands.push(SceneCommand::PopTransform);
    s
}

/// Device point of the box-local offset `(u, v)` from the center, under the
/// 45° rotation (local x → (c, s), local y → (−s, c)).
fn rotated(u: f64, v: f64) -> (u32, u32) {
    let c = std::f64::consts::FRAC_1_SQRT_2;
    let x = 50.0 + u * c - v * c;
    let y = 50.0 + u * c + v * c;
    (x.round() as u32, y.round() as u32)
}

#[test]
fn rotated_cover_image_clips_to_the_rotated_quad() {
    let img = render_image(
        &rotated_cover_scene(),
        &default_provider(),
        &swatch_provider(),
    )
    .expect("render");
    let alpha = |(x, y): (u32, u32)| pixel(&img.rgba, img.width, x, y).3;

    // Inside the quad: center and just inside each corner.
    assert_eq!(alpha((50, 50)), 255, "center painted");
    for (u, v) in [(17.0, 7.0), (-17.0, 7.0), (17.0, -7.0), (-17.0, -7.0)] {
        assert_eq!(alpha(rotated(u, v)), 255, "just inside corner ({u}, {v})");
    }
    // Inside the bounding box (±21.2 px) and inside the overflowing 40×40
    // cover square, but outside the 40×20 quad along the short axis.
    for (u, v) in [(0.0, 15.0), (0.0, -15.0), (8.0, 14.0), (-8.0, -14.0)] {
        assert_eq!(alpha(rotated(u, v)), 0, "outside the quad ({u}, {v})");
    }
}

#[test]
fn rotated_clip_is_deterministic() {
    let a = render_png(
        &rotated_cover_scene(),
        &default_provider(),
        &swatch_provider(),
    )
    .expect("a");
    let b = render_png(
        &rotated_cover_scene(),
        &default_provider(),
        &swatch_provider(),
    )
    .expect("b");
    assert_eq!(a, b);
}

/// A fill bigger than its clip under a scale-only transform keeps the exact
/// AA-off rect clip: hard edges, no partial coverage.
#[test]
fn axis_aligned_clip_under_scale_keeps_hard_edges() {
    let mut s = Scene::new(40.0, 40.0);
    s.commands.push(SceneCommand::PushScaleTranslate {
        sx: 2.0,
        sy: 2.0,
        tx: 0.0,
        ty: 0.0,
    });
    s.commands.push(SceneCommand::PushClip {
        x: 5.0,
        y: 5.0,
        w: 5.0,
        h: 5.0,
    });
    s.commands.push(SceneCommand::FillRect {
        x: 0.0,
        y: 0.0,
        w: 20.0,
        h: 20.0,
        paint: Paint::solid(red()),
    });
    s.commands.push(SceneCommand::PopClip);
    s.commands.push(SceneCommand::PopTransform);
    let img = render_image(&s, &default_provider(), &no_assets()).expect("render");
    for y in 0..40 {
        for x in 0..40 {
            let a = pixel(&img.rgba, img.width, x, y).3;
            let inside = (10..20).contains(&x) && (10..20).contains(&y);
            assert_eq!(a, if inside { 255 } else { 0 }, "pixel ({x}, {y})");
        }
    }
}
