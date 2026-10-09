//! Auto-layout of content-sized children: hugging text (natural width, wrap at
//! the cap, height at the final width).

mod common;
use common::parse;
#[path = "common/fill_rects.rs"]
mod fill_rects;
use fill_rects::fill_rects;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::SceneCommand;
use zenith_scene::layout_boxes;

fn doc(children: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.lt" name="LT"
  tokens format="zenith-token-v1" {{
    token id="color.k" type="color" value="#000000"
  }}
  styles {{}}
  document id="doc.lt" title="LT" {{
    page id="p" w=(px)800 h=(px)600 {{
{children}
    }}
  }}
}}
"##
    )
}

fn text_box(src: &str) -> (f64, f64, f64, f64) {
    let m = layout_boxes(&parse(src), 0, &default_provider());
    let b = m.get("t").expect("text box");
    (b.x, b.y, b.w, b.h)
}

#[test]
fn hugging_text_takes_its_natural_width() {
    let (_, _, w, h) = text_box(&doc(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)600 layout="row" {
        text id="t" font-size=(px)20 fill=(token)"color.k" {
          span "Short"
        }
      }"#,
    ));
    assert!(w > 20.0 && w < 120.0, "natural width {w}");
    assert!(h > 15.0 && h < 40.0, "one line {h}");
}

#[test]
fn hugging_text_wraps_at_the_cap() {
    let long = "A long sentence that cannot fit on one line inside a narrow frame";
    let (_, _, w_one, h_one) = text_box(&doc(&format!(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)2000 layout="row" {{
        text id="t" font-size=(px)20 fill=(token)"color.k" {{
          span "{long}"
        }}
      }}"#
    )));
    let (_, _, w, h) = text_box(&doc(&format!(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)150 layout="row" {{
        text id="t" font-size=(px)20 fill=(token)"color.k" {{
          span "{long}"
        }}
      }}"#
    )));
    assert!(w_one > 150.0, "natural width {w_one}");
    assert_eq!(w, 150.0);
    assert!(h > 2.0 * h_one, "wrapped height {h} vs one line {h_one}");
}

#[test]
fn hugging_text_renders_unclipped_and_the_next_child_follows() {
    let src = doc(
        r#"      frame id="f" x=(px)100 y=(px)100 w=(px)300 layout="column" gap=(px)8 {
        text id="t" font-size=(px)24 fill=(token)"color.k" {
          span "Heading"
        }
        rect id="r" h=(px)10 fill=(token)"color.k"
      }"#,
    );
    let (_, ty, _, th) = text_box(&src);
    let result = compile(&parse(&src), &default_provider());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        fill_rects(&result),
        vec![(100.0, ty + th + 8.0, 300.0, 10.0)]
    );
    assert!(
        !result
            .scene
            .commands
            .iter()
            .any(|c| matches!(c, SceneCommand::PushClip { w, .. } if *w == 300.0)),
        "a hugging text box draws no overflow clip"
    );
}

#[test]
fn text_fill_width_then_height_at_that_width() {
    let long = "Words that wrap once the fill width is known and not before then";
    let src = doc(&format!(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)400 layout="row" gap=(px)10 align="start" {{
        rect id="side" w=(px)240 h=(px)10 fill=(token)"color.k"
        text id="t" w="fill" font-size=(px)18 fill=(token)"color.k" {{
          span "{long}"
        }}
      }}"#
    ));
    let (x, _, w, h) = text_box(&src);
    assert_eq!((x, w), (250.0, 150.0));
    assert!(h > 30.0, "wrapped at 150px: {h}");
}

#[test]
fn hugging_code_width_is_the_widest_line_advance() {
    let src = r##"zenith version=1 {
  project id="proj.lt" name="LT"
  tokens format="zenith-token-v1" {
    token id="color.k" type="color" value="#000000"
    token id="font.mono" type="fontFamily" value="Noto Sans Mono"
  }
  styles {}
  document id="doc.lt" title="LT" {
    page id="p" w=(px)800 h=(px)600 {
      frame id="f" x=(px)0 y=(px)0 layout="column" align="start" {
        code id="c" fill=(token)"color.k" {
          content "fn a() {}\nlet widest_line = 1;\nx"
        }
        text id="t" font-family=(token)"font.mono" font-size=(px)14 fill=(token)"color.k" {
          span "let widest_line = 1;"
        }
      }
    }
  }
}
"##;
    let m = layout_boxes(&parse(src), 0, &default_provider());
    let code = m.get("c").expect("code box");
    let text = m.get("t").expect("text box");
    // Both use the advance width of the same shaped line (f32 run advance
    // vs summed glyph advances).
    assert!((code.w - text.w).abs() < 1e-3, "{} vs {}", code.w, text.w);
    assert!(code.w > 100.0, "{}", code.w);
}
