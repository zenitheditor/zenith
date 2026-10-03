//! `PushClipRoundedRect` clips draws to the rounded box: corners outside the
//! arcs stay unpainted, axis-aligned or rotated, and nested clips intersect.

mod common;
use common::*;

/// A 100×100 page: optional rotation about (50,50), a rounded clip of
/// (20,20,60,60) with radius 20, and a red fill over the whole page.
fn rounded_clip_scene(angle_deg: Option<f64>, outer_clip: Option<(f64, f64, f64, f64)>) -> Scene {
    let mut s = Scene::new(100.0, 100.0);
    if let Some((x, y, w, h)) = outer_clip {
        s.commands.push(SceneCommand::PushClip { x, y, w, h });
    }
    if let Some(angle_deg) = angle_deg {
        s.commands.push(SceneCommand::PushTransform {
            angle_deg,
            cx: 50.0,
            cy: 50.0,
        });
    }
    s.commands.push(SceneCommand::PushClipRoundedRect {
        x: 20.0,
        y: 20.0,
        w: 60.0,
        h: 60.0,
        radius: 20.0,
    });
    s.commands.push(SceneCommand::FillRect {
        x: 0.0,
        y: 0.0,
        w: 100.0,
        h: 100.0,
        paint: Paint::solid(red()),
    });
    s.commands.push(SceneCommand::PopClip);
    if angle_deg.is_some() {
        s.commands.push(SceneCommand::PopTransform);
    }
    if outer_clip.is_some() {
        s.commands.push(SceneCommand::PopClip);
    }
    s
}

fn alpha_at(img: &RasterImage, (x, y): (u32, u32)) -> u8 {
    pixel(&img.rgba, img.width, x, y).3
}

#[test]
fn rounded_clip_hides_the_corner_outside_the_arc() {
    let img = render_image(
        &rounded_clip_scene(None, None),
        &default_provider(),
        &no_assets(),
    )
    .expect("render");
    assert_eq!(alpha_at(&img, (50, 50)), 255, "center painted");
    assert_eq!(alpha_at(&img, (50, 21)), 255, "top edge middle painted");
    assert_eq!(alpha_at(&img, (21, 50)), 255, "left edge middle painted");
    // Inside the square box but outside the radius-20 arc (corner center at
    // (40,40); (22,22) is ~25 px away).
    for p in [(22, 22), (77, 22), (22, 77), (77, 77)] {
        assert_eq!(alpha_at(&img, p), 0, "corner {p:?} clipped");
    }
    // Outside the box.
    assert_eq!(alpha_at(&img, (10, 50)), 0);
    assert_eq!(alpha_at(&img, (90, 50)), 0);
}

/// Device point of the box-local offset `(u, v)` from the center under a 45°
/// rotation about (50,50).
fn rotated(u: f64, v: f64) -> (u32, u32) {
    let c = std::f64::consts::FRAC_1_SQRT_2;
    let x = 50.0 + u * c - v * c;
    let y = 50.0 + u * c + v * c;
    (x.round() as u32, y.round() as u32)
}

#[test]
fn rotated_rounded_clip_follows_the_rotated_arcs() {
    let img = render_image(
        &rounded_clip_scene(Some(45.0), None),
        &default_provider(),
        &no_assets(),
    )
    .expect("render");
    assert_eq!(alpha_at(&img, (50, 50)), 255, "center painted");
    for (u, v) in [(-27.0, 0.0), (27.0, 0.0), (0.0, -27.0), (0.0, 27.0)] {
        assert_eq!(alpha_at(&img, rotated(u, v)), 255, "edge middle ({u}, {v})");
    }
    // Inside the rotated square, outside the rounded corner.
    for (u, v) in [(-27.0, -27.0), (27.0, -27.0), (-27.0, 27.0), (27.0, 27.0)] {
        assert_eq!(alpha_at(&img, rotated(u, v)), 0, "corner ({u}, {v})");
    }
}

#[test]
fn rounded_clip_intersects_the_enclosing_clip() {
    // The outer rect clip keeps only the left half of the page.
    let img = render_image(
        &rounded_clip_scene(None, Some((0.0, 0.0, 50.0, 100.0))),
        &default_provider(),
        &no_assets(),
    )
    .expect("render");
    assert_eq!(alpha_at(&img, (40, 50)), 255, "inside both clips");
    assert_eq!(alpha_at(&img, (60, 50)), 0, "outside the outer clip");
    assert_eq!(alpha_at(&img, (22, 22)), 0, "outside the rounded corner");
}

#[test]
fn rounded_clip_render_is_deterministic() {
    let scene = rounded_clip_scene(Some(30.0), None);
    let a = render_png(&scene, &default_provider(), &no_assets()).expect("a");
    let b = render_png(&scene, &default_provider(), &no_assets()).expect("b");
    assert_eq!(a, b);
}
