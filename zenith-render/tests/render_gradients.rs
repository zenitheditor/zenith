//! Integration-style gradient-fill rasterization tests via the tiny-skia
//! backend. Exercises the public `TinySkiaBackend` directly.

mod common;
use common::*;

// ── Gradient fill (GRAD-2) ────────────────────────────────────────────

/// A `FillRectGradient` (vertical, 90°) renders without error, produces a
/// non-uniform fill (top row differs from bottom row), and is byte-identical
/// across two renders.
#[test]
fn fill_rect_gradient_renders_non_uniform_and_deterministic() {
    let mut scene = Scene::new(40.0, 40.0);
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: 40.0,
        h: 40.0,
    });
    scene.commands.push(SceneCommand::FillRectGradient {
        x: 0.0,
        y: 0.0,
        w: 40.0,
        h: 40.0,
        gradient: GradientPaint {
            angle_deg: 90.0, // top-to-bottom
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    color: Color::srgb(0, 0, 0, 255),
                },
                GradientStop {
                    offset: 1.0,
                    color: Color::srgb(255, 255, 255, 255),
                },
            ],
            radial: false,
            center_x: None,
            center_y: None,
            radius_frac: None,
        },
    });
    scene.commands.push(SceneCommand::PopClip);

    let backend = TinySkiaBackend;
    let provider = default_provider();
    let img = backend
        .rasterize(&scene, &provider, &no_assets())
        .expect("rasterize must succeed");

    // Vertical gradient: the top row must be darker than the bottom row.
    let (top_r, _, _, _) = pixel(&img.rgba, img.width, 20, 1);
    let (bot_r, _, _, _) = pixel(&img.rgba, img.width, 20, 38);
    assert!(
        bot_r > top_r,
        "vertical gradient must brighten downward: top={top_r}, bottom={bot_r}"
    );

    // Byte-identical across two renders.
    let img2 = backend
        .rasterize(&scene, &provider, &no_assets())
        .expect("rasterize must succeed");
    assert_eq!(img.rgba, img2.rgba, "gradient render must be deterministic");
}

// ── Radial gradient (GRAD-2) ──────────────────────────────────────────

/// A radial gradient with center at (0.5, 0.5) must render symmetrically:
/// - The luminance at the horizontal center on both sides must be equal
///   (left-center ≈ right-center, demonstrating radial symmetry).
/// - The center pixel must differ from a corner pixel (demonstrating the
///   gradient actually varies across the surface).
#[test]
fn radial_gradient_renders_symmetric_and_varies() {
    let size = 40.0_f64;
    let mut scene = Scene::new(size, size);
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: size,
        h: size,
    });
    // White center → black edge, centered radial gradient.
    scene.commands.push(SceneCommand::FillRectGradient {
        x: 0.0,
        y: 0.0,
        w: size,
        h: size,
        gradient: GradientPaint {
            angle_deg: 0.0,
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    color: Color::srgb(255, 255, 255, 255),
                },
                GradientStop {
                    offset: 1.0,
                    color: Color::srgb(0, 0, 0, 255),
                },
            ],
            radial: true,
            center_x: Some(0.5),
            center_y: Some(0.5),
            radius_frac: None,
        },
    });
    scene.commands.push(SceneCommand::PopClip);

    let backend = TinySkiaBackend;
    let provider = default_provider();
    let img = backend
        .rasterize(&scene, &provider, &no_assets())
        .expect("radial gradient rasterize must succeed");

    let mid = (size / 2.0) as u32;
    let edge = 2_u32;

    // Symmetry: left-center and right-center luminance should be equal.
    let (left_r, _, _, _) = pixel(&img.rgba, img.width, edge, mid);
    let (right_r, _, _, _) = pixel(&img.rgba, img.width, size as u32 - edge - 1, mid);
    assert_eq!(
        left_r, right_r,
        "radial gradient must be horizontally symmetric: left={left_r}, right={right_r}"
    );

    // Center must differ from corner (gradient actually varies).
    let (center_r, _, _, _) = pixel(&img.rgba, img.width, mid, mid);
    let (corner_r, _, _, _) = pixel(&img.rgba, img.width, 1, 1);
    assert!(
        center_r != corner_r,
        "radial center must differ from corner: center={center_r}, corner={corner_r}"
    );
}
