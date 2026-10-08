//! Stroke coverage for PDF content streams.

use zenith_core::{BytesAssetProvider, FontProvider, default_provider};
use zenith_scene::{
    Color, FillRule, LineCap, Scene, SceneCommand, SceneGlyph, StrokeAlign, ir::PathSegment,
};

use super::{
    content::translate,
    font::{build_plan, collect_usage},
};

fn stream(commands: Vec<SceneCommand>) -> String {
    let fonts = default_provider();
    let assets = BytesAssetProvider::new();
    let mut scene = Scene::new(100.0, 80.0);
    scene.commands = commands;
    let plan = build_plan(&collect_usage(std::slice::from_ref(&scene)), &fonts, true);
    String::from_utf8(
        translate(&scene, &fonts, &assets, &plan)
            .0
            .finish()
            .into_vec(),
    )
    .unwrap()
}

fn strokes(dash: Option<f64>, gap: Option<f64>) -> Vec<SceneCommand> {
    strokes_with_cap(dash, gap, None)
}

fn strokes_with_cap(
    dash: Option<f64>,
    gap: Option<f64>,
    cap: Option<LineCap>,
) -> Vec<SceneCommand> {
    let color = Color::srgb(0, 0, 0, 255);
    vec![
        SceneCommand::StrokeRect {
            x: 10.0,
            y: 10.0,
            w: 40.0,
            h: 30.0,
            color,
            stroke_width: 2.0,
            stroke_dash: dash,
            stroke_gap: gap,
            stroke_linecap: cap,
        },
        SceneCommand::StrokeRoundedRect {
            x: 10.0,
            y: 10.0,
            w: 40.0,
            h: 30.0,
            radius: 4.0,
            radii: None,
            color,
            stroke_width: 2.0,
            stroke_dash: dash,
            stroke_gap: gap,
            stroke_linecap: cap,
        },
        SceneCommand::StrokeEllipse {
            x: 10.0,
            y: 10.0,
            w: 40.0,
            h: 30.0,
            rx: None,
            ry: None,
            color,
            stroke_width: 2.0,
            stroke_dash: dash,
            stroke_gap: gap,
            stroke_linecap: cap,
        },
        SceneCommand::StrokeLine {
            x1: 10.0,
            y1: 10.0,
            x2: 50.0,
            y2: 40.0,
            color,
            stroke_width: 2.0,
            stroke_dash: dash,
            stroke_gap: gap,
            stroke_linecap: cap,
        },
    ]
}

#[test]
fn every_dash_capable_shape_emits_scoped_dash() {
    for command in strokes(Some(7.0), Some(3.0)) {
        let output = stream(vec![command]);
        assert_eq!(output.matches("[7 3] 0 d").count(), 1, "{output}");
        assert!(output.contains("q\n"));
        assert!(output.ends_with("Q"));
    }
}

#[test]
fn dash_defaults_clamps_and_rejects_like_raster() {
    for (dash, gap, expected) in [
        (Some(7.0), None, Some("[7 7] 0 d")),
        (Some(7.0), Some(-3.0), Some("[7 0] 0 d")),
        (Some(7.0), Some(f64::NAN), Some("[7 0] 0 d")),
        (None, Some(3.0), None),
        (Some(0.0), Some(3.0), None),
        (Some(-1.0), Some(3.0), None),
        (Some(f64::NAN), None, None),
        (Some(f64::INFINITY), None, None),
        (Some(f64::MAX), None, None),
        (Some(f64::MIN_POSITIVE), Some(0.0), None),
        (Some(7.0), Some(f64::INFINITY), None),
    ] {
        for command in strokes(dash, gap) {
            let output = stream(vec![command]);
            match expected {
                Some(pattern) => assert!(output.contains(pattern), "{output}"),
                None => assert!(!output.contains(" d\n"), "{output}"),
            }
        }
    }
}

#[test]
fn dash_state_does_not_leak_into_following_solid_stroke() {
    let mut commands = strokes(Some(7.0), Some(3.0));
    commands.extend(strokes(None, None));
    let output = stream(commands);
    assert_eq!(output.matches(" d\n").count(), 4);
    assert_eq!(output.matches("q\n").count(), 8);
    assert_eq!(output.lines().filter(|line| *line == "Q").count(), 8);
    let solid = stream(strokes(None, None));
    assert!(output.ends_with(solid.strip_prefix("1 0 0 -1 0 80 cm\n").unwrap()));
}

fn glyph(
    selectable: bool,
    fill_alpha: u8,
    stroke: Option<Color>,
    width: Option<f64>,
) -> SceneCommand {
    let fonts = default_provider();
    let faces = fonts.all_faces();
    let font = faces.first().unwrap();
    let data = fonts.by_id(&font.id).unwrap();
    let face = ttf_parser::Face::parse(&data.bytes, data.index).unwrap();
    let id = face.glyph_index('A').unwrap().0;
    SceneCommand::DrawGlyphRun {
        x: 10.0,
        y: 40.0,
        font_id: font.id.clone(),
        font_size: 24.0,
        color: Color::srgb(10, 20, 30, fill_alpha),
        stroke_color: stroke,
        stroke_width: width,
        link: None,
        selectable,
        source_node_id: None,
        glyphs: vec![SceneGlyph {
            glyph_id: id,
            dx: 3.0,
            dy: 2.0,
            text: "A".into(),
        }],
    }
}

#[test]
fn selectable_glyph_stroke_preserves_single_cid_pass() {
    let output = stream(vec![glyph(
        true,
        255,
        Some(Color::srgb(255, 0, 0, 255)),
        Some(2.0),
    )]);
    assert!(!output.contains(" Tr\n"), "{output}");
    assert!(output.contains("S\n"), "{output}");
    assert_eq!(output.matches("1 0 0 -1 13 42 Tm").count(), 1);
    assert_eq!(output.matches(" Tj\n").count(), 1);
    assert_eq!(output.matches("BT\n").count(), 1);
    assert!(output.contains("1 0 0 RG\n2 w"));
}

#[test]
fn outline_glyph_stroke_draws_after_fill() {
    let output = stream(vec![glyph(
        false,
        255,
        Some(Color::srgb(255, 0, 0, 255)),
        Some(2.0),
    )]);
    assert!(!output.contains("BT\n"));
    assert!(output.contains("f\nQ\nq\n"), "{output}");
    assert_eq!(output.matches("S\n").count(), 1);
    assert!(output.contains("1 0 0 RG\n2 w"));
}

#[test]
fn glyph_fill_and_stroke_alpha_use_separate_graphics_scopes() {
    for selectable in [true, false] {
        for (fill_alpha, stroke_alpha) in [(0, 255), (255, 0), (80, 160)] {
            let output = stream(vec![glyph(
                selectable,
                fill_alpha,
                Some(Color::srgb(255, 0, 0, stroke_alpha)),
                Some(2.0),
            )]);
            assert_eq!(output.matches("q\n").count(), 2, "{output}");
            assert_eq!(output.lines().filter(|line| *line == "Q").count(), 2);
            assert_eq!(
                output.matches(" gs\n").count(),
                usize::from(fill_alpha != 255) + usize::from(stroke_alpha != 255)
            );
        }
    }
}

#[test]
fn absent_or_invalid_glyph_stroke_preserves_fill_bytes() {
    for selectable in [true, false] {
        let baseline = stream(vec![glyph(selectable, 255, None, None)]);
        for width in [
            None,
            Some(0.0),
            Some(-1.0),
            Some(f64::NAN),
            Some(f64::INFINITY),
            Some(f64::MAX),
            Some(f64::MIN_POSITIVE),
        ] {
            let output = stream(vec![glyph(
                selectable,
                255,
                Some(Color::srgb(255, 0, 0, 255)),
                width,
            )]);
            assert_eq!(output, baseline);
        }
        assert_eq!(
            stream(vec![glyph(selectable, 255, None, Some(2.0))]),
            baseline
        );
    }
}

#[test]
fn solid_stroke_retains_historical_content() {
    let output = stream(vec![strokes(None, None).remove(0)]);
    assert_eq!(
        output,
        "1 0 0 -1 0 80 cm\nq\n0 0 0 RG\n2 w\n10 10 40 30 re\nS\nQ"
    );
}

#[test]
fn dashed_shape_caps_follow_explicit_line_cap() {
    for (cap, operator) in [
        (LineCap::Butt, "0 J"),
        (LineCap::Round, "1 J"),
        (LineCap::Square, "2 J"),
    ] {
        for command in strokes_with_cap(Some(7.0), Some(3.0), Some(cap)) {
            assert!(stream(vec![command]).contains(operator));
        }
    }
}

#[test]
fn dashed_shape_scope_preserves_following_polyline_and_path_strokes() {
    let color = Color::srgb(0, 0, 0, 255);
    let solid = vec![
        SceneCommand::StrokePolyline {
            points: vec![10.0, 10.0, 30.0, 20.0],
            color,
            stroke_width: 2.0,
            closed: false,
            align: StrokeAlign::Center,
            clip_fill_rule: FillRule::NonZero,
        },
        SceneCommand::StrokePath {
            segments: vec![
                PathSegment::MoveTo { x: 10.0, y: 10.0 },
                PathSegment::LineTo { x: 30.0, y: 20.0 },
            ],
            color,
            stroke_width: 2.0,
            closed: false,
            align: StrokeAlign::Center,
            clip_fill_rule: FillRule::NonZero,
            stroke_linejoin: None,
            stroke_linecap: None,
            stroke_miter_limit: None,
        },
    ];
    let baseline = stream(solid.clone());
    let mut commands = strokes(Some(7.0), Some(3.0));
    commands.extend(solid);
    let output = stream(commands);
    assert!(output.ends_with(baseline.strip_prefix("1 0 0 -1 0 80 cm\n").unwrap()));
}

#[test]
fn stroked_selectable_text_records_link_once_and_scopes_outline() {
    let mut linked = glyph(true, 255, Some(Color::srgb(255, 0, 0, 255)), Some(2.0));
    if let SceneCommand::DrawGlyphRun { link, .. } = &mut linked {
        *link = Some("https://example.com".into());
    }
    let mut scene = Scene::new(100.0, 80.0);
    scene.commands = vec![linked, glyph(true, 255, None, None)];
    let fonts = default_provider();
    let assets = BytesAssetProvider::new();
    let plan = build_plan(&collect_usage(std::slice::from_ref(&scene)), &fonts, true);
    let (content, resources) = translate(&scene, &fonts, &assets, &plan);
    assert_eq!(resources.links.len(), 1);
    let output = String::from_utf8(content.finish().into_vec()).unwrap();
    assert_eq!(output.matches(" Tr").count(), 0);
    assert_eq!(output.matches(" Tj").count(), 2);
    assert_eq!(output.lines().filter(|line| *line == "Q").count(), 3);
    assert!(output.contains("S\nQ\nq\n0.039215688 0.078431375 0.11764706 rg\nBT"));
}

#[test]
fn overlapping_stroked_glyphs_follow_raster_paint_order() {
    for selectable in [true, false] {
        let mut command = glyph(
            selectable,
            255,
            Some(Color::srgb(255, 0, 0, 255)),
            Some(3.0),
        );
        let SceneCommand::DrawGlyphRun { glyphs, link, .. } = &mut command else {
            panic!("glyph fixture must contain a glyph run");
        };
        let mut second = glyphs.first().unwrap().clone();
        second.dx = 0.0;
        glyphs.push(second);
        *link = Some("https://example.com/overlap".into());
        let output = stream(vec![command.clone()]);
        let paints: Vec<_> = output
            .lines()
            .filter(|line| *line == "f" || *line == "S" || line.ends_with(" Tj"))
            .collect();
        if selectable {
            assert_eq!(paints.len(), 4, "{output}");
            assert!(paints[0].ends_with(" Tj"));
            assert_eq!(paints[1], "S");
            assert!(paints[2].ends_with(" Tj"));
            assert_eq!(paints[3], "S");
        } else {
            assert_eq!(paints, ["f", "S", "f", "S"]);
        }

        let SceneCommand::DrawGlyphRun { glyphs, .. } = &command else {
            panic!("glyph fixture must contain a glyph run");
        };
        let split: Vec<_> = glyphs
            .iter()
            .map(|glyph| {
                let mut single = command.clone();
                if let SceneCommand::DrawGlyphRun { glyphs, .. } = &mut single {
                    *glyphs = vec![glyph.clone()];
                }
                single
            })
            .collect();
        assert_eq!(output, stream(split.clone()));
        let mut scene = Scene::new(100.0, 80.0);
        scene.commands = vec![command];
        let mut split_scene = scene.clone();
        split_scene.commands = split;
        let fonts = default_provider();
        let assets = BytesAssetProvider::new();
        let plan = build_plan(&collect_usage(std::slice::from_ref(&scene)), &fonts, true);
        let (_, resources) = translate(&scene, &fonts, &assets, &plan);
        assert_eq!(resources.links.len(), usize::from(selectable));
        assert_eq!(
            crate::render_png(&scene, &fonts, &assets).unwrap(),
            crate::render_png(&split_scene, &fonts, &assets).unwrap()
        );
    }
}

#[test]
fn stroked_glyphs_use_raster_miter_limit_at_acute_corners() {
    assert_eq!(tiny_skia::Stroke::default().miter_limit, 4.0);
    for selectable in [true, false] {
        let output = stream(vec![glyph(
            selectable,
            255,
            Some(Color::srgb(255, 0, 0, 255)),
            Some(5.0),
        )]);
        assert_eq!(output.matches("4 M").count(), 1);
        assert!(output.contains("5 w\n4 M\n"));
        assert!(!stream(vec![glyph(selectable, 255, None, None)]).contains(" M"));
    }
}
