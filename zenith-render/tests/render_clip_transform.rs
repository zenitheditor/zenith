//! Integration-style transform/clip rasterization tests via the tiny-skia
//! backend. Exercises the public `TinySkiaBackend` directly.

mod common;
use common::*;

// ── PushTransform: rotation moves ink outside the axis-aligned bbox ────

/// A red FillRect [25,25,50,50] on a 100×100 page is wrapped in a 45°
/// rotation about the rect center (50,50). We assert:
/// - at least one red pixel exists (the rect still renders), and
/// - the inked-pixel set differs from the SAME FillRect rendered WITHOUT
///   the transform (proving the rotation actually rotated the geometry), and
/// - two renders of the rotated scene are byte-identical (deterministic).
#[test]
fn push_transform_rotates_fill_rect() {
    let red_color = Color::srgb(255, 0, 0, 255);

    // Rotated scene.
    let mut rotated = Scene::new(100.0, 100.0);
    rotated.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 100.0,
        h: 100.0,
    });
    rotated.commands.push(SceneCommand::PushTransform {
        angle_deg: 45.0,
        cx: 50.0,
        cy: 50.0,
    });
    rotated.commands.push(SceneCommand::FillRect {
        x: 25.0,
        y: 25.0,
        w: 50.0,
        h: 50.0,
        paint: Paint::solid(red_color),
    });
    rotated.commands.push(SceneCommand::PopTransform);
    rotated.commands.push(SceneCommand::PopClip);

    // Unrotated baseline (same FillRect, no transform).
    let mut plain = Scene::new(100.0, 100.0);
    plain.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 100.0,
        h: 100.0,
    });
    plain.commands.push(SceneCommand::FillRect {
        x: 25.0,
        y: 25.0,
        w: 50.0,
        h: 50.0,
        paint: Paint::solid(red_color),
    });
    plain.commands.push(SceneCommand::PopClip);

    let backend = TinySkiaBackend;
    let provider = default_provider();
    let rot1 = backend
        .rasterize(&rotated, &provider, &no_assets())
        .expect("rotated rasterize 1");
    let rot2 = backend
        .rasterize(&rotated, &provider, &no_assets())
        .expect("rotated rasterize 2");
    let base = backend
        .rasterize(&plain, &provider, &no_assets())
        .expect("plain rasterize");

    // (i) Determinism: two renders of the rotated scene are byte-identical.
    assert_eq!(
        rot1.rgba, rot2.rgba,
        "two rasterizes of the rotated scene must be byte-identical"
    );

    // (ii) At least one red pixel was drawn.
    let any_red = (0..rot1.height).any(|py| {
        (0..rot1.width).any(|px| {
            let (r, g, b, a) = pixel(&rot1.rgba, rot1.width, px, py);
            a > 0 && r > g && r > b
        })
    });
    assert!(
        any_red,
        "rotated FillRect must produce at least one red pixel"
    );

    // (iii) The inked-pixel set must differ from the unrotated baseline —
    // a 45° rotation pushes ink past the original [25,75] axis-aligned bbox
    // (the rotated diamond reaches the page edges at x≈14.6 and x≈85.4).
    assert_ne!(
        rot1.rgba, base.rgba,
        "rotation must change the inked pixels versus the unrotated FillRect"
    );

    // (iv) A specific pixel OUTSIDE the original axis-aligned bbox but inside
    // the rotated diamond must be inked. The rotated diamond has corners at
    // (50,~14.6),(85.4,50),(50,85.4),(14.6,50); the point (20,50) lies inside
    // the diamond but outside the original [25..75]×[25..75] rect.
    let (_, _, _, a_outside_bbox) = pixel(&rot1.rgba, rot1.width, 20, 50);
    assert!(
        a_outside_bbox > 0,
        "pixel (20,50) is outside the unrotated bbox but inside the rotated diamond; must be inked"
    );
    // Sanity: the same pixel is transparent in the unrotated baseline.
    let (_, _, _, a_base) = pixel(&base.rgba, base.width, 20, 50);
    assert_eq!(
        a_base, 0,
        "pixel (20,50) must be transparent in the unrotated baseline"
    );
}
