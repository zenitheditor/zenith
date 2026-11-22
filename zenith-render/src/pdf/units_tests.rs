//! Unit tests for PDF page units: one scene px is `PX_TO_PT` = 0.75 pt.

use zenith_core::{BytesAssetProvider, FontProvider, PX_TO_PT, default_provider};
use zenith_scene::{Color, Paint, Rect, Scene, SceneCommand, SceneGlyph};

use super::content::translate;
use super::font;
use super::render_pdf;

fn render(scene: &Scene) -> String {
    let fonts = default_provider();
    let assets = BytesAssetProvider::new();
    String::from_utf8_lossy(&render_pdf(scene, &fonts, &assets)).into_owned()
}

/// The numbers of the first `/<key> [ ... ]` array in `text`.
fn array(text: &str, key: &str) -> Vec<f64> {
    let needle = format!("/{key} [");
    let start = text.find(&needle).expect("key present") + needle.len();
    let end = start + text[start..].find(']').expect("array closes");
    text[start..end]
        .split_whitespace()
        .map(|v| v.parse::<f64>().expect("number"))
        .collect()
}

fn assert_close(got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "got {got:?}, want {want:?}");
    for (g, w) in got.iter().zip(want) {
        assert!((g - w).abs() < 1e-3, "got {got:?}, want {want:?}");
    }
}

/// The page content stream: the stream that holds the `re` operator.
fn content_stream(text: &str) -> &str {
    text.split("stream\n")
        .skip(1)
        .map(|s| s.split("endstream").next().unwrap_or_default())
        .find(|s| s.contains(" re\n"))
        .expect("content stream with a rect")
}

fn rect_scene(w: f64, h: f64) -> Scene {
    let mut scene = Scene::new(w, h);
    scene.commands.push(SceneCommand::FillRect {
        x: 10.0,
        y: 20.0,
        w: 30.0,
        h: 40.0,
        paint: Paint::solid(Color::srgb(1, 2, 3, 255)),
    });
    scene
}

#[test]
fn px_to_pt_is_72_over_96() {
    assert_eq!(PX_TO_PT, 0.75);
}

#[test]
fn a4_page_boxes_are_in_points() {
    let text = render(&rect_scene(794.0, 1123.0));
    for key in ["MediaBox", "TrimBox", "BleedBox", "CropBox"] {
        assert_close(&array(&text, key), &[0.0, 0.0, 595.5, 842.25]);
    }
}

#[test]
fn bleed_trim_box_is_in_points() {
    // A4 trim with a 12 px bleed on each side: canvas 818 x 1147 px.
    let mut scene = rect_scene(818.0, 1147.0);
    scene.trim = Some(Rect {
        x: 12.0,
        y: 12.0,
        w: 794.0,
        h: 1123.0,
    });
    let text = render(&scene);
    // 818 * 0.75 = 613.5, 1147 * 0.75 = 860.25.
    assert_close(&array(&text, "MediaBox"), &[0.0, 0.0, 613.5, 860.25]);
    assert_close(&array(&text, "BleedBox"), &[0.0, 0.0, 613.5, 860.25]);
    assert_close(&array(&text, "CropBox"), &[0.0, 0.0, 613.5, 860.25]);
    // Trim: 12 * 0.75 = 9, (12 + 794) * 0.75 = 604.5, (12 + 1123) * 0.75 = 851.25.
    assert_close(&array(&text, "TrimBox"), &[9.0, 9.0, 604.5, 851.25]);
}

#[test]
fn content_stream_opens_with_scale_then_flip_and_keeps_px() {
    let text = render(&rect_scene(64.0, 48.0));
    let stream = content_stream(&text);
    assert!(
        stream.starts_with("0.75 0 0 0.75 0 0 cm\n1 0 0 -1 0 48 cm\n"),
        "stream must open with the scale cm, then the flip cm; got:\n{stream}"
    );
    assert!(
        stream.contains("10 20 30 40 re\n"),
        "rect operands must stay in px; got:\n{stream}"
    );
    assert_eq!(
        stream.matches("q\n").count(),
        stream.matches("Q\n").count(),
        "q and Q must balance; got:\n{stream}"
    );
}

#[test]
fn link_rect_is_the_scaled_flipped_run_box() {
    let fonts = default_provider();
    let font_id = fonts.all_faces().first().expect("bundled face").id.clone();
    let mut scene = Scene::new(200.0, 60.0);
    scene.commands.push(SceneCommand::DrawGlyphRun {
        x: 10.0,
        y: 40.0,
        font_id,
        font_size: 32.0,
        color: Color::srgb(0, 0, 0, 255),
        stroke_color: None,
        stroke_width: None,
        link: Some("https://zenith.example/".to_owned()),
        selectable: true,
        source_node_id: None,
        glyphs: vec![SceneGlyph {
            glyph_id: 36,
            dx: 0.0,
            dy: 0.0,
            text: "A".to_owned(),
        }],
    });
    let assets = BytesAssetProvider::new();
    let plan = font::build_plan(
        &font::collect_usage(std::slice::from_ref(&scene)),
        &fonts,
        true,
    );
    let (_, res) = translate(&scene, &fonts, &assets, &plan);
    let link = res.links.first().expect("one link");
    let want = [
        link.x0 * 0.75,
        (60.0 - link.y1) * 0.75,
        link.x1 * 0.75,
        (60.0 - link.y0) * 0.75,
    ];
    let text = String::from_utf8_lossy(&render_pdf(&scene, &fonts, &assets)).into_owned();
    assert_close(&array(&text, "Rect"), &want);
    // The text still draws as selectable text in px under the page transform.
    assert!(text.contains("/Type0"), "the run must embed a font");
    assert!(
        text.contains("Tj") || text.contains("TJ"),
        "the run must show text"
    );
}

#[test]
fn link_rect_tracks_push_scale_translate_and_rotation() {
    let fonts = default_provider();
    let font_id = fonts.all_faces().first().expect("bundled face").id.clone();
    let glyph_cmd = |link: &str| SceneCommand::DrawGlyphRun {
        x: 10.0,
        y: 40.0,
        font_id: font_id.clone(),
        font_size: 32.0,
        color: Color::srgb(0, 0, 0, 255),
        stroke_color: None,
        stroke_width: None,
        link: Some(link.to_owned()),
        selectable: true,
        source_node_id: None,
        glyphs: vec![SceneGlyph {
            glyph_id: 36,
            dx: 0.0,
            dy: 0.0,
            text: "A".to_owned(),
        }],
    };
    let assets = BytesAssetProvider::new();

    // Baseline untransformed link box on a 200x60 page.
    let mut base = Scene::new(200.0, 60.0);
    base.commands.push(glyph_cmd("https://zenith.example/base"));
    let base_plan = font::build_plan(
        &font::collect_usage(std::slice::from_ref(&base)),
        &fonts,
        true,
    );
    let (_, base_res) = translate(&base, &fonts, &assets, &base_plan);
    let b = base_res.links.first().expect("baseline link");

    // Imposed spread right-hand panel: PushScaleTranslate(1, 1, 200, 0) on a 400x60 sheet.
    let mut spread = Scene::new(400.0, 60.0);
    spread.commands.push(SceneCommand::PushScaleTranslate {
        sx: 1.0,
        sy: 1.0,
        tx: 200.0,
        ty: 0.0,
    });
    spread
        .commands
        .push(glyph_cmd("https://zenith.example/right"));
    spread.commands.push(SceneCommand::PopTransform);
    let spread_text = String::from_utf8_lossy(&render_pdf(&spread, &fonts, &assets)).into_owned();
    let want_spread = [
        (b.x0 + 200.0) * 0.75,
        (60.0 - b.y1) * 0.75,
        (b.x1 + 200.0) * 0.75,
        (60.0 - b.y0) * 0.75,
    ];
    assert_close(&array(&spread_text, "Rect"), &want_spread);

    // Duplex long-edge 180 deg rotation around sheet center (200, 30) composed with panel shift.
    let mut duplex = Scene::new(400.0, 60.0);
    duplex.commands.push(SceneCommand::PushTransform {
        angle_deg: 180.0,
        cx: 200.0,
        cy: 30.0,
    });
    duplex.commands.push(SceneCommand::PushScaleTranslate {
        sx: 1.0,
        sy: 1.0,
        tx: 200.0,
        ty: 0.0,
    });
    duplex
        .commands
        .push(glyph_cmd("https://zenith.example/duplex"));
    duplex.commands.push(SceneCommand::PopTransform);
    duplex.commands.push(SceneCommand::PopTransform);
    let duplex_text = String::from_utf8_lossy(&render_pdf(&duplex, &fonts, &assets)).into_owned();
    // 180 deg around (200, 30) maps (x + 200, y) -> (400 - (x + 200), 60 - y) = (200 - x, 60 - y).
    let want_duplex = [
        (200.0 - b.x1) * 0.75,
        b.y0 * 0.75,
        (200.0 - b.x0) * 0.75,
        b.y1 * 0.75,
    ];
    assert_close(&array(&duplex_text, "Rect"), &want_duplex);
}
