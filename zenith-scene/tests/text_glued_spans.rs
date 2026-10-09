//! Integration tests for the no-whitespace span-boundary "glue" fix on the WRAP
//! path.
//!
//! When two spans are source-adjacent with NO whitespace between them (e.g. a
//! bold `span "24%"` immediately followed by a plain `span ","`), the last word
//! of the first span and the first word of the second span were contiguous text
//! and must render FLUSH — no spurious inter-word space ("24%," not "24% ,").
//! Spans separated by REAL whitespace keep the normal inter-word gap, unchanged.
//!
//! These exercise the WRAP path (the box is narrow enough that the text overflows
//! and is re-tokenised into per-word `WordToken`s), which is where the per-word
//! packer inserted the spurious space. The fast single-line path shapes each span
//! whole and is unaffected.

mod common;
use common::parse;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::{Paint, SceneCommand};

/// Build a two-span text node on the WRAP path. The first span is bold `"24%"`;
/// the second span's text is `joiner` (either `","` for the glued case or `" ,"`
/// with a leading space for the control), followed by enough words to force the
/// node to overflow its narrow box and take the wrapping path. The bold + comma
/// pair therefore lands at the start of line 0.
fn glued_doc_src(joiner_plus_tail: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.glue" name="GLUE"
  tokens format="zenith-token-v1" {{
token id="color.ink" type="color"      value="#111827"
token id="font.body" type="fontFamily" value="Noto Sans"
token id="size.body" type="dimension"  value=(px)24
  }}
  styles {{}}
  document id="doc.glue" title="GLUE" {{
page id="page.glue" w=(px)600 h=(px)400 {{
  text id="t.glue" x=(px)10 y=(px)20 w=(px)200 h=(px)300 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {{
    span "24%" font-weight=700
    span "{joiner_plus_tail}"
  }}
}}
  }}
}}"##
    )
}

/// `(x, y, font_id, glyph_count)` for every DrawGlyphRun, in command order.
fn runs(src: &str) -> Vec<(f64, f64, String, usize)> {
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.code != "scene.text_unshaped"),
        "no text_unshaped diagnostics expected; got: {:?}",
        result.diagnostics
    );
    result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::DrawGlyphRun {
                x,
                y,
                font_id,
                glyphs,
                ..
            } => Some((*x, *y, font_id.clone(), glyphs.len())),
            _ => None,
        })
        .collect()
}

/// The x-origin of the comma run: the first SINGLE-glyph run (the lone `,`) on the
/// first baseline that follows the bold `"24%"` run. The bold "24%" run is the
/// first run at node x; the comma is the next run on the same baseline.
fn comma_run_x(src: &str) -> f64 {
    let all = runs(src);
    let (bold_x, bold_y, _, _) = all.first().cloned().expect("at least the bold run");
    // The bold run begins at the node x origin (start-aligned line 0).
    assert!(
        (bold_x - 10.0).abs() < 1e-6,
        "bold '24%' run must start at node x (10); got {bold_x}"
    );
    // The comma is the next single-glyph run on the SAME baseline as the bold run.
    all.iter()
        .skip(1)
        .find(|(_, y, _, n)| (*y - bold_y).abs() < 1e-6 && *n == 1)
        .map(|(x, _, _, _)| *x)
        .expect("a single-glyph comma run on the first baseline")
}

/// GLUED: `span "24%"` immediately followed by `span ","` (no whitespace in the
/// source) must place the comma FLUSH against "24%" — its x equals the bold run's
/// end. The CONTROL (a leading space before the comma) inserts a real inter-word
/// space, so its comma sits one `space_advance` further right. We assert the
/// glued comma is strictly LEFT of the control comma (the suppressed space).
#[test]
fn glued_comma_is_flush_control_has_gap() {
    // Same tail in both so wrapping geometry is identical up to the joiner.
    let tail = "the quick brown fox jumps over the lazy dog runs far away today";
    let glued = comma_run_x(&glued_doc_src(&format!(", {tail}")));
    let control = comma_run_x(&glued_doc_src(&format!(" , {tail}")));
    assert!(
        control > glued,
        "control (space-separated) comma must sit right of the glued comma: \
         glued={glued}, control={control}"
    );
}

/// The glued comma follows the bold `"24%"` run on the SAME line (no line break
/// was forced) and sits to its right. The differential test above proves the
/// inter-word space was actually removed relative to the space-separated control;
/// this test pins down that the comma stays on line 0 directly after the bold run.
#[test]
fn glued_comma_has_no_leading_space() {
    let tail = "the quick brown fox jumps over the lazy dog runs far away today";
    let glued_runs = runs(&glued_doc_src(&format!(", {tail}")));
    let (bold_x, bold_y, _, _) = glued_runs.first().cloned().expect("bold run");
    // The comma run on line 0 (the first single-glyph run after the bold run).
    let comma_x = glued_runs
        .iter()
        .skip(1)
        .find(|(_, y, _, n)| (*y - bold_y).abs() < 1e-6 && *n == 1)
        .map(|(x, _, _, _)| *x)
        .expect("comma run on line 0");
    // The comma follows "24%" on the same line and sits to its right; the
    // differential test above proves the inter-word space was removed.
    assert!(
        comma_x > bold_x,
        "comma must follow the bold '24%' run; got comma_x={comma_x}, bold_x={bold_x}"
    );
}

/// Defect 2: a multi-word highlighted run on the WRAP path coalesces into ONE
/// continuous FillRect spanning the whole run (including the inter-word spaces),
/// instead of one rect per word. A three-word highlight that lands on a single
/// line must emit FEWER highlight FillRects than it has words.
#[test]
fn multi_word_highlight_coalesces_into_one_rect() {
    // color.mark = #FFFF00 (bright yellow). The highlighted span holds three
    // words; the box is wide enough to keep them on one line but the node overall
    // overflows (long trailing span) so the WRAP path runs.
    let src = r##"zenith version=1 {
  project id="proj.hlc" name="HLC"
  tokens format="zenith-token-v1" {
token id="color.ink"  type="color"      value="#111827"
token id="color.mark" type="color"      value="#FFFF00"
token id="font.body"  type="fontFamily" value="Noto Sans"
token id="size.body"  type="dimension"  value=(px)20
  }
  styles {}
  document id="doc.hlc" title="HLC" {
page id="page.hlc" w=(px)900 h=(px)400 {
  text id="t.hlc" x=(px)10 y=(px)20 w=(px)480 h=(px)300 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
    span "best quarter yet" highlight=(token)"color.mark"
    span " and the rest of this sentence keeps going so the node overflows"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.code != "scene.text_unshaped"),
        "no text_unshaped diagnostics; got: {:?}",
        result.diagnostics
    );

    // Count yellow (#FFFF00) highlight FillRects.
    let yellow_rects: Vec<&SceneCommand> = result
        .scene
        .commands
        .iter()
        .filter(|c| {
            matches!(
                c,
                SceneCommand::FillRect {
                    paint: Paint::Solid { color },
                    ..
                } if color.r == 0xFF && color.g == 0xFF && color.b == 0x00
            )
        })
        .collect();

    // The highlighted span is THREE words ("best", "quarter", "yet"). Before the
    // fix this produced three rects; coalesced it must produce FEWER than three
    // (one per line the run occupies — here a single line → one rect).
    assert!(
        !yellow_rects.is_empty(),
        "expected at least one highlight FillRect"
    );
    assert!(
        yellow_rects.len() < 3,
        "multi-word highlight must coalesce into fewer rects than words (3); got {}",
        yellow_rects.len()
    );

    // The single coalesced rect must be WIDER than any single word — it spans the
    // whole "best quarter yet" run including the inter-word spaces.
    let widest = yellow_rects
        .iter()
        .filter_map(|c| match c {
            SceneCommand::FillRect { w, .. } => Some(*w),
            _ => None,
        })
        .fold(0.0_f64, f64::max);
    assert!(
        widest > 0.0,
        "coalesced highlight rect must have positive width; got {widest}"
    );
}
