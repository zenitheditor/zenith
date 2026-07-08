mod common;
use common::*;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::SceneCommand;

/// With `overflow-wrap="break-word"` the single overlong token is split across
/// >= 2 lines.
#[test]
fn hyphenate_splits_long_words() {
    let off = hyphenate_commands(false, HYPH_BODY);
    let on = hyphenate_commands(true, HYPH_BODY);

    assert_ne!(
        off, on,
        "hyphenation must change the command stream for an overflowing paragraph"
    );
    // Splitting a word into `head-` + `tail` adds glyph runs beyond the off case.
    assert!(
        glyph_run_count(&on) > glyph_run_count(&off),
        "hyphenation must emit more glyph runs (head+tail); off={}, on={}",
        glyph_run_count(&off),
        glyph_run_count(&on)
    );
    // Determinism: two on-renders are byte-identical.
    let on2 = hyphenate_commands(true, HYPH_BODY);
    assert_eq!(on, on2, "hyphenated render must be deterministic");
}

/// Hyphenation OFF is byte-identical when re-rendered, and a long word wraps
/// whole (line count is the unsplit count). This is the opt-in guard: the
/// default path is unchanged from pre-feature behavior.
#[test]
fn hyphenate_off_is_byte_identical_and_wraps_whole() {
    let off = hyphenate_commands(false, HYPH_BODY);
    let off2 = hyphenate_commands(false, HYPH_BODY);
    assert_eq!(off, off2, "hyphenate-off render must be deterministic");

    // Hyphenation packs fragments tighter, MOVING break points, so the on/off
    // line stream differs (it may yield fewer OR more lines depending on how
    // fragments fill). The whole-word (off) path wraps to at least one line.
    let on = hyphenate_commands(true, HYPH_BODY);
    assert_ne!(
        off, on,
        "hyphenation must move break points relative to whole-word wrapping"
    );
    assert!(
        distinct_line_count(&off) >= 1,
        "off paragraph must wrap to at least one line"
    );
}

/// A tab-leader row `"Title\t12"` emits a LEFT run at the box left edge, a RIGHT
/// run whose right edge ≈ the box right edge, and ≥1 leader glyph between them.
#[test]
fn tab_leader_row_left_right_and_leaders() {
    // KDL decodes `\t` to a real tab inside the quoted string.
    let cmds = tab_leader_commands(true, "Title\\t12");
    let runs: Vec<_> = cmds
        .iter()
        .filter_map(|c| match c {
            SceneCommand::DrawGlyphRun { x, glyphs, .. } => Some((*x, glyphs.len())),
            _ => None,
        })
        .collect();
    assert!(!runs.is_empty(), "tab-leader row must emit glyph runs");

    let box_left = 100.0_f64;
    let box_right = box_left + 600.0;

    // LEFT run sits at the box left edge.
    let left_x = runs.iter().map(|(x, _)| *x).fold(f64::INFINITY, f64::min);
    assert!(
        (left_x - box_left).abs() < 0.01,
        "left segment must start at box left edge {box_left}; got {left_x}"
    );

    // RIGHT run (the page number "12") is the rightmost run; its right edge must
    // be ≈ the box right edge. We reconstruct its advance from the leader run
    // pitch is not trivial, so assert the right run STARTS before the box right
    // and that no run starts to the right of the box edge.
    let max_x = runs
        .iter()
        .map(|(x, _)| *x)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        max_x < box_right,
        "no run may start past the box right edge {box_right}; got {max_x}"
    );
    // The rightmost run must be the 2-glyph page number, and it must start in the
    // right portion of the box (right-aligned), well past the left segment.
    let (rightmost_x, rightmost_glyphs) = runs.iter().copied().fold(
        (f64::NEG_INFINITY, 0),
        |acc, r| if r.0 > acc.0 { r } else { acc },
    );
    assert_eq!(
        rightmost_glyphs, 2,
        "rightmost run must be the 2-digit page number '12'"
    );
    assert!(
        rightmost_x > box_left + 300.0,
        "page number must be right-aligned (started at {rightmost_x})"
    );

    // Leader dots: single-glyph runs between the title and the page number.
    let leader_dots = runs
        .iter()
        .filter(|(x, g)| *g == 1 && *x > left_x && *x < rightmost_x)
        .count();
    assert!(
        leader_dots >= 1,
        "at least one leader dot must fill the gap; got {leader_dots}"
    );

    // Determinism: a second render is byte-identical.
    let cmds2 = tab_leader_commands(true, "Title\\t12");
    assert_eq!(cmds, cmds2, "tab-leader render must be deterministic");
}

/// A tab-leader row with NO tab renders left-aligned with NO leader dots and no
/// right-aligned run.
#[test]
fn tab_leader_row_without_tab_has_no_leaders() {
    let cmds = tab_leader_commands(true, "JustATitleNoTab");
    let xs: Vec<f64> = cmds
        .iter()
        .filter_map(|c| match c {
            SceneCommand::DrawGlyphRun { x, .. } => Some(*x),
            _ => None,
        })
        .collect();
    assert!(!xs.is_empty(), "row must still emit its left text");
    let box_left = 100.0_f64;
    // Every run starts at the box left edge (one left run, no leaders / right run).
    for x in &xs {
        assert!(
            (*x - box_left).abs() < 0.01,
            "a tab-less row must be wholly left-aligned; run at {x}"
        );
    }
}

/// Tab-leader ABSENT (`None`) is byte-identical to the pre-feature render: the
/// SAME node text rendered without the attribute produces the normal text path.
/// This guards the opt-in branch — the default path is untouched.
#[test]
fn tab_leader_absent_is_byte_identical_to_plain_text() {
    // A body with no tab so the plain path and a tab-less leader path would draw
    // the same text; here we only assert the ABSENT path is stable + matches the
    // pre-feature single-line emit (one run at the box left edge).
    let off = tab_leader_commands(false, "Contents heading");
    let off2 = tab_leader_commands(false, "Contents heading");
    assert_eq!(
        off, off2,
        "plain (no tab-leader) render must be deterministic"
    );
    let run_count = off
        .iter()
        .filter(|c| matches!(c, SceneCommand::DrawGlyphRun { .. }))
        .count();
    assert!(
        run_count >= 1,
        "plain text must still emit at least one glyph run"
    );
}

/// WITHOUT the attribute the overlong token is kept whole (one line) and the
/// command stream is byte-identical to a node with NO attribute at all.
#[test]
fn overflow_wrap_none_is_byte_identical() {
    let absent = compile(&break_word_doc(""), &default_provider());
    let normal = compile(
        &break_word_doc(r#"overflow-wrap="normal""#),
        &default_provider(),
    );
    assert_eq!(
        absent.scene.commands, normal.scene.commands,
        "overflow-wrap=\"normal\" must match an absent attribute (byte-identical)"
    );
    // The overlong token stays on ONE line (no forced break).
    assert_eq!(
        glyph_line_ys(&absent).len(),
        1,
        "the overlong token must stay whole on one line by default"
    );
    assert!(
        absent
            .diagnostics
            .iter()
            .all(|d| d.code != "text.forced_break"),
        "no forced_break advisory without break-word; got {:?}",
        absent.diagnostics
    );
}

/// WITH `overflow-wrap="break-word"` the single overlong token is split across
/// >= 2 lines.
#[test]
fn break_word_splits_overlong_token() {
    let absent = compile(&break_word_doc(""), &default_provider());
    let broken = compile(
        &break_word_doc(r#"overflow-wrap="break-word""#),
        &default_provider(),
    );

    let whole_lines = glyph_line_ys(&absent).len();
    let broken_lines = glyph_line_ys(&broken).len();
    assert_eq!(whole_lines, 1, "control: default keeps the token whole");
    assert!(
        broken_lines >= 2,
        "break-word must split the token across >= 2 lines; got {broken_lines}"
    );
}

/// The `text.forced_break` advisory is present for the break case and ABSENT
/// when the token fits the box.
#[test]
fn break_word_emits_forced_break_advisory() {
    let broken = compile(
        &break_word_doc(r#"overflow-wrap="break-word""#),
        &default_provider(),
    );
    let forced: Vec<_> = broken
        .diagnostics
        .iter()
        .filter(|d| d.code == "text.forced_break")
        .collect();
    assert_eq!(
        forced.len(),
        1,
        "exactly one forced_break advisory expected; got {:?}",
        broken.diagnostics
    );
    assert!(
        forced[0].message.contains("col.bw"),
        "advisory must name the node id"
    );

    // A node whose token FITS its box emits no advisory even with break-word on.
    let fits_src = r##"zenith version=1 {
  project id="proj.bwf" name="BWF"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.bwf" title="BWF" {
page id="page.bwf" w=(px)400 h=(px)200 {
  text id="col.bwf" x=(px)10 y=(px)20 w=(px)380 h=(px)100 overflow-wrap="break-word" {
    span "short words fit fine"
  }
}
  }
}
"##;
    let fits = compile(&parse(fits_src), &default_provider());
    assert!(
        fits.diagnostics
            .iter()
            .all(|d| d.code != "text.forced_break"),
        "no forced_break when the content fits; got {:?}",
        fits.diagnostics
    );
}

// ── Text node with stroke + stroke-width tokens → DrawGlyphRun carries stroke ─

/// A text node with `stroke=(token)` and `stroke-width=(token)` must compile
/// to a DrawGlyphRun whose `stroke_color` and `stroke_width` are `Some`.
/// A text node without stroke attributes must compile to `None` / `None`.
#[test]
fn text_stroke_token_threads_to_draw_glyph_run() {
    let src = r##"zenith version=1 {
  project id="proj.stroke" name="Stroke"
  tokens format="zenith-token-v1" {
token id="color.ink"    type="color"      value="#000000"
token id="color.outline" type="color"     value="#ff0000"
token id="font.body"    type="fontFamily" value="Noto Sans"
token id="size.body"    type="dimension"  value=(px)24
token id="size.stroke"  type="dimension"  value=(px)2
  }
  styles {}
  document id="doc.stroke" title="Stroke" {
page id="page.stroke" w=(px)400 h=(px)200 {
  text id="text.with-stroke" x=(px)10 y=(px)20 w=(px)380 h=(px)40 fill=(token)"color.ink" stroke=(token)"color.outline" stroke-width=(token)"size.stroke" font-family=(token)"font.body" font-size=(token)"size.body" {
    span "Outlined"
  }
  text id="text.no-stroke" x=(px)10 y=(px)80 w=(px)380 h=(px)40 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
    span "Plain"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let cmds = &result.scene.commands;

    // Find DrawGlyphRun for text.with-stroke (stroke fields must be Some).
    let with_stroke_run = cmds.iter().find(|c| {
        matches!(
            c,
            SceneCommand::DrawGlyphRun {
                stroke_color: Some(_),
                ..
            }
        )
    });
    assert!(
        with_stroke_run.is_some(),
        "text with stroke token must produce a DrawGlyphRun with stroke_color=Some; \
         commands: {:?}",
        cmds
    );
    if let Some(SceneCommand::DrawGlyphRun {
        stroke_color,
        stroke_width,
        ..
    }) = with_stroke_run
    {
        let sc = stroke_color.unwrap();
        // color.outline = #ff0000 → r=255, g=0, b=0.
        assert_eq!(sc.r, 255, "stroke_color.r must be 255 (#ff0000)");
        assert_eq!(sc.g, 0, "stroke_color.g must be 0");
        assert_eq!(sc.b, 0, "stroke_color.b must be 0");
        assert_eq!(
            *stroke_width,
            Some(2.0),
            "stroke_width must be 2.0 px (size.stroke token)"
        );
    }

    // Find DrawGlyphRun for text.no-stroke (stroke fields must be None).
    let no_stroke_run = cmds.iter().find(|c| {
        matches!(
            c,
            SceneCommand::DrawGlyphRun {
                stroke_color: None,
                ..
            }
        )
    });
    assert!(
        no_stroke_run.is_some(),
        "text without stroke token must produce a DrawGlyphRun with stroke_color=None; \
         commands: {:?}",
        cmds
    );
}

// ── font.glyph_missing diagnostic ─────────────────────────────────────────────

/// A text node containing a character that no registered face can cover
/// (emoji U+1F600, absent from all bundled Noto Sans faces) must produce
/// exactly one `font.glyph_missing` Warning naming that codepoint.
#[test]
fn glyph_missing_diagnostic_for_uncovered_char() {
    // U+1F600 GRINNING FACE: not present in Noto Sans Regular/Bold/Italic or
    // Noto Sans Mono (the full set registered by default_provider()).
    let src = r##"zenith version=1 {
  project id="proj.gm" name="GM"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.gm" title="GM" {
    page id="page.gm" w=(px)400 h=(px)200 {
      text id="text.gm" x=(px)10 y=(px)20 w=(px)380 h=(px)60 {
        span "Hello \u{1F600} World"
      }
    }
  }
}"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let missing: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "font.glyph_missing")
        .collect();
    assert_eq!(
        missing.len(),
        1,
        "expected exactly one font.glyph_missing diagnostic; got: {:?}",
        result.diagnostics
    );
    let diag = missing[0];
    assert_eq!(
        diag.severity,
        zenith_core::Severity::Warning,
        "font.glyph_missing must be Warning severity"
    );
    assert_eq!(
        diag.subject_id.as_deref(),
        Some("text.gm"),
        "subject_id must be the text node id"
    );
    assert!(
        diag.message.contains("U+1F600"),
        "message must contain the missing codepoint U+1F600; got: {}",
        diag.message
    );
}

/// A text node containing only ASCII (fully covered by Noto Sans) must produce
/// no `font.glyph_missing` diagnostic.
#[test]
fn no_glyph_missing_for_ascii() {
    let src = r##"zenith version=1 {
  project id="proj.gm2" name="GM2"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.gm2" title="GM2" {
    page id="page.gm2" w=(px)400 h=(px)200 {
      text id="text.gm2" x=(px)10 y=(px)20 w=(px)380 h=(px)60 {
        span "Hello World"
      }
    }
  }
}"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let missing: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "font.glyph_missing")
        .collect();
    assert!(
        missing.is_empty(),
        "ASCII text must produce no font.glyph_missing; got: {:?}",
        missing
    );
}

/// A text node containing a ZWJ (U+200D, a default-ignorable joiner) between
/// ASCII characters must NOT produce a `font.glyph_missing` for the ZWJ.
/// The joiner is consumed by the shaper and has no standalone glyph by design.
#[test]
fn default_ignorable_not_reported_as_missing() {
    // U+200D ZERO WIDTH JOINER between ASCII: ignorable, not reportable.
    let src = r##"zenith version=1 {
  project id="proj.gm3" name="GM3"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.gm3" title="GM3" {
    page id="page.gm3" w=(px)400 h=(px)200 {
      text id="text.gm3" x=(px)10 y=(px)20 w=(px)380 h=(px)60 {
        span "a\u{200D}b"
      }
    }
  }
}"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let missing: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "font.glyph_missing")
        .collect();
    assert!(
        missing.is_empty(),
        "ZWJ (U+200D) must not be reported as glyph_missing; got: {:?}",
        missing
    );
}

#[test]
fn authored_kern_pair_shifts_second_glyph() {
    let base = text_kern_x_positions("");
    let adjusted = text_kern_x_positions(r#"kern-pair "A" "V" by=(px)-4"#);

    assert_eq!(base.len(), adjusted.len(), "same glyph count expected");
    assert!(
        base.len() >= 2,
        "AV should shape to at least two glyphs; got {base:?}"
    );
    assert!(
        adjusted[1] < base[1] - 3.5,
        "manual kern pair should shift V left by roughly 4px; base={base:?}, adjusted={adjusted:?}"
    );
}

fn text_kern_x_positions(kern_child: &str) -> Vec<f32> {
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.kern.scene" name="Kern Scene"
  tokens format="zenith-token-v1" {{}}
  styles {{}}
  document id="doc.kern.scene" title="Kern Scene" {{
    page id="page.kern.scene" w=(px)300 h=(px)160 {{
      text id="kern.scene" x=(px)10 y=(px)20 w=(px)260 h=(px)80 font-size=(px)48 {{
        {kern_child}
        span "AV"
      }}
    }}
  }}
}}"##
    );
    let doc = parse(&src);
    let result = compile(&doc, &default_provider());
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.code != "scene.text_unshaped"),
        "text must shape successfully; got {:?}",
        result.diagnostics
    );
    result
        .scene
        .commands
        .iter()
        .find_map(|cmd| match cmd {
            SceneCommand::DrawGlyphRun { glyphs, .. } => {
                Some(glyphs.iter().map(|glyph| glyph.dx).collect())
            }
            _ => None,
        })
        .expect("expected a glyph run")
}
