use super::{content::translate, font::build_plan, render_pdf};
use zenith_core::{BytesAssetProvider, default_provider};
use zenith_scene::{Color, GradientPaint, GradientStop, Paint, Scene, SceneCommand};

fn gradient(radial: bool) -> GradientPaint {
    GradientPaint {
        angle_deg: 0.0,
        radial,
        center_x: None,
        center_y: None,
        radius_frac: None,
        stops: vec![
            GradientStop {
                offset: 0.0,
                color: Color::srgb(255, 0, 0, 255),
            },
            GradientStop {
                offset: 1.0,
                color: Color::srgb(0, 0, 255, 255),
            },
        ],
    }
}
fn scene(paint: GradientPaint) -> Scene {
    let mut scene = Scene::new(60.0, 40.0);
    scene.commands.push(SceneCommand::FillRect {
        x: 10.0,
        y: 5.0,
        w: 40.0,
        h: 30.0,
        paint: Paint::Gradient(paint),
    });
    scene
}
#[test]
fn radial_gradient_uses_native_shading() {
    let scene = scene(gradient(true));
    let bytes = render_pdf(&scene, &default_provider(), &BytesAssetProvider::new());
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("/ShadingType 3"));
    assert!(text.contains("/Coords [30 20 0 30 20 25]"));
    assert!(!text.contains("/Subtype /Image"));
    assert!(text.contains("W\nn\n/sh0 sh"));
}
#[test]
fn translucent_and_outer_offset_gradients_capture_raster() {
    for radial in [false, true] {
        for variant in 0..3 {
            let mut paint = gradient(radial);
            match variant {
                0 => paint.stops[0].color.a = 128,
                1 => paint.stops[0].offset = 0.2,
                2 => paint.stops[1].offset = 0.8,
                _ => unreachable!(),
            }
            super::scope_tests::check_raster(&scene(paint));
        }
    }
}
#[test]
fn radial_center_radius_and_multistop_keep_native_clip() {
    let mut paint = gradient(true);
    paint.center_x = Some(0.25);
    paint.center_y = Some(0.75);
    paint.radius_frac = Some(0.5);
    paint.stops.insert(
        1,
        GradientStop {
            offset: 0.4,
            color: Color::srgb(0, 255, 0, 255),
        },
    );
    let scene = scene(paint);
    let fonts = default_provider();
    let assets = BytesAssetProvider::new();
    let bytes = render_pdf(&scene, &fonts, &assets);
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("/Coords [20 27.5 0 20 27.5 12.5]"));
    assert!(text.contains("/FunctionType 3"));
    let plan = build_plan(&Default::default(), &fonts, true);
    let (_, resources) = translate(&scene, &fonts, &assets, &plan);
    assert!(resources.images.is_empty());
    assert_eq!(bytes, render_pdf(&scene, &fonts, &assets));
}

#[test]
fn gradient_fallback_captures_all_fill_kinds_and_enclosing_state() {
    use zenith_scene::{FillRule, ir::PathSegment};
    for radial in [false, true] {
        let mut gradient = gradient(radial);
        gradient.stops[0].color.a = 128;
        let paint = Paint::Gradient(gradient);
        let shapes = [
            SceneCommand::FillRect {
                x: 1.0,
                y: 2.0,
                w: 16.0,
                h: 12.0,
                paint: paint.clone(),
            },
            SceneCommand::FillRoundedRect {
                x: 1.0,
                y: 2.0,
                w: 16.0,
                h: 12.0,
                radius: 3.0,
                radii: None,
                paint: paint.clone(),
            },
            SceneCommand::FillEllipse {
                x: 1.0,
                y: 2.0,
                w: 16.0,
                h: 12.0,
                rx: None,
                ry: None,
                paint: paint.clone(),
            },
            SceneCommand::FillPolygon {
                points: vec![1.0, 2.0, 17.0, 2.0, 17.0, 14.0, 1.0, 14.0],
                paint: paint.clone(),
                fill_rule: FillRule::EvenOdd,
            },
            SceneCommand::FillPath {
                segments: vec![
                    PathSegment::MoveTo { x: 1.0, y: 2.0 },
                    PathSegment::LineTo { x: 17.0, y: 2.0 },
                    PathSegment::LineTo { x: 17.0, y: 14.0 },
                    PathSegment::Close,
                ],
                paint,
                fill_rule: FillRule::EvenOdd,
            },
        ];
        for shape in shapes {
            let mut scene = Scene::new(40.0, 40.0);
            scene.commands = vec![
                SceneCommand::PushScaleTranslate {
                    sx: 1.2,
                    sy: 0.8,
                    tx: 5.0,
                    ty: 2.0,
                },
                SceneCommand::PushClip {
                    x: 3.0,
                    y: 0.0,
                    w: 12.0,
                    h: 24.0,
                },
                shape,
                SceneCommand::PopClip,
                SceneCommand::PopTransform,
            ];
            super::scope_tests::check_raster(&scene);
        }
    }
}
#[test]
fn duplicate_and_descending_stops_use_raster() {
    for offset in [0.0, 1.0, 1.2] {
        let mut paint = gradient(false);
        paint.stops.insert(
            1,
            GradientStop {
                offset,
                color: Color::srgb(0, 255, 0, 255),
            },
        );
        super::scope_tests::check_raster(&scene(paint));
    }
}

#[test]
fn incomplete_and_nonfinite_stop_layouts_select_raster_scopes() {
    let fonts = default_provider();
    for stops in [
        vec![],
        vec![GradientStop {
            offset: 0.0,
            color: Color::srgb(255, 0, 0, 255),
        }],
        vec![
            GradientStop {
                offset: f64::NAN,
                color: Color::srgb(255, 0, 0, 255),
            },
            GradientStop {
                offset: 1.0,
                color: Color::srgb(0, 0, 255, 255),
            },
        ],
    ] {
        let mut paint = gradient(false);
        paint.stops = stops;
        assert_eq!(
            super::scopes::plan(&scene(paint), &fonts).unwrap(),
            vec![0..1]
        );
    }
}
#[test]
fn degenerate_and_nonfinite_radial_geometry_emits_no_shading() {
    for value in [0.0, -1.0, f64::INFINITY, f64::NAN] {
        let mut paint = gradient(true);
        paint.radius_frac = Some(value);
        let bytes = render_pdf(
            &scene(paint),
            &default_provider(),
            &BytesAssetProvider::new(),
        );
        assert!(!String::from_utf8_lossy(&bytes).contains("/ShadingType"));
    }
    let mut paint = gradient(true);
    paint.center_x = Some(f64::NAN);
    let bytes = render_pdf(
        &scene(paint),
        &default_provider(),
        &BytesAssetProvider::new(),
    );
    assert!(!String::from_utf8_lossy(&bytes).contains("/ShadingType"));
}
