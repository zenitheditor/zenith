//! Raster-time output scale: identity at 1, exact sizes, scaled effects.

mod common;
use common::pixel;
#[path = "common/no_assets.rs"]
mod no_assets;
use no_assets::no_assets;
#[path = "common/red.rs"]
mod red;
use red::red;
use zenith_core::default_provider;
use zenith_render::{RasterImage, render_image_scaled, render_png, render_png_scaled, scaled_size};
use zenith_scene::{Color, Paint, Scene, SceneCommand, ShadowSpec};

/// A page with a solid backdrop, a shadowed square, a blurred circle, a
/// stroked line, and a clipped rect.
fn effects_scene() -> Scene {
    let mut s = Scene::new(400.0, 300.0);
    s.commands.push(SceneCommand::FillRect {
        x: 0.0,
        y: 0.0,
        w: 400.0,
        h: 300.0,
        paint: Paint::solid(Color::srgb(255, 255, 255, 255)),
    });
    s.commands.push(SceneCommand::BeginShadow {
        shadows: vec![ShadowSpec {
            dx: 0.0,
            dy: 0.0,
            blur: 12.0,
            color: Color::srgb(0, 0, 0, 255),
        }],
    });
    s.commands.push(SceneCommand::FillRect {
        x: 100.0,
        y: 100.0,
        w: 100.0,
        h: 100.0,
        paint: Paint::solid(Color::srgb(0, 0, 255, 255)),
    });
    s.commands.push(SceneCommand::EndShadow);
    s.commands.push(SceneCommand::BeginBlur { radius: 6.0 });
    s.commands.push(SceneCommand::FillEllipse {
        x: 260.0,
        y: 40.0,
        w: 80.0,
        h: 80.0,
        rx: None,
        ry: None,
        paint: Paint::solid(red()),
    });
    s.commands.push(SceneCommand::EndBlur);
    s.commands.push(SceneCommand::PushClip {
        x: 250.0,
        y: 200.0,
        w: 100.0,
        h: 50.0,
    });
    s.commands.push(SceneCommand::FillRect {
        x: 230.0,
        y: 180.0,
        w: 200.0,
        h: 200.0,
        paint: Paint::solid(Color::srgb(0, 128, 0, 255)),
    });
    s.commands.push(SceneCommand::PopClip);
    s
}

#[test]
fn scale_one_is_byte_identical_to_unscaled() {
    let fonts = default_provider();
    let scene = effects_scene();
    let base = render_png(&scene, &fonts, &no_assets()).expect("render");
    let scaled = render_png_scaled(&scene, 1.0, &fonts, &no_assets()).expect("render at 1");
    assert_eq!(base, scaled);
}

#[test]
fn half_scale_has_exact_size_and_is_deterministic() {
    let fonts = default_provider();
    let scene = effects_scene();
    let a = render_image_scaled(&scene, 0.5, &fonts, &no_assets()).expect("render a");
    let b = render_image_scaled(&scene, 0.5, &fonts, &no_assets()).expect("render b");
    assert_eq!((a.width, a.height), (200, 150));
    assert_eq!(a.rgba, b.rgba);
    let pa = render_png_scaled(&scene, 0.5, &fonts, &no_assets()).expect("png a");
    let pb = render_png_scaled(&scene, 0.5, &fonts, &no_assets()).expect("png b");
    assert_eq!(pa, pb);
}

#[test]
fn size_rounds_half_away_from_zero() {
    assert_eq!(scaled_size(401.0, 301.0, 0.5).expect("size"), (201, 151));
    assert_eq!(scaled_size(1920.0, 1080.0, 0.25).expect("size"), (480, 270));
    let mut s = Scene::new(401.0, 301.0);
    s.commands.push(SceneCommand::FillRect {
        x: 0.0,
        y: 0.0,
        w: 401.0,
        h: 301.0,
        paint: Paint::solid(red()),
    });
    let img = render_image_scaled(&s, 0.5, &default_provider(), &no_assets()).expect("render");
    assert_eq!((img.width, img.height), (201, 151));
}

#[test]
fn invalid_scale_is_an_error() {
    let fonts = default_provider();
    let scene = effects_scene();
    for s in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(
            render_image_scaled(&scene, s, &fonts, &no_assets()).is_err(),
            "scale {s} must fail"
        );
    }
}

/// Distance in pixels from `x0` rightward along row `y` until the red channel
/// (white backdrop darkened by the black shadow) climbs back above 250.
fn shadow_reach(img: &RasterImage, x0: u32, y: u32) -> u32 {
    (x0..img.width)
        .find(|&x| pixel(&img.rgba, img.width, x, y).0 > 250)
        .map_or(img.width - x0, |x| x - x0)
}

#[test]
fn half_scale_shadow_blur_is_proportional() {
    let fonts = default_provider();
    let scene = effects_scene();
    let full = render_image_scaled(&scene, 1.0, &fonts, &no_assets()).expect("full");
    let half = render_image_scaled(&scene, 0.5, &fonts, &no_assets()).expect("half");
    // Right edge of the shadowed square, middle row.
    let reach_full = shadow_reach(&full, 200, 150);
    let reach_half = shadow_reach(&half, 100, 75);
    assert!(reach_full > 10, "full shadow reach {reach_full}");
    let expected = f64::from(reach_full) / 2.0;
    assert!(
        (f64::from(reach_half) - expected).abs() <= 3.0,
        "half reach {reach_half} not ~{expected} (full {reach_full})"
    );
    // The shadow ramp at half scale samples the full ramp at twice the step:
    // compare a few points loosely.
    for d in [2u32, 4, 6] {
        let f = i32::from(pixel(&full.rgba, full.width, 200 + 2 * d, 150).0);
        let h = i32::from(pixel(&half.rgba, half.width, 100 + d, 75).0);
        assert!((f - h).abs() <= 40, "profile at d={d}: full {f} half {h}");
    }
}

#[test]
fn half_scale_clip_follows_the_page() {
    let fonts = default_provider();
    let scene = effects_scene();
    let half = render_image_scaled(&scene, 0.5, &fonts, &no_assets()).expect("half");
    // Clip (250,200)-(350,250) maps to (125,100)-(175,125).
    assert_eq!(pixel(&half.rgba, half.width, 150, 112), (0, 128, 0, 255));
    assert_eq!(pixel(&half.rgba, half.width, 150, 130).0, 255);
    assert_eq!(pixel(&half.rgba, half.width, 120, 112).0, 255);
}
