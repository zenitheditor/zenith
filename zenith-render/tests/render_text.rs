//! Integration-style glyph-run rasterization tests via the tiny-skia backend.
//! Exercises the public `render_image` / `render_png` entry points and the
//! `TinySkiaBackend` directly.

mod common;
use common::*;

// ── glyph: draws pixels ───────────────────────────────────────────────

/// Build a DrawGlyphRun scene for the letter "A" using the bundled Noto Sans
/// font, then verify that at least one pixel in the output matches the run
/// color (i.e. text was actually rasterized).
#[test]
fn glyph_run_draws_pixels() {
    let provider = default_provider();
    let families = vec!["Noto Sans".to_string()];
    let font_size = 32.0_f32;

    // Shape "A" to get a real glyph id from the bundled font.
    let req = ShapeRequest {
        text: "A",
        families: &families,
        weight: 400,
        style: FontStyle::Normal,
        font_size,
        direction: TextDirection::Ltr,
        features: &[],
        kerning_pairs: &[],
        letter_spacing_px: 0.0,
    };
    let run = RustybuzzEngine::new()
        .shape(&req, &provider)
        .expect("shaping must succeed");

    // Page: 80×40.  Baseline at y=32 (leaves room for the glyph above).
    let page_w = 80.0_f64;
    let page_h = 40.0_f64;
    let baseline_y = 34.0_f64;
    let origin_x = 4.0_f64;

    let ink_color = Color::srgb(0, 0, 200, 255);

    // Map the shaped glyphs into SceneGlyph instances.
    let glyphs: Vec<SceneGlyph> = run
        .glyphs
        .iter()
        .map(|g| SceneGlyph {
            glyph_id: g.glyph_id,
            dx: g.x,
            dy: g.y,
            text: String::new(),
        })
        .collect();

    let mut scene = Scene::new(page_w, page_h);
    scene.commands.push(SceneCommand::DrawGlyphRun {
        x: origin_x,
        y: baseline_y,
        font_id: run.font_id.clone(),
        font_size,
        color: ink_color,
        stroke_color: None,
        stroke_width: None,
        link: None,
        selectable: true,
        source_node_id: None,
        glyphs,
    });

    let img = render_image(&scene, &provider, &no_assets()).expect("render must succeed");

    // At least one pixel must have non-zero blue (the ink color).
    let any_ink = (0..img.height).any(|py| {
        (0..img.width).any(|px| {
            let (r, g, b, a) = pixel(&img.rgba, img.width, px, py);
            // Anti-aliased: the pixel need not be exactly (0,0,200,255);
            // just check that the blue channel is dominant and alpha > 0.
            a > 0 && b > r && b > g
        })
    });

    assert!(
        any_ink,
        "DrawGlyphRun must rasterize at least one ink pixel for 'A' at 32px"
    );
}

// ── glyph: determinism ────────────────────────────────────────────────

#[test]
fn glyph_run_deterministic_png() {
    let provider = default_provider();
    let families = vec!["Noto Sans".to_string()];
    let font_size = 24.0_f32;

    let req = ShapeRequest {
        text: "Zenith",
        families: &families,
        weight: 400,
        style: FontStyle::Normal,
        font_size,
        direction: TextDirection::Ltr,
        features: &[],
        kerning_pairs: &[],
        letter_spacing_px: 0.0,
    };
    let run = RustybuzzEngine::new()
        .shape(&req, &provider)
        .expect("shaping must succeed");

    let glyphs: Vec<SceneGlyph> = run
        .glyphs
        .iter()
        .map(|g| SceneGlyph {
            glyph_id: g.glyph_id,
            dx: g.x,
            dy: g.y,
            text: String::new(),
        })
        .collect();

    let mut scene = Scene::new(200.0, 40.0);
    scene.commands.push(SceneCommand::DrawGlyphRun {
        x: 4.0,
        y: 30.0,
        font_id: run.font_id.clone(),
        font_size,
        color: Color::srgb(10, 10, 10, 255),
        stroke_color: None,
        stroke_width: None,
        link: None,
        selectable: true,
        source_node_id: None,
        glyphs,
    });

    let png1 = render_png(&scene, &provider, &no_assets()).expect("first render");
    let png2 = render_png(&scene, &provider, &no_assets()).expect("second render");
    assert_eq!(
        png1, png2,
        "glyph run PNG must be byte-identical across two renders"
    );
}

// ── glyph: missing font id ────────────────────────────────────────────

#[test]
fn glyph_run_missing_font_id_succeeds_silently() {
    let provider = default_provider();

    let mut scene = Scene::new(40.0, 40.0);
    scene.commands.push(SceneCommand::DrawGlyphRun {
        x: 0.0,
        y: 20.0,
        font_id: "nonexistent-font-000-normal".to_string(),
        font_size: 16.0,
        color: Color::srgb(255, 0, 0, 255),
        stroke_color: None,
        stroke_width: None,
        link: None,
        selectable: true,
        source_node_id: None,
        glyphs: vec![SceneGlyph {
            glyph_id: 36,
            dx: 0.0,
            dy: 0.0,
            text: String::new(),
        }],
    });

    // Must succeed (Ok) — the run is skipped, no panic, no error.
    let img = render_image(&scene, &provider, &no_assets())
        .expect("render must succeed even with unknown font");

    // All pixels should be transparent (nothing was drawn).
    let any_opaque = (0..img.height).any(|py| {
        (0..img.width).any(|px| {
            let (_, _, _, a) = pixel(&img.rgba, img.width, px, py);
            a > 0
        })
    });
    assert!(
        !any_opaque,
        "no pixels should be drawn when the font id is unknown"
    );
}

// ── glyph run: sub-page clip mask is honored ──────────────────────────

/// A glyph run for "A" at 32px is placed at x≈20, baseline≈34 on an
/// 80×40 page, then wrapped in a tiny clip [0,0,4,4] that lies far from
/// the glyph ink.  After fixing DrawGlyphRun to pass `mask.as_ref()`, the
/// effective clip mask must suppress all ink → NO opaque pixel anywhere.
///
/// Before the fix (mask=None) tiny-skia only clips to the pixmap edge, so
/// the glyph would render normally and the test would fail.
#[test]
fn glyph_run_clipped_to_subpage_clip() {
    let provider = default_provider();
    let families = vec!["Noto Sans".to_string()];
    let font_size = 32.0_f32;

    let req = ShapeRequest {
        text: "A",
        families: &families,
        weight: 400,
        style: FontStyle::Normal,
        font_size,
        direction: TextDirection::Ltr,
        features: &[],
        kerning_pairs: &[],
        letter_spacing_px: 0.0,
    };
    let run = RustybuzzEngine::new()
        .shape(&req, &provider)
        .expect("shaping must succeed");

    let glyphs: Vec<SceneGlyph> = run
        .glyphs
        .iter()
        .map(|g| SceneGlyph {
            glyph_id: g.glyph_id,
            dx: g.x,
            dy: g.y,
            text: String::new(),
        })
        .collect();

    let mut scene = Scene::new(80.0, 40.0);
    // Tiny clip box [0,0,4,4] — entirely disjoint from the glyph ink.
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 4.0,
        h: 4.0,
    });
    // Glyph ink lands around x≥20, y up to baseline 34 — well outside the clip.
    scene.commands.push(SceneCommand::DrawGlyphRun {
        x: 20.0,
        y: 34.0,
        font_id: run.font_id.clone(),
        font_size,
        color: Color::srgb(0, 0, 200, 255),
        stroke_color: None,
        stroke_width: None,
        link: None,
        selectable: true,
        source_node_id: None,
        glyphs,
    });
    scene.commands.push(SceneCommand::PopClip);

    let backend = TinySkiaBackend;
    let img = backend
        .rasterize(&scene, &provider, &no_assets())
        .expect("rasterize must succeed");

    // The clip mask must suppress all glyph ink — no opaque pixel anywhere.
    let any_opaque = (0..img.height).any(|py| {
        (0..img.width).any(|px| {
            let (_, _, _, a) = pixel(&img.rgba, img.width, px, py);
            a > 0
        })
    });
    assert!(
        !any_opaque,
        "glyph ink must be fully clipped by the sub-page mask; found opaque pixels"
    );
}

// ── glyph stroke: renders without panic ───────────────────────────────

/// A DrawGlyphRun with stroke_color+stroke_width set must rasterize without
/// panic. We also verify that the rendered output DIFFERS from the same run
/// without stroke (the outline adds extra pixels).
#[test]
fn glyph_run_with_stroke_renders_without_panic() {
    let provider = default_provider();
    let families = vec!["Noto Sans".to_string()];
    let font_size = 32.0_f32;

    let req = ShapeRequest {
        text: "A",
        families: &families,
        weight: 400,
        style: FontStyle::Normal,
        font_size,
        direction: TextDirection::Ltr,
        features: &[],
        kerning_pairs: &[],
        letter_spacing_px: 0.0,
    };
    let run = RustybuzzEngine::new()
        .shape(&req, &provider)
        .expect("shaping must succeed");

    let glyphs: Vec<SceneGlyph> = run
        .glyphs
        .iter()
        .map(|g| SceneGlyph {
            glyph_id: g.glyph_id,
            dx: g.x,
            dy: g.y,
            text: String::new(),
        })
        .collect();

    // Scene WITH stroke (red outline, 3 px wide, on a blue fill).
    let mut scene_with = Scene::new(80.0, 50.0);
    scene_with.commands.push(SceneCommand::DrawGlyphRun {
        x: 4.0,
        y: 38.0,
        font_id: run.font_id.clone(),
        font_size,
        color: Color::srgb(0, 0, 200, 255),
        stroke_color: Some(Color::srgb(200, 0, 0, 255)),
        stroke_width: Some(3.0),
        link: None,
        selectable: true,
        source_node_id: None,
        glyphs: glyphs.clone(),
    });

    // Scene WITHOUT stroke (fill only, same geometry — byte-identical to prior).
    let mut scene_without = Scene::new(80.0, 50.0);
    scene_without.commands.push(SceneCommand::DrawGlyphRun {
        x: 4.0,
        y: 38.0,
        font_id: run.font_id.clone(),
        font_size,
        color: Color::srgb(0, 0, 200, 255),
        stroke_color: None,
        stroke_width: None,
        link: None,
        selectable: true,
        source_node_id: None,
        glyphs,
    });

    let backend = TinySkiaBackend;
    let img_with = backend
        .rasterize(&scene_with, &provider, &no_assets())
        .expect("stroke render must succeed without panic");
    let img_without = backend
        .rasterize(&scene_without, &provider, &no_assets())
        .expect("no-stroke render must succeed");

    // The stroke adds a red outline: at least one pixel differs.
    assert_ne!(
        img_with.rgba, img_without.rgba,
        "a 3px red stroke must alter at least one pixel vs. fill-only"
    );
}

/// A DrawGlyphRun with stroke_color=None renders byte-identically to
/// the same run with stroke_width=None (default-off / byte-identical guarantee).
#[test]
fn glyph_run_without_stroke_is_byte_identical() {
    let provider = default_provider();
    let families = vec!["Noto Sans".to_string()];
    let font_size = 24.0_f32;

    let req = ShapeRequest {
        text: "Z",
        families: &families,
        weight: 400,
        style: FontStyle::Normal,
        font_size,
        direction: TextDirection::Ltr,
        features: &[],
        kerning_pairs: &[],
        letter_spacing_px: 0.0,
    };
    let run = RustybuzzEngine::new()
        .shape(&req, &provider)
        .expect("shaping must succeed");

    let glyphs: Vec<SceneGlyph> = run
        .glyphs
        .iter()
        .map(|g| SceneGlyph {
            glyph_id: g.glyph_id,
            dx: g.x,
            dy: g.y,
            text: String::new(),
        })
        .collect();

    let make = |sc: Option<Color>, sw: Option<f64>| {
        let mut scene = Scene::new(60.0, 40.0);
        scene.commands.push(SceneCommand::DrawGlyphRun {
            x: 4.0,
            y: 30.0,
            font_id: run.font_id.clone(),
            font_size,
            color: Color::srgb(10, 10, 10, 255),
            stroke_color: sc,
            stroke_width: sw,
            link: None,
            selectable: true,
            source_node_id: None,
            glyphs: glyphs.clone(),
        });
        scene
    };

    // Both None → must be byte-identical.
    let img_a = backend_render(&make(None, None), &provider);
    let img_b = backend_render(&make(None, None), &provider);
    assert_eq!(img_a, img_b, "two no-stroke renders must be byte-identical");

    // stroke_width=0 with a color → treated as no stroke (≤0 filtered out).
    let img_zero = backend_render(
        &make(Some(Color::srgb(255, 0, 0, 255)), Some(0.0)),
        &provider,
    );
    assert_eq!(
        img_a, img_zero,
        "stroke_width=0 must produce byte-identical output to no stroke"
    );
}
