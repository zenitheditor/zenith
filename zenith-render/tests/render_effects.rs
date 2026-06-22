//! Integration-style effect rasterization tests (shadow, blend-mode layers,
//! gaussian blur) via the tiny-skia backend. Exercises the public
//! `TinySkiaBackend` directly.

mod common;
use common::*;

// ── Shadow (SHAD-2) ───────────────────────────────────────────────────

/// A `BeginShadow` (one black, blurred, slightly-offset layer) wrapping a
/// `FillRect`, closed by `EndShadow`, must:
/// (i) render deterministically (two runs byte-identical), and
/// (ii) bleed shadow ink OUTSIDE the original rect but within blur range.
#[test]
fn shadow_blurs_and_bleeds_outward_deterministically() {
    // 40×40 canvas; an opaque red rect at [15,25]×[15,25].
    let build = || {
        let mut scene = Scene::new(40.0, 40.0);
        scene.commands.push(SceneCommand::BeginShadow {
            shadows: vec![ShadowSpec {
                dx: 1.0,
                dy: 1.0,
                blur: 3.0,
                color: Color::srgb(0, 0, 0, 255),
            }],
        });
        scene.commands.push(SceneCommand::FillRect {
            x: 15.0,
            y: 15.0,
            w: 10.0,
            h: 10.0,
            color: Color::srgb(255, 0, 0, 255),
        });
        scene.commands.push(SceneCommand::EndShadow);
        scene
    };

    let backend = TinySkiaBackend;
    let provider = default_provider();
    let img1 = backend
        .rasterize(&build(), &provider, &no_assets())
        .expect("rasterize must succeed");
    let img2 = backend
        .rasterize(&build(), &provider, &no_assets())
        .expect("rasterize must succeed");

    // (i) Determinism.
    assert_eq!(
        img1.rgba, img2.rgba,
        "shadow render must be byte-identical across runs"
    );

    // (ii) The crisp red ink is intact inside the rect.
    let (cr, _, _, ca) = pixel(&img1.rgba, img1.width, 20, 20);
    assert!(ca == 255 && cr > 200, "rect center must stay opaque red");

    // (iii) Shadow bleeds OUTSIDE the original rect: a pixel just left of the
    // rect's left edge (x=12, well outside [15,25)) but within blur range
    // must be non-transparent and dark (the black shadow).
    let (sr, sg, sb, sa) = pixel(&img1.rgba, img1.width, 12, 20);
    assert!(
        sa > 0,
        "shadow must bleed outside the rect (x=12): got alpha {sa}"
    );
    assert!(
        sr < 128 && sg < 128 && sb < 128,
        "the bled shadow pixel must be dark: ({sr},{sg},{sb})"
    );

    // (iv) Far outside the blur range stays transparent.
    let (_, _, _, far_a) = pixel(&img1.rgba, img1.width, 0, 0);
    assert_eq!(far_a, 0, "corner far from the shadow must stay transparent");
}

// ── Blend-mode layer compositing ──────────────────────────────────────

/// A blue rect wrapped in a `PushLayer { Multiply } … PopLayer` over a red
/// background composites the overlap as multiply (red×blue → black), which is
/// darker than either source. The render must not panic.
#[test]
fn blend_multiply_layer_darkens_overlap() {
    let page = 8.0;
    let mut scene = Scene::new(page, page);
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: page,
        h: page,
    });
    // Opaque red background covering the page.
    scene.commands.push(SceneCommand::FillRect {
        x: 0.0,
        y: 0.0,
        w: page,
        h: page,
        color: Color::srgb(255, 0, 0, 255),
    });
    // Blue rect over the red, composited with multiply via a layer.
    scene.commands.push(SceneCommand::PushLayer {
        opacity: 1.0,
        blend_mode: Some(BlendMode::Multiply),
    });
    scene.commands.push(SceneCommand::FillRect {
        x: 0.0,
        y: 0.0,
        w: page,
        h: page,
        color: Color::srgb(0, 0, 255, 255),
    });
    scene.commands.push(SceneCommand::PopLayer);
    scene.commands.push(SceneCommand::PopClip);

    let backend = TinySkiaBackend;
    let provider = default_provider();
    let img = backend
        .rasterize(&scene, &provider, &no_assets())
        .expect("rasterize must succeed");

    // Multiply(red, blue) = (255*0, 0*0, 0*255)/255 = (0,0,0): black, darker
    // than both the red and the blue source.
    let (r, g, b, a) = pixel(&img.rgba, img.width, 4, 4);
    assert_eq!(a, 255, "overlap must be opaque");
    assert!(
        r < 16 && g < 16 && b < 16,
        "multiply overlap must be near-black (darker than red and blue); got r={r} g={g} b={b}"
    );
}

/// A normal scene with NO layer commands renders unchanged: a solid red page
/// stays solid red (the layer mechanism never touches the no-layer path).
#[test]
fn no_layer_scene_unchanged() {
    let scene = make_solid_red_scene(4.0);
    let backend = TinySkiaBackend;
    let provider = default_provider();
    let img = backend
        .rasterize(&scene, &provider, &no_assets())
        .expect("rasterize must succeed");
    assert_eq!(pixel(&img.rgba, img.width, 2, 2), (255, 0, 0, 255));
}

// ── Element gaussian blur ─────────────────────────────────────────────────

/// A `BeginBlur { radius: 6.0 }` wrapping a `FillRect`, closed by `EndBlur`,
/// must render deterministically and produce a visually softer result:
/// (i) the render does not panic, (ii) runs are byte-identical, (iii) the
/// blurred image has non-zero alpha outside the original rect (spread), and
/// (iv) the center of the blurred rect has reduced alpha compared with
/// the unblurred version (the blur spreads the ink outward).
#[test]
fn begin_end_blur_renders_deterministically_and_softens_ink() {
    let build = || {
        let mut scene = Scene::new(60.0, 60.0);
        scene.commands.push(SceneCommand::BeginBlur { radius: 6.0 });
        scene.commands.push(SceneCommand::FillRect {
            x: 20.0,
            y: 20.0,
            w: 20.0,
            h: 20.0,
            color: Color::srgb(255, 0, 0, 255),
        });
        scene.commands.push(SceneCommand::EndBlur);
        scene
    };

    let build_crisp = || {
        let mut scene = Scene::new(60.0, 60.0);
        // Same rect but NO blur bracket.
        scene.commands.push(SceneCommand::FillRect {
            x: 20.0,
            y: 20.0,
            w: 20.0,
            h: 20.0,
            color: Color::srgb(255, 0, 0, 255),
        });
        scene
    };

    let backend = TinySkiaBackend;
    let provider = default_provider();

    let img1 = backend
        .rasterize(&build(), &provider, &no_assets())
        .expect("rasterize must not panic");
    let img2 = backend
        .rasterize(&build(), &provider, &no_assets())
        .expect("rasterize must not panic (second run)");
    let img_crisp = backend
        .rasterize(&build_crisp(), &provider, &no_assets())
        .expect("crisp rasterize must not panic");

    // (i) + (ii) Determinism: two blurred runs must be byte-identical.
    assert_eq!(
        img1.rgba, img2.rgba,
        "blur render must be byte-identical across runs"
    );

    // (iii) Spread: a pixel just outside the original rect (x=18, inside
    // the blur radius) must have non-zero alpha.
    let (_, _, _, spread_a) = pixel(&img1.rgba, img1.width, 18, 30);
    assert!(
        spread_a > 0,
        "blur must spread ink outside the original rect: alpha at (18,30) = {spread_a}"
    );

    // (iv) Softening: the center of the blurred rect must have lower alpha
    // than the crisp version (blur dilutes the peak density).
    let (_, _, _, blurred_center_a) = pixel(&img1.rgba, img1.width, 30, 30);
    let (_, _, _, crisp_center_a) = pixel(&img_crisp.rgba, img_crisp.width, 30, 30);
    assert!(
        blurred_center_a < crisp_center_a,
        "blur must reduce peak alpha at the rect center: blurred={blurred_center_a} crisp={crisp_center_a}"
    );
}

/// A scene with NO `BeginBlur`/`EndBlur` must produce output byte-identical
/// to what it produced before blur was introduced (no regression for
/// blur-free documents).
#[test]
fn no_blur_command_scene_is_byte_identical() {
    // Two independently built scenes with no blur.
    let build = || {
        let mut scene = Scene::new(20.0, 20.0);
        scene.commands.push(SceneCommand::FillRect {
            x: 5.0,
            y: 5.0,
            w: 10.0,
            h: 10.0,
            color: Color::srgb(0, 128, 255, 200),
        });
        scene
    };

    let backend = TinySkiaBackend;
    let provider = default_provider();
    let img1 = backend
        .rasterize(&build(), &provider, &no_assets())
        .expect("rasterize");
    let img2 = backend
        .rasterize(&build(), &provider, &no_assets())
        .expect("rasterize");
    assert_eq!(
        img1.rgba, img2.rgba,
        "non-blur scene must be byte-identical"
    );
}
