//! `overflow` modes of a `text` node: the clip bracket and the fit suggestion.
//!
//! `clip` (also the default) clips overflowing ink at the box edge and warns.
//! `visible` paints past the box silently. `fit`/`autofit` keep their error.
//! Content that fits emits no clip bracket in any mode.

mod common;
use common::parse;
use zenith_core::{Diagnostic, Severity, default_provider};
use zenith_scene::CompileResult;
use zenith_scene::compile;
use zenith_scene::ir::SceneCommand;

const TITLE: &str = "Quarterly revenue grew across every region this year";

/// A one-page document with one text node. `attrs` carries the box and the
/// overflow attribute; the font size comes from the `size.title` token.
fn doc(attrs: &str, font_px: u32, body: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.ov" name="OV"
  tokens format="zenith-token-v1" {{
token id="color.ink"      type="color"      value="#111827"
token id="font.body"      type="fontFamily" value="Noto Sans"
token id="size.title"     type="dimension"  value=(px){font_px}
token id="size.title.min" type="dimension"  value=(px)40
  }}
  styles {{}}
  document id="doc.ov" title="OV" {{
page id="page.ov" w=(px)800 h=(px)800 {{
  text id="title" x=(px)20 y=(px)30 {attrs} fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.title" font-size-min=(token)"size.title.min" {{
    span "{body}"
  }}
}}
  }}
}}
"##
    )
}

fn run(src: &str) -> CompileResult {
    compile(&parse(src), &default_provider())
}

fn overflow_diags(r: &CompileResult) -> Vec<&Diagnostic> {
    r.diagnostics
        .iter()
        .filter(|d| d.code == "text.overflow" || d.code == "text.fit_failed")
        .collect()
}

/// Is this a page-level clip (anchored at the page origin)? Every text box in
/// these fixtures sits away from the origin.
fn is_page_clip(c: &SceneCommand) -> bool {
    matches!(c, SceneCommand::PushClip { x, y, .. } if *x == 0.0 && *y == 0.0)
}

/// The text `PushClip` rects in the scene (page clips excluded), in order.
fn clips(r: &CompileResult) -> Vec<(f64, f64, f64, f64)> {
    r.scene
        .commands
        .iter()
        .filter(|c| !is_page_clip(c))
        .filter_map(|c| match c {
            SceneCommand::PushClip { x, y, w, h } => Some((*x, *y, *w, *h)),
            _ => None,
        })
        .collect()
}

/// Every glyph run sits between the first `PushClip` and its `PopClip`.
fn glyphs_inside_clip(r: &CompileResult) -> bool {
    let cmds = &r.scene.commands;
    let Some(open) = cmds
        .iter()
        .position(|c| matches!(c, SceneCommand::PushClip { .. }) && !is_page_clip(c))
    else {
        return false;
    };
    // The text clip's PopClip is the first one after it (text emits no nested clip).
    let Some(close) = cmds
        .iter()
        .skip(open)
        .position(|c| matches!(c, SceneCommand::PopClip))
        .map(|i| i + open)
    else {
        return false;
    };
    let runs: Vec<usize> = cmds
        .iter()
        .enumerate()
        .filter(|(_, c)| matches!(c, SceneCommand::DrawGlyphRun { .. }))
        .map(|(i, _)| i)
        .collect();
    !runs.is_empty() && runs.iter().all(|&i| open < i && i < close)
}

/// Parse `set h=(px)N` and `font-size ≤ Npx` out of a message.
fn suggested(message: &str, key: &str) -> Option<u32> {
    let start = message.find(key)? + key.len();
    let digits: String = message[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

#[test]
fn default_overflow_clips_at_the_box_and_warns() {
    let r = run(&doc("w=(px)400 h=(px)60", 64, TITLE));
    assert_eq!(clips(&r), vec![(20.0, 30.0, 400.0, 60.0)]);
    assert!(
        glyphs_inside_clip(&r),
        "glyph runs must sit inside the clip"
    );
    let d = overflow_diags(&r);
    assert_eq!(d.len(), 1, "{:?}", r.diagnostics);
    assert_eq!(d[0].code, "text.overflow");
    assert_eq!(d[0].severity, Severity::Warning);
    assert_eq!(d[0].subject_id.as_deref(), Some("title"));
    assert!(
        d[0].message.contains("clipped at the box edge"),
        "{}",
        d[0].message
    );
}

#[test]
fn explicit_clip_matches_the_default() {
    let absent = run(&doc("w=(px)400 h=(px)60", 64, TITLE));
    let clip = run(&doc(r#"w=(px)400 h=(px)60 overflow="clip""#, 64, TITLE));
    assert_eq!(absent.scene.commands, clip.scene.commands);
    assert_eq!(clips(&clip).len(), 1);
}

#[test]
fn visible_paints_past_the_box_silently() {
    let r = run(&doc(r#"w=(px)400 h=(px)60 overflow="visible""#, 64, TITLE));
    assert!(clips(&r).is_empty(), "visible must not clip");
    assert!(overflow_diags(&r).is_empty(), "{:?}", r.diagnostics);
}

#[test]
fn fitting_text_emits_no_clip_in_any_mode() {
    let visible = run(&doc(r#"w=(px)300 h=(px)200 overflow="visible""#, 16, "Hi"));
    for mode in ["", r#"overflow="clip""#, r#"overflow="fit""#] {
        let r = run(&doc(&format!("w=(px)300 h=(px)200 {mode}"), 16, "Hi"));
        assert!(clips(&r).is_empty(), "mode {mode:?} must not clip");
        assert!(overflow_diags(&r).is_empty(), "{:?}", r.diagnostics);
        assert_eq!(r.scene.commands, visible.scene.commands, "mode {mode:?}");
    }
}

#[test]
fn fit_errors_without_clipping() {
    let r = run(&doc(r#"w=(px)400 h=(px)60 overflow="fit""#, 64, TITLE));
    assert!(clips(&r).is_empty(), "fit must not clip");
    let d = overflow_diags(&r);
    assert_eq!(d.len(), 1, "{:?}", r.diagnostics);
    assert_eq!(d[0].code, "text.fit_failed");
    assert_eq!(d[0].severity, Severity::Error);
    assert!(
        d[0].message
            .starts_with("text 'title': overflow=\"fit\" failed: "),
        "{}",
        d[0].message
    );
}

/// The suggested height and font size each make the warning go away, and the
/// suggested font size is the LARGEST that does.
#[test]
fn suggestions_fit_the_box() {
    let r = run(&doc("w=(px)400 h=(px)60", 64, TITLE));
    let msg = &overflow_diags(&r)[0].message;
    assert!(msg.contains(" lines at 64px need "), "{msg}");
    assert!(msg.contains("px height in a 60px box"), "{msg}");
    let h = suggested(msg, "set h=(px)").expect("message names a height");
    let fs = suggested(msg, "font-size ≤ ").expect("message names a font size");
    assert!(h > 60, "{msg}");
    assert!(fs < 64, "{msg}");

    let taller = run(&doc(&format!("w=(px)400 h=(px){h}"), 64, TITLE));
    assert!(
        overflow_diags(&taller).is_empty(),
        "h={h}: {:?}",
        taller.diagnostics
    );
    assert!(clips(&taller).is_empty());

    let smaller = run(&doc("w=(px)400 h=(px)60", fs, TITLE));
    assert!(
        overflow_diags(&smaller).is_empty(),
        "fs={fs}: {:?}",
        smaller.diagnostics
    );

    let one_up = run(&doc("w=(px)400 h=(px)60", fs + 1, TITLE));
    assert_eq!(
        overflow_diags(&one_up).len(),
        1,
        "fs={} must still overflow",
        fs + 1
    );
}

/// The `fit` message carries the same fitting font size as the `clip` one.
#[test]
fn fit_and_clip_name_the_same_suggestion() {
    let clip = run(&doc("w=(px)400 h=(px)60", 64, TITLE));
    let fit = run(&doc(r#"w=(px)400 h=(px)60 overflow="fit""#, 64, TITLE));
    let (c, f) = (
        &overflow_diags(&clip)[0].message,
        &overflow_diags(&fit)[0].message,
    );
    assert_eq!(suggested(c, "set h=(px)"), suggested(f, "set h=(px)"));
    assert_eq!(suggested(c, "font-size ≤ "), suggested(f, "font-size ≤ "));
}

/// Autofit that fails at its floor errors, names the floor, and suggests the
/// `font-size-min` that fits; it never clips.
#[test]
fn autofit_floor_failure_names_the_floor_and_a_fitting_min() {
    let r = run(&doc(r#"w=(px)400 h=(px)60 overflow="autofit""#, 64, TITLE));
    assert!(clips(&r).is_empty(), "autofit must not clip");
    let d = overflow_diags(&r);
    assert_eq!(d.len(), 1, "{:?}", r.diagnostics);
    assert_eq!(d[0].code, "text.fit_failed");
    assert!(
        d[0].message
            .starts_with("text 'title': overflow=\"autofit\" failed at its 40px floor: "),
        "{}",
        d[0].message
    );
    let min = suggested(&d[0].message, "font-size-min ≤ ").expect("names a fitting floor");
    assert!(min < 40, "{}", d[0].message);
}

/// The clip rect follows the BOX top, not a v-align offset of the lines.
#[test]
fn clip_rect_is_the_box_under_v_align() {
    let r = run(&doc(r#"w=(px)400 h=(px)60 v-align="bottom""#, 64, TITLE));
    assert_eq!(clips(&r), vec![(20.0, 30.0, 400.0, 60.0)]);
}

#[test]
fn overflow_output_is_deterministic() {
    let src = doc("w=(px)400 h=(px)60", 64, TITLE);
    let (a, b) = (run(&src), run(&src));
    assert_eq!(a.scene.commands, b.scene.commands);
    assert_eq!(a.diagnostics, b.diagnostics);
}

/// The last chain member clips its overflowing remainder in the default mode.
#[test]
fn chain_last_member_clips_by_default() {
    let src = r##"zenith version=1 {
  project id="proj.chc" name="CHC"
  tokens format="zenith-token-v1" {
token id="color.ink" type="color"      value="#111827"
token id="font.body" type="fontFamily" value="Noto Sans"
token id="size.body" type="dimension"  value=(px)24
  }
  styles {}
  document id="doc.chc" title="CHC" {
page id="page.chc" w=(px)600 h=(px)1400 {
  text id="cbox1" x=(px)10 y=(px)0 w=(px)300 h=(px)40 chain="article" fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
    span "Alpha bravo charlie delta echo foxtrot golf hotel india juliet kilo lima mike november oscar papa quebec romeo sierra tango uniform victor whiskey"
  }
  text id="cbox2" x=(px)10 y=(px)1000 w=(px)300 h=(px)40 chain="article" fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
  }
}
  }
}
"##;
    let r = run(src);
    assert_eq!(clips(&r), vec![(10.0, 1000.0, 300.0, 40.0)]);
    let d = overflow_diags(&r);
    assert_eq!(d.len(), 1, "{:?}", r.diagnostics);
    assert_eq!(d[0].code, "text.overflow");
    assert_eq!(d[0].subject_id.as_deref(), Some("cbox2"));
    assert!(
        d[0].message.contains("chain=\"article\""),
        "{}",
        d[0].message
    );

    let visible = run(&src.replace(
        r#"h=(px)40 chain="article" fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
  }"#,
        r#"h=(px)40 overflow="visible" chain="article" fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
  }"#,
    ));
    assert!(clips(&visible).is_empty());
    assert!(
        overflow_diags(&visible).is_empty(),
        "{:?}",
        visible.diagnostics
    );
}

/// A one-line label whose LINE BOX is taller than its box but whose glyph INK
/// fits: no warning, no clip, identical to `overflow="visible"`.
#[test]
fn line_box_taller_than_box_but_ink_fits_does_not_overflow() {
    // At 19px the line box is ~26px; the capitals' ink spans ~14px.
    let tight = run(&doc("w=(px)200 h=(px)24", 19, "HELLO"));
    assert!(clips(&tight).is_empty(), "ink fits: no clip");
    assert!(overflow_diags(&tight).is_empty(), "{:?}", tight.diagnostics);
    let visible = run(&doc(
        r#"w=(px)200 h=(px)24 overflow="visible""#,
        19,
        "HELLO",
    ));
    assert_eq!(tight.scene.commands, visible.scene.commands);
    let fit = run(&doc(r#"w=(px)200 h=(px)24 overflow="fit""#, 19, "HELLO"));
    assert!(overflow_diags(&fit).is_empty(), "{:?}", fit.diagnostics);

    // A box shorter than the ink still clips and warns.
    let short = run(&doc("w=(px)200 h=(px)12", 19, "HELLO"));
    assert_eq!(clips(&short).len(), 1);
    assert_eq!(overflow_diags(&short).len(), 1, "{:?}", short.diagnostics);
}

/// A shape label that overflows reports the SHAPE: its id and the shape `h`
/// (label box + 2 × padding) that fits. Applying that `h` removes the warning.
#[test]
fn shape_label_overflow_names_the_shape_geometry() {
    let shape_doc = |h: u32| {
        format!(
            r##"zenith version=1 {{
  project id="proj.shl" name="SHL"
  tokens format="zenith-token-v1" {{
token id="color.fill" type="color" value="#dbeafe"
token id="color.line" type="color" value="#1e3a8a"
token id="size.stroke" type="dimension" value=(px)2
token id="size.pad" type="dimension" value=(px)8
  }}
  styles {{}}
  document id="doc.shl" title="SHL" {{
page id="page.shl" w=(px)640 h=(px)640 {{
  shape id="s1" x=(px)40 y=(px)40 w=(px)120 h=(px){h} kind="process" fill=(token)"color.fill" stroke=(token)"color.line" stroke-width=(token)"size.stroke" padding=(token)"size.pad" {{
    span "{TITLE}"
  }}
}}
  }}
}}
"##
        )
    };
    let r = run(&shape_doc(40));
    let d = overflow_diags(&r);
    assert_eq!(d.len(), 1, "{:?}", r.diagnostics);
    assert_eq!(d[0].code, "text.overflow");
    assert_eq!(d[0].subject_id.as_deref(), Some("s1"));
    assert!(
        d[0].message
            .starts_with("label of shape 's1': clipped at the box edge: ")
            && d[0].message.contains("px height in a 40px box"),
        "{}",
        d[0].message
    );
    let h = suggested(&d[0].message, "set h=(px)").expect("names the shape h");
    let fixed = run(&shape_doc(h));
    assert!(
        overflow_diags(&fixed).is_empty(),
        "h={h}: {:?}",
        fixed.diagnostics
    );
    let short = run(&shape_doc(h - 2));
    assert_eq!(
        overflow_diags(&short).len(),
        1,
        "h={} must still overflow",
        h - 2
    );
}
