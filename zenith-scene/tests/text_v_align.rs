//! Integration tests for `text` node `v-align` attribute.
//!
//! Verifies that:
//! - `v-align="top"` (default) leaves `text_y` unchanged.
//! - `v-align="middle"` offsets the text block to the vertical center of its box.
//! - `v-align="bottom"` offsets the text block to the box bottom.
//! - A text node WITHOUT `v-align` produces the same scene as one with `v-align="top"`.
//! - Round-trip: parse → format → parse preserves `v_align`; absent stays absent.

mod common;
use common::parse;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::SceneCommand;

// ── Helpers ─────────────────────────────────────────────────────────────

/// Compile a single-line text node with the given `v-align` attribute string (or
/// `None` for absent), a tall box, and a single-line span. Returns the minimum
/// glyph-run baseline y across all emitted `DrawGlyphRun` commands.
fn min_glyph_y(v_align_attr: Option<&str>) -> f64 {
    let v_attr = v_align_attr.map_or(String::new(), |v| format!(" v-align=\"{v}\""));
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.va" name="VA"
  tokens format="zenith-token-v1" {{}}
  styles {{}}
  document id="doc.va" title="VA" {{
    page id="page.va" w=(px)800 h=(px)600 {{
      text id="t1" x=(px)100 y=(px)50 w=(px)400 h=(px)300 font-size=(px)20{v_attr} {{
        span "Hello"
      }}
    }}
  }}
}}
"##
    );
    let doc = parse(&src);
    let result = compile(&doc, &default_provider());
    result
        .scene
        .commands
        .iter()
        .filter_map(|c| {
            if let SceneCommand::DrawGlyphRun { y, .. } = c {
                Some(*y)
            } else {
                None
            }
        })
        .fold(f64::INFINITY, f64::min)
}

// ── Tests ────────────────────────────────────────────────────────────────

/// Default (no `v-align` attribute): glyph baseline is near the box top — the
/// same as `v-align="top"`.
#[test]
fn text_v_align_absent_is_top() {
    let y_absent = min_glyph_y(None);
    let y_top = min_glyph_y(Some("top"));
    assert!(
        (y_absent - y_top).abs() < 1.0,
        "absent v-align must match top: absent={y_absent}, top={y_top}"
    );
}

/// `v-align="top"` positions the text at the box top (no offset).
/// The first baseline must be close to `y + ascent` (≈ y=70 for font-size=20,
/// box y=50). We assert it's in the upper half of the 300px box.
#[test]
fn text_v_align_top_sits_near_box_top() {
    let y = min_glyph_y(Some("top"));
    // box y = 50, box h = 300. Upper half ends at 50+150=200.
    assert!(
        y < 200.0,
        "v-align=top baseline must be in the upper half of the box (< 200); got {y}"
    );
}

/// `v-align="middle"` positions the text block in the vertical center of the box.
/// The first baseline must be strictly below the `top` baseline.
#[test]
fn text_v_align_middle_is_below_top() {
    let y_top = min_glyph_y(Some("top"));
    let y_mid = min_glyph_y(Some("middle"));
    assert!(
        y_mid > y_top + 1.0,
        "v-align=middle baseline must be strictly below top: top={y_top}, middle={y_mid}"
    );
}

/// `v-align="bottom"` positions the text block at the box bottom.
/// Its baseline must be strictly below the `middle` baseline.
#[test]
fn text_v_align_bottom_is_below_middle() {
    let y_mid = min_glyph_y(Some("middle"));
    let y_bot = min_glyph_y(Some("bottom"));
    assert!(
        y_bot > y_mid + 1.0,
        "v-align=bottom baseline must be below middle: middle={y_mid}, bottom={y_bot}"
    );
}

/// `v-align="bottom"` baseline must be in the lower half of the box.
/// box y=50, box_h=300, lower half starts at 50+150=200.
#[test]
fn text_v_align_bottom_sits_near_box_bottom() {
    let y = min_glyph_y(Some("bottom"));
    assert!(
        y > 200.0,
        "v-align=bottom baseline must be in the lower half of the box (> 200); got {y}"
    );
}

/// `v-align="middle"` and `v-align="bottom"` offsets sum correctly:
/// `middle_offset ≈ bottom_offset / 2` (within a pixel for rounding).
/// This verifies the arithmetic is consistent with shape's implementation.
#[test]
fn text_v_align_middle_is_halfway_to_bottom() {
    let y_top = min_glyph_y(Some("top"));
    let y_mid = min_glyph_y(Some("middle"));
    let y_bot = min_glyph_y(Some("bottom"));
    let offset_mid = y_mid - y_top;
    let offset_bot = y_bot - y_top;
    let expected_mid = offset_bot / 2.0;
    assert!(
        (offset_mid - expected_mid).abs() < 1.5,
        "middle offset ({offset_mid}) must be ≈ bottom_offset/2 ({expected_mid})"
    );
}

/// Byte-identical when absent: the scene produced by a text node WITHOUT
/// `v-align` must have the same first glyph-run y as one with `v-align="top"`.
#[test]
fn text_v_align_absent_byte_identical_to_top() {
    // Compare entire command streams: absent and top must emit the same glyphs
    // at the same positions.
    fn glyph_ys(v_align: Option<&str>) -> Vec<f64> {
        let v_attr = v_align.map_or(String::new(), |v| format!(" v-align=\"{v}\""));
        let src = format!(
            r##"zenith version=1 {{
  project id="proj.va2" name="VA2"
  tokens format="zenith-token-v1" {{}}
  styles {{}}
  document id="doc.va2" title="VA2" {{
    page id="page.va2" w=(px)800 h=(px)600 {{
      text id="t2" x=(px)100 y=(px)50 w=(px)400 h=(px)300 font-size=(px)20{v_attr} {{
        span "Byte identical check"
      }}
    }}
  }}
}}
"##
        );
        let doc = parse(&src);
        let result = compile(&doc, &default_provider());
        result
            .scene
            .commands
            .iter()
            .filter_map(|c| {
                if let SceneCommand::DrawGlyphRun { y, .. } = c {
                    Some(*y)
                } else {
                    None
                }
            })
            .collect()
    }

    let ys_absent = glyph_ys(None);
    let ys_top = glyph_ys(Some("top"));
    assert_eq!(
        ys_absent, ys_top,
        "absent v-align must produce identical glyph-y positions as v-align=top"
    );
}

// ── Round-trip tests ─────────────────────────────────────────────────────

/// Round-trip: `v-align="middle"` survives parse → format → parse.
#[test]
fn text_v_align_round_trips() {
    use zenith_core::format::format_document;
    use zenith_core::{KdlAdapter, KdlSource, Node};

    let src = r##"zenith version=1 {
  project id="proj.vart" name="VART"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.vart" title="VART" {
    page id="p1" w=(px)400 h=(px)400 {
      text id="t1" x=(px)10 y=(px)10 w=(px)200 h=(px)300 v-align="middle" {
        span "Round-trip"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let out = format_document(&doc).expect("format");
    let text = String::from_utf8(out).unwrap();

    // The formatter must emit v-align on the text line.
    let text_line = text
        .lines()
        .find(|l| l.trim_start().starts_with("text"))
        .expect("must find text line");
    assert!(
        text_line.contains("v-align=\"middle\""),
        "formatted text line must contain v-align=\"middle\"; got: {text_line:?}"
    );

    // Re-parse: the v_align field must be Some("middle").
    let doc2 = adapter.parse(text.as_bytes()).expect("re-parse");
    match &doc2.body.pages[0].children[0] {
        Node::Text(t) => assert_eq!(
            t.v_align.as_deref(),
            Some("middle"),
            "v_align must survive the format round-trip"
        ),
        other => panic!("expected Text, got {other:?}"),
    }
}

/// Round-trip: a text node WITHOUT `v-align` must NOT have `v-align` emitted
/// in the formatted output (byte-identical: no attribute added).
#[test]
fn text_v_align_absent_not_emitted() {
    use zenith_core::format::format_document;
    use zenith_core::{KdlAdapter, KdlSource};

    let src = r##"zenith version=1 {
  project id="proj.vane" name="VANE"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.vane" title="VANE" {
    page id="p1" w=(px)400 h=(px)400 {
      text id="t1" x=(px)10 y=(px)10 w=(px)200 h=(px)200 {
        span "No v-align here"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let out = format_document(&doc).expect("format");
    let text = String::from_utf8(out).unwrap();

    let text_line = text
        .lines()
        .find(|l| l.trim_start().starts_with("text"))
        .expect("must find text line");
    assert!(
        !text_line.contains("v-align"),
        "text without v-align must NOT emit v-align attribute; got: {text_line:?}"
    );
}
