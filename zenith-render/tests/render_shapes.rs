//! Integration-style pixel/determinism tests for shape rasterization via the
//! tiny-skia backend. Exercises the public `render_png` entry point and the
//! `TinySkiaBackend` directly.

mod common;
use common::*;
use zenith_scene::ir::PathSegment;

// ── pixel correctness ─────────────────────────────────────────────────

#[test]
fn pixel_correctness_solid_red() {
    let scene = make_solid_red_scene(4.0);
    let backend = TinySkiaBackend;
    let provider = default_provider();
    let img = backend
        .rasterize(&scene, &provider, &no_assets())
        .expect("rasterize must succeed");
    assert_eq!(img.width, 4);
    assert_eq!(img.height, 4);
    // center pixel
    assert_eq!(pixel(&img.rgba, img.width, 2, 2), (255, 0, 0, 255));
    // corner pixel
    assert_eq!(pixel(&img.rgba, img.width, 0, 0), (255, 0, 0, 255));
}

// ── determinism ───────────────────────────────────────────────────────

#[test]
fn determinism_identical_png_bytes() {
    let scene = make_solid_red_scene(4.0);
    let backend = TinySkiaBackend;
    let provider = default_provider();
    let png1 = backend
        .rasterize(&scene, &provider, &no_assets())
        .and_then(|img| backend.encode_png(&img))
        .expect("first render");
    let png2 = backend
        .rasterize(&scene, &provider, &no_assets())
        .and_then(|img| backend.encode_png(&img))
        .expect("second render");
    assert_eq!(
        png1, png2,
        "PNG output must be byte-identical for the same scene"
    );
}

#[test]
fn cubic_path_renders_ink_deterministically() {
    let mut scene = Scene::new(80.0, 80.0);
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 80.0,
        h: 80.0,
    });
    scene.commands.push(SceneCommand::FillPath {
        segments: vec![
            PathSegment::MoveTo { x: 10.0, y: 60.0 },
            PathSegment::CubicTo {
                x1: 20.0,
                y1: 5.0,
                x2: 60.0,
                y2: 5.0,
                x: 70.0,
                y: 60.0,
            },
            PathSegment::LineTo { x: 10.0, y: 60.0 },
            PathSegment::Close,
        ],
        paint: Paint::solid(Color::srgb(0, 180, 80, 255)),
        even_odd: false,
    });
    scene.commands.push(SceneCommand::PopClip);

    let backend = TinySkiaBackend;
    let provider = default_provider();
    let img = backend
        .rasterize(&scene, &provider, &no_assets())
        .expect("path render");
    assert!(
        img.rgba.chunks_exact(4).any(|px| px[3] > 0),
        "cubic path must render at least one ink pixel"
    );

    let png1 = backend.encode_png(&img).expect("first png");
    let png2 = backend
        .rasterize(&scene, &provider, &no_assets())
        .and_then(|next| backend.encode_png(&next))
        .expect("second png");
    assert_eq!(png1, png2, "cubic path PNG must be byte-identical");
}

// ── PNG validity ──────────────────────────────────────────────────────

#[test]
fn png_magic_bytes() {
    let scene = make_solid_red_scene(4.0);
    let backend = TinySkiaBackend;
    let provider = default_provider();
    let png = backend
        .rasterize(&scene, &provider, &no_assets())
        .and_then(|img| backend.encode_png(&img))
        .expect("render");
    assert_eq!(
        &png[..8],
        &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        "output must start with PNG magic bytes"
    );
}

// ── clip enforced ─────────────────────────────────────────────────────

#[test]
fn clip_clamps_fill_to_page() {
    // 4×4 page; FillRect extends well beyond the page edge.
    let mut scene = Scene::new(4.0, 4.0);
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 4.0,
        h: 4.0,
    });
    scene.commands.push(SceneCommand::FillRect {
        x: 2.0,
        y: 2.0,
        w: 10.0,
        h: 10.0,
        paint: Paint::solid(red()),
    });
    scene.commands.push(SceneCommand::PopClip);

    let backend = TinySkiaBackend;
    let provider = default_provider();
    let img = backend
        .rasterize(&scene, &provider, &no_assets())
        .expect("must not panic or error");
    assert_eq!(img.width, 4);
    assert_eq!(img.height, 4);
    // Pixel inside the overlap region (3,3) should be red.
    assert_eq!(pixel(&img.rgba, img.width, 3, 3), (255, 0, 0, 255));
    // Pixel outside the fill (0,0) should be transparent.
    assert_eq!(pixel(&img.rgba, img.width, 0, 0), (0, 0, 0, 0));
}

// ── transparent default ───────────────────────────────────────────────

#[test]
fn transparent_default_no_fill() {
    let mut scene = Scene::new(4.0, 4.0);
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 4.0,
        h: 4.0,
    });
    scene.commands.push(SceneCommand::PopClip);

    let backend = TinySkiaBackend;
    let provider = default_provider();
    let img = backend
        .rasterize(&scene, &provider, &no_assets())
        .expect("must succeed");
    // All pixels must be fully transparent.
    for i in 0..(img.width * img.height) {
        let base = (i * 4) as usize;
        assert_eq!(
            &img.rgba[base..base + 4],
            &[0, 0, 0, 0],
            "pixel {i} must be transparent"
        );
    }
}

// ── invalid size ──────────────────────────────────────────────────────

#[test]
fn invalid_zero_size_returns_error() {
    let scene = Scene::new(0.0, 0.0);
    let backend = TinySkiaBackend;
    let provider = default_provider();
    assert!(
        backend.rasterize(&scene, &provider, &no_assets()).is_err(),
        "zero-size scene must return RenderError"
    );
}

// ── FillPolygon: triangle renders + determinism ───────────────────────

#[test]
fn fill_polygon_renders() {
    // A simple triangle on a 100×100 page.
    let color = Color::srgb(0, 200, 0, 255);
    let mut scene = Scene::new(100.0, 100.0);
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 100.0,
        h: 100.0,
    });
    scene.commands.push(SceneCommand::FillPolygon {
        // Triangle: top-center, bottom-right, bottom-left
        points: vec![50.0, 10.0, 90.0, 90.0, 10.0, 90.0],
        paint: Paint::solid(color),
        even_odd: false,
    });
    scene.commands.push(SceneCommand::PopClip);

    let backend = TinySkiaBackend;
    let provider = default_provider();
    let img1 = backend
        .rasterize(&scene, &provider, &no_assets())
        .expect("rasterize 1");

    // At least one pixel inside the triangle must be green.
    let any_ink = (0..img1.height).any(|py| {
        (0..img1.width).any(|px| {
            let (_, g, _, a) = pixel(&img1.rgba, img1.width, px, py);
            a > 0 && g > 0
        })
    });
    assert!(
        any_ink,
        "FillPolygon must rasterize at least one green pixel"
    );

    // Determinism: two renders must be byte-identical.
    let img2 = backend
        .rasterize(&scene, &provider, &no_assets())
        .expect("rasterize 2");
    assert_eq!(
        img1.rgba, img2.rgba,
        "two rasterizes of FillPolygon must be byte-identical"
    );
}

// ── StrokePolyline: open stroke renders + determinism ─────────────────

#[test]
fn stroke_polyline_renders() {
    let color = Color::srgb(255, 0, 0, 255);
    let mut scene = Scene::new(100.0, 100.0);
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 100.0,
        h: 100.0,
    });
    scene.commands.push(SceneCommand::StrokePolyline {
        points: vec![10.0, 50.0, 50.0, 10.0, 90.0, 50.0],
        color,
        stroke_width: 4.0,
        closed: false,
        align: StrokeAlign::Center,
        fill_even_odd: false,
    });
    scene.commands.push(SceneCommand::PopClip);

    let backend = TinySkiaBackend;
    let provider = default_provider();
    let img1 = backend
        .rasterize(&scene, &provider, &no_assets())
        .expect("rasterize 1");

    // At least one pixel must be inked.
    let any_ink = (0..img1.height).any(|py| {
        (0..img1.width).any(|px| {
            let (_, _, _, a) = pixel(&img1.rgba, img1.width, px, py);
            a > 0
        })
    });
    assert!(
        any_ink,
        "StrokePolyline must rasterize at least one ink pixel"
    );

    // Determinism.
    let img2 = backend
        .rasterize(&scene, &provider, &no_assets())
        .expect("rasterize 2");
    assert_eq!(
        img1.rgba, img2.rgba,
        "two rasterizes of StrokePolyline must be byte-identical"
    );
}

#[test]
fn stroke_ellipse_renders() {
    let color = Color::srgb(255, 0, 0, 255);
    let mut scene = Scene::new(100.0, 100.0);
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 100.0,
        h: 100.0,
    });
    scene.commands.push(SceneCommand::StrokeEllipse {
        x: 20.0,
        y: 30.0,
        w: 60.0,
        h: 40.0,
        rx: None,
        ry: None,
        color,
        stroke_width: 4.0,
        stroke_dash: None,
        stroke_gap: None,
        stroke_linecap: None,
    });
    scene.commands.push(SceneCommand::PopClip);

    let backend = TinySkiaBackend;
    let provider = default_provider();
    let img1 = backend
        .rasterize(&scene, &provider, &no_assets())
        .expect("rasterize 1");

    // At least one pixel must be inked.
    let any_ink = (0..img1.height).any(|py| {
        (0..img1.width).any(|px| {
            let (_, _, _, a) = pixel(&img1.rgba, img1.width, px, py);
            a > 0
        })
    });
    assert!(
        any_ink,
        "StrokeEllipse must rasterize at least one ink pixel"
    );

    // Determinism.
    let img2 = backend
        .rasterize(&scene, &provider, &no_assets())
        .expect("rasterize 2");
    assert_eq!(
        img1.rgba, img2.rgba,
        "two rasterizes of StrokeEllipse must be byte-identical"
    );
}

// ── ellipse: partial clip truncates, does not reshape ─────────────────

/// A 20×20 circle (FillEllipse x=0,y=0,w=20,h=20) is drawn inside a
/// bottom-right quadrant clip [10,10,20,20].
///
/// Correct behaviour (TRUNCATE): the ellipse is drawn at its TRUE bounds
/// and the mask chops off the parts outside [10,10,20,20].
///
/// Old wrong behaviour (RESHAPE): the ellipse bbox was intersected with the
/// clip, yielding a tiny oval fitted to [10,10,10,10].  A corner pixel such
/// as (18,18) — inside the clip box but outside the true circle — would
/// have been filled because the reshaping made the oval touch it.
///
/// We assert:
/// - (18,18) alpha == 0  (outside the true circle; must stay transparent)
/// - (12,12) alpha > 0   (inside both clip and true circle; must be filled)
#[test]
fn ellipse_partial_clip_truncates_not_reshapes() {
    let mut scene = Scene::new(20.0, 20.0);
    // Full-page outer clip.
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 20.0,
        h: 20.0,
    });
    // Bottom-right quadrant sub-page clip.
    scene.commands.push(SceneCommand::PushClip {
        x: 10.0,
        y: 10.0,
        w: 10.0,
        h: 10.0,
    });
    // A circle that exactly fits the full page (center (10,10), r=10).
    scene.commands.push(SceneCommand::FillEllipse {
        x: 0.0,
        y: 0.0,
        w: 20.0,
        h: 20.0,
        rx: None,
        ry: None,
        paint: Paint::solid(Color::srgb(255, 255, 255, 255)),
    });
    scene.commands.push(SceneCommand::PopClip);
    scene.commands.push(SceneCommand::PopClip);

    let backend = TinySkiaBackend;
    let provider = default_provider();
    let img = backend
        .rasterize(&scene, &provider, &no_assets())
        .expect("rasterize must succeed");

    // (18,18): inside clip [10,10,20,20] but outside the true circle
    // (dist from center (10,10) ≈ √(8²+8²) ≈ 11.3 > 10).
    // Must be transparent — the ellipse should be TRUNCATED here, not
    // reshaping the oval to fill the entire clip box.
    let (_, _, _, a_outside) = pixel(&img.rgba, img.width, 18, 18);
    assert_eq!(
        a_outside, 0,
        "pixel (18,18) is outside the true circle; must be transparent (truncate, not reshape)"
    );

    // (12,12): inside both the clip box and the true circle
    // (dist from center (10,10) ≈ √(2²+2²) ≈ 2.8 < 10).
    // Must have been drawn (alpha > 0).
    let (_, _, _, a_inside) = pixel(&img.rgba, img.width, 12, 12);
    assert!(
        a_inside > 0,
        "pixel (12,12) is inside both the clip and the circle; must be filled"
    );
}

// ── stroke line: sub-page clip mask is honored ────────────────────────

/// A diagonal stroked line spanning the page is wrapped in a small top-left
/// clip [0,0,5,5]. After wiring StrokeLine to `mask.as_ref()`, ink beyond the
/// clip (e.g. (15,15), on the line but outside the box) must be suppressed,
/// while ink inside the clip (near (2,2)) remains. Before the fix the line
/// drew its full length (sub-page clip ignored) and (15,15) would be inked.
#[test]
fn stroke_line_clipped_to_subpage_clip() {
    let mut scene = Scene::new(20.0, 20.0);
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 20.0,
        h: 20.0,
    });
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 5.0,
        h: 5.0,
    });
    scene.commands.push(SceneCommand::StrokeLine {
        x1: 0.0,
        y1: 0.0,
        x2: 20.0,
        y2: 20.0,
        color: Color::srgb(0, 0, 0, 255),
        stroke_width: 4.0,
        stroke_dash: None,
        stroke_gap: None,
        stroke_linecap: None,
    });
    scene.commands.push(SceneCommand::PopClip);
    scene.commands.push(SceneCommand::PopClip);

    let backend = TinySkiaBackend;
    let provider = default_provider();
    let img = backend
        .rasterize(&scene, &provider, &no_assets())
        .expect("rasterize must succeed");

    // (15,15): on the line but outside the [0,0,5,5] clip → must be clipped away.
    let (_, _, _, a_outside) = pixel(&img.rgba, img.width, 15, 15);
    assert_eq!(
        a_outside, 0,
        "pixel (15,15) is outside the sub-page clip; the stroked line must be truncated there"
    );

    // (2,2): on the line and inside the clip → must be inked.
    let (_, _, _, a_inside) = pixel(&img.rgba, img.width, 2, 2);
    assert!(
        a_inside > 0,
        "pixel (2,2) is on the line inside the clip; must be inked"
    );
}

// ── StrokeRect: border pixels are inked ───────────────────────────────

#[test]
fn stroke_rect_draws_border_pixels() {
    let mut scene = Scene::new(40.0, 40.0);
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 40.0,
        h: 40.0,
    });
    scene.commands.push(SceneCommand::StrokeRect {
        x: 10.0,
        y: 10.0,
        w: 20.0,
        h: 20.0,
        color: Color::srgb(0, 0, 0, 255),
        stroke_width: 4.0,
        stroke_dash: None,
        stroke_gap: None,
        stroke_linecap: None,
    });
    scene.commands.push(SceneCommand::PopClip);

    let backend = TinySkiaBackend;
    let provider = default_provider();
    let img = backend
        .rasterize(&scene, &provider, &no_assets())
        .expect("rasterize must succeed");

    // A pixel on the top border (around y=10) must be inked.
    let (_, _, _, a_border) = pixel(&img.rgba, img.width, 20, 10);
    assert!(a_border > 0, "top border pixel (20,10) must be inked");

    // The interior center (20,20) must be EMPTY (stroke, not fill).
    let (_, _, _, a_center) = pixel(&img.rgba, img.width, 20, 20);
    assert_eq!(a_center, 0, "stroke-only interior (20,20) must be empty");
}

// ── FillRoundedRect: corner is cut, center is filled ──────────────────

#[test]
fn fill_rounded_rect_cuts_corner_fills_center() {
    // A rect [0,0,40,40] with a large radius (20 → fully circular) leaves the
    // extreme corner pixel (0,0) at background while the center is filled.
    let mut scene = Scene::new(40.0, 40.0);
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 40.0,
        h: 40.0,
    });
    scene.commands.push(SceneCommand::FillRoundedRect {
        x: 0.0,
        y: 0.0,
        w: 40.0,
        h: 40.0,
        radius: 20.0,
        radii: None,
        paint: Paint::solid(Color::srgb(255, 255, 255, 255)),
    });
    scene.commands.push(SceneCommand::PopClip);

    let backend = TinySkiaBackend;
    let provider = default_provider();
    let img = backend
        .rasterize(&scene, &provider, &no_assets())
        .expect("rasterize must succeed");

    // Corner (0,0) is outside the rounded shape → transparent.
    let (_, _, _, a_corner) = pixel(&img.rgba, img.width, 0, 0);
    assert_eq!(
        a_corner, 0,
        "corner pixel (0,0) must be cut by the radius (transparent)"
    );

    // Center (20,20) is inside → filled.
    let (_, _, _, a_center) = pixel(&img.rgba, img.width, 20, 20);
    assert!(a_center > 0, "center pixel (20,20) must be filled");
}

// ── determinism: StrokeRect + FillRoundedRect + StrokeRoundedRect ─────

#[test]
fn rounded_and_stroke_rects_deterministic_png() {
    let mut scene = Scene::new(80.0, 80.0);
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 80.0,
        h: 80.0,
    });
    scene.commands.push(SceneCommand::StrokeRect {
        x: 5.0,
        y: 5.0,
        w: 30.0,
        h: 30.0,
        color: Color::srgb(200, 0, 0, 255),
        stroke_width: 3.0,
        stroke_dash: None,
        stroke_gap: None,
        stroke_linecap: None,
    });
    scene.commands.push(SceneCommand::FillRoundedRect {
        x: 40.0,
        y: 5.0,
        w: 30.0,
        h: 30.0,
        radius: 10.0,
        radii: None,
        paint: Paint::solid(Color::srgb(0, 200, 0, 255)),
    });
    scene.commands.push(SceneCommand::StrokeRoundedRect {
        x: 20.0,
        y: 40.0,
        w: 40.0,
        h: 30.0,
        radius: 8.0,
        radii: None,
        color: Color::srgb(0, 0, 200, 255),
        stroke_width: 3.0,
        stroke_dash: None,
        stroke_gap: None,
        stroke_linecap: None,
    });
    scene.commands.push(SceneCommand::PopClip);

    let provider = default_provider();
    let png1 = render_png(&scene, &provider, &no_assets()).expect("first render");
    let png2 = render_png(&scene, &provider, &no_assets()).expect("second render");
    assert_eq!(
        png1, png2,
        "StrokeRect + FillRoundedRect + StrokeRoundedRect scene must render byte-identically"
    );
}

// ── dashed StrokeRect: renders without panic ──────────────────────────

/// A `StrokeRect` with `stroke_dash=Some(8.0)` and `stroke_gap=Some(4.0)` must
/// rasterize without panicking and ink at least one pixel (proving the dashed
/// path is exercised). The dashed render must also differ from the solid render
/// (different pixel pattern).
#[test]
fn dashed_stroke_rect_renders_without_panic() {
    let color = Color::srgb(255, 0, 0, 255);

    let mut dashed_scene = Scene::new(60.0, 60.0);
    dashed_scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 60.0,
        h: 60.0,
    });
    dashed_scene.commands.push(SceneCommand::StrokeRect {
        x: 5.0,
        y: 5.0,
        w: 50.0,
        h: 50.0,
        color,
        stroke_width: 3.0,
        stroke_dash: Some(8.0),
        stroke_gap: Some(4.0),
        stroke_linecap: Some(zenith_scene::ir::LineCap::Round),
    });
    dashed_scene.commands.push(SceneCommand::PopClip);

    let backend = TinySkiaBackend;
    let provider = default_provider();
    let img = backend
        .rasterize(&dashed_scene, &provider, &no_assets())
        .expect("dashed StrokeRect must rasterize without panic");

    // At least one pixel must be inked (the dashed path does produce ink).
    let any_ink = (0..img.height).any(|py| {
        (0..img.width).any(|px| {
            let (_, _, _, a) = pixel(&img.rgba, img.width, px, py);
            a > 0
        })
    });
    assert!(any_ink, "dashed StrokeRect must ink at least one pixel");

    // The dashed version differs from a solid stroke (different pixel pattern).
    let mut solid_scene = Scene::new(60.0, 60.0);
    solid_scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 60.0,
        h: 60.0,
    });
    solid_scene.commands.push(SceneCommand::StrokeRect {
        x: 5.0,
        y: 5.0,
        w: 50.0,
        h: 50.0,
        color,
        stroke_width: 3.0,
        stroke_dash: None,
        stroke_gap: None,
        stroke_linecap: None,
    });
    solid_scene.commands.push(SceneCommand::PopClip);
    let solid_img = backend
        .rasterize(&solid_scene, &provider, &no_assets())
        .expect("solid StrokeRect must rasterize");

    assert_ne!(
        img.rgba, solid_img.rgba,
        "dashed and solid strokes must produce different pixel output"
    );
}
