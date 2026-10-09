//! Viewport renders: `doc.render` and `gesture.preview` with
//! `viewport {x, y, w, h}` render one window of the page at any scale.

use serde_json::json;
use zenith_editor::{ImageRegion, hex_sha256};

use crate::common::{Driver, doc};

const RECT: &str = r#"      rect id="r" x=(px)40 y=(px)50 w=(px)100 h=(px)60 fill=(token)"color.ink" rotate=(deg)12"#;

#[test]
fn whole_page_viewport_equals_the_page_render() {
    let mut d = Driver::example("gradient.zen");
    let page = d.outcome("doc.render", json!({ "scale": 1.5 }));
    let full = d.outcome(
        "doc.render",
        json!({ "scale": 1.5, "viewport": { "x": 0, "y": 0, "w": 400, "h": 300 } }),
    );
    let page_png = page.image.expect("page png");
    let full_png = full.image.expect("viewport png");
    assert_eq!(page_png.png, full_png.png);
    assert_eq!(page_png.region, None);
    assert_eq!(
        full_png.region,
        Some(ImageRegion {
            x: 0,
            y: 0,
            scale: 1.5,
            device_width: 600,
            device_height: 450,
            page_width: 400.0,
            page_height: 300.0,
        })
    );
    let reply = page.result.expect("reply");
    assert!(
        reply.get("rect").is_none(),
        "no window fields without viewport"
    );
}

#[test]
fn viewport_reply_names_the_snapped_window() {
    let mut d = Driver::example("gradient.zen");
    let out = d.outcome(
        "doc.render",
        json!({ "scale": 7.25, "viewport": { "x": 101.3, "y": 40.1, "w": 50.5, "h": 33 } }),
    );
    let reply = out.result.expect("reply");
    let image = out.image.expect("png");
    // floor(101.3 × 7.25) = 734, ceil(151.8 × 7.25) = 1101.
    assert_eq!(
        reply["rect"],
        json!({ "x": 734, "y": 290, "w": 367, "h": 240 })
    );
    assert_eq!(reply["device_size"], json!({ "w": 2900, "h": 2175 }));
    assert_eq!(reply["page_size"], json!({ "w": 400.0, "h": 300.0 }));
    assert_eq!(reply["scale"], 7.25);
    assert_eq!(reply["width"], 367);
    assert_eq!(reply["height"], 240);
    assert_eq!(reply["view"]["x"], 734.0 / 7.25);
    assert_eq!(reply["sha256"], hex_sha256(&image.png));
    assert_eq!((image.width, image.height), (367, 240));
    let again = d.outcome(
        "doc.render",
        json!({ "scale": 7.25, "viewport": { "x": 101.3, "y": 40.1, "w": 50.5, "h": 33 } }),
    );
    assert_eq!(again.image.expect("png").png, image.png, "deterministic");
}

#[test]
fn viewport_limits_have_codes() {
    let mut d = Driver::open(&doc(RECT));
    let window = json!({ "x": 0, "y": 0, "w": 400, "h": 300 });
    let code = |d: &mut Driver, params: serde_json::Value| d.err("doc.render", params).code;
    assert_eq!(
        code(&mut d, json!({ "scale": 30, "viewport": window })),
        "render.region_too_large"
    );
    assert_eq!(
        code(&mut d, json!({ "scale": 0, "viewport": window })),
        "render.invalid_scale"
    );
    assert_eq!(
        code(&mut d, json!({ "scale": -2, "viewport": window })),
        "render.invalid_scale"
    );
    assert_eq!(
        code(&mut d, json!({ "scale": 1e9, "viewport": window })),
        "render.scale_too_large"
    );
    assert_eq!(
        code(
            &mut d,
            json!({ "viewport": { "x": 0, "y": 0, "w": 0, "h": 10 } })
        ),
        "render.invalid_viewport"
    );
    assert_eq!(
        code(
            &mut d,
            json!({ "viewport": { "x": 500, "y": 0, "w": 10, "h": 10 } })
        ),
        "render.invalid_viewport"
    );
    assert_eq!(
        code(
            &mut d,
            json!({ "viewport": { "x": 0, "y": 0, "w": 10, "h": 10, "z": 1 } })
        ),
        "editor.invalid_params"
    );
    // Without a viewport the whole-page cap still holds.
    assert_eq!(code(&mut d, json!({ "scale": 9 })), "render.invalid_scale");
    // A small window renders far past it.
    let out = d.outcome(
        "doc.render",
        json!({ "scale": 64, "viewport": { "x": 40, "y": 50, "w": 20, "h": 20 } }),
    );
    let image = out.image.expect("png");
    assert_eq!((image.width, image.height), (1280, 1280));
}

#[test]
fn stale_text_renders_the_last_valid_window() {
    let text = doc(RECT);
    let mut d = Driver::open(&text);
    let params = json!({ "scale": 3, "viewport": { "x": 30, "y": 40, "w": 80, "h": 50 } });
    let valid = d.outcome("doc.render", params.clone());
    let first = valid.image.expect("png");
    let broken = text.replace("(token)\"color.ink\"", "(token)\"color.missing\"");
    d.ok("buffer.set", json!({ "text": broken }));
    let stale = d.outcome("doc.render", params);
    assert_eq!(stale.result.expect("reply")["stale"], true);
    assert_eq!(stale.image.expect("png").png, first.png);
}

#[test]
fn gesture_preview_renders_the_window() {
    let mut d = Driver::open(&doc(RECT));
    let out = d.outcome(
        "gesture.preview",
        json!({
            "node": "r", "dx": 10, "dy": 5, "scale": 6,
            "viewport": { "x": 30, "y": 40, "w": 120, "h": 90 }
        }),
    );
    let reply = out.result.expect("preview");
    let image = out.image.expect("png");
    assert_eq!(
        reply["rect"],
        json!({ "x": 180, "y": 240, "w": 720, "h": 540 })
    );
    assert_eq!(reply["sha256"], hex_sha256(&image.png));
    assert_eq!(
        image.region.map(|r| (r.x, r.y, r.scale)),
        Some((180, 240, 6.0))
    );
    let e = d.err(
        "gesture.preview",
        json!({ "node": "r", "dx": 1, "scale": 0, "viewport": { "x": 0, "y": 0, "w": 9, "h": 9 } }),
    );
    assert_eq!(e.code, "render.invalid_scale");
}
