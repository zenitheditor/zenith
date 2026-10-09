mod common;
use common::parse;
#[path = "common/glyph_run_positions.rs"]
mod glyph_run_positions;
use glyph_run_positions::glyph_run_positions;
use zenith_core::{Document, default_provider};
use zenith_scene::compile;
use zenith_scene::ir::SceneCommand;

/// Build a runaround page doc. `extra` is injected before the text node;
/// `exclusion_attr` is appended to the text line.
fn runaround_doc(extra: &str, exclusion_attr: &str) -> Document {
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.ra" name="RA"
  tokens format="zenith-token-v1" {{
  }}
  styles {{}}
  document id="doc.ra" title="RA" {{
    page id="page.ra" w=(px)600 h=(px)600 {{
      {extra}
      text id="body" x=(px)0 y=(px)0 w=(px)400 h=(px)560 font-size=(px)20 {exclusion_attr} {{
        span "The quick brown fox jumps over the lazy dog and then keeps running far beyond the box edge to force wrapping across many lines of body text here"
      }}
    }}
  }}
}}
"##
    );
    parse(&src)
}

/// A wrapping text node WITHOUT `text-exclusion` must emit a command stream
/// identical to the same node with no exclusion attribute — the determinism gate.
#[test]
fn runaround_none_is_byte_identical() {
    // Same body, no rect, no exclusion: the attribute path must be inert.
    let a = compile(&runaround_doc("", ""), &default_provider());
    let b = compile(&runaround_doc("", ""), &default_provider());
    assert_eq!(
        a.scene.commands, b.scene.commands,
        "a node without text-exclusion must be deterministic and unchanged"
    );
    // Sanity: the body actually wrapped (more than one glyph-run line).
    let ys: std::collections::BTreeSet<u64> = glyph_run_positions(&a.scene.commands)
        .iter()
        .map(|(_, y)| y.to_bits())
        .collect();
    assert!(ys.len() > 1, "body must wrap onto multiple lines");
}

/// An exclusion on the LEFT half over the top of the box shifts the affected
/// lines' first glyph to at/after the exclusion's right edge; lines below the
/// exclusion return to the box origin x.
#[test]
fn runaround_left_exclusion_shifts_lines_right() {
    // Rect [0,0,200,120]: left half, top 120px. text_x=0, box_w=400.
    // left_w = 0, right_w = 400-200 = 200 → right segment wins → origin = 200.
    let rect = r##"rect id="ex" x=(px)0 y=(px)0 w=(px)200 h=(px)120 fill="#000000""##;
    let result = compile(
        &runaround_doc(rect, r#"text-exclusion="ex""#),
        &default_provider(),
    );
    let pos = glyph_run_positions(&result.scene.commands);
    assert!(!pos.is_empty(), "text must render");
    // Lines whose baseline falls within the exclusion band start at x >= 200.
    let mut saw_shifted = false;
    let mut saw_returned = false;
    for (x, y) in &pos {
        if *y <= 120.0 {
            assert!(
                *x >= 200.0 - 0.5,
                "a line in the exclusion band must start at/after the rect right edge (200); got x={x} y={y}"
            );
            saw_shifted = true;
        } else if *x < 1.0 {
            saw_returned = true;
        }
    }
    assert!(saw_shifted, "at least one line must be shifted right");
    assert!(
        saw_returned,
        "lines below the exclusion must return to text_x (0)"
    );
}

/// An exclusion on the RIGHT keeps affected lines at origin `text_x` but packs
/// them into the narrower LEFT segment → more line breaks within the band.
#[test]
fn runaround_right_exclusion_narrows_lines() {
    // Rect [250,0,150,120]: right side. left_w = 250, right_w = 400-400 = 0.
    // left >= right and left >= min → origin = text_x (0), width = 250.
    let rect = r##"rect id="ex" x=(px)250 y=(px)0 w=(px)150 h=(px)120 fill="#000000""##;
    let narrowed = compile(
        &runaround_doc(rect, r#"text-exclusion="ex""#),
        &default_provider(),
    );
    let uniform = compile(&runaround_doc("", ""), &default_provider());

    // Affected lines keep origin x == 0. A wrapped line emits one run PER word,
    // so the line ORIGIN is the MINIMUM x among runs sharing a baseline y.
    let mut min_x_by_y: std::collections::BTreeMap<u64, f64> = std::collections::BTreeMap::new();
    for (x, y) in glyph_run_positions(&narrowed.scene.commands) {
        let e = min_x_by_y.entry(y.to_bits()).or_insert(f64::INFINITY);
        *e = e.min(x);
    }
    for (y_bits, min_x) in &min_x_by_y {
        let y = f64::from_bits(*y_bits);
        if y <= 120.0 {
            assert!(
                *min_x < 1.0,
                "a right-exclusion line must start at text_x (0); got min_x={min_x} y={y}"
            );
        }
    }
    // The narrower measure produces at least as many lines as the full width.
    let n_narrow = glyph_run_positions(&narrowed.scene.commands).len();
    let n_uniform = glyph_run_positions(&uniform.scene.commands).len();
    assert!(
        n_narrow >= n_uniform,
        "narrowing the band must not reduce the line/run count ({n_narrow} vs {n_uniform})"
    );
}

/// A FULL-WIDTH exclusion band leaves the lines in that band EMPTY (no glyph
/// runs in the excluded y-range); text resumes below the band.
#[test]
fn runaround_full_width_exclusion_skips_lines() {
    // Rect [0,100,400,120] spans the whole box width over y in [100,220].
    // Both segments are 0 wide → those lines are BLOCKED (empty).
    let rect = r##"rect id="ex" x=(px)0 y=(px)100 w=(px)400 h=(px)120 fill="#000000""##;
    let result = compile(
        &runaround_doc(rect, r#"text-exclusion="ex""#),
        &default_provider(),
    );
    let pos = glyph_run_positions(&result.scene.commands);
    // No glyph baseline may fall strictly inside the excluded band interior.
    for (_, y) in &pos {
        assert!(
            !(*y > 100.0 && *y < 220.0),
            "no glyph run may land inside a full-width exclusion band (100..220); got y={y}"
        );
    }
    // Text must exist both above and below the band (flow resumes).
    assert!(
        pos.iter().any(|(_, y)| *y <= 100.0),
        "text must flow above the band"
    );
    assert!(
        pos.iter().any(|(_, y)| *y >= 220.0),
        "text must resume below the band"
    );
}

/// An unresolved `text-exclusion` emits the advisory AND renders byte-identically
/// to the uniform (no-exclusion) stream.
#[test]
fn runaround_unresolved_ref_emits_advisory_and_renders_uniform() {
    let bad = compile(
        &runaround_doc("", r#"text-exclusion="nope""#),
        &default_provider(),
    );
    let uniform = compile(&runaround_doc("", ""), &default_provider());

    let advisories: Vec<_> = bad
        .diagnostics
        .iter()
        .filter(|d| d.code == "text-exclusion.unresolved_ref")
        .collect();
    assert_eq!(
        advisories.len(),
        1,
        "exactly one unresolved-ref advisory; got {:?}",
        bad.diagnostics
    );

    assert_eq!(
        bad.scene.commands, uniform.scene.commands,
        "an unresolved exclusion must render the uniform stream (byte-identical)"
    );
}

/// Runaround source with an exclusion rect at a fixed page position, on a
/// page with `bleed_attr`.
fn bleed_runaround_src(bleed_attr: &str, layout: bool) -> String {
    let body = if layout {
        r#"frame id="f" x=(px)0 y=(px)0 w=(px)400 layout="column" {
        rect id="spacer" h=(px)40 fill=(token)"color.k"
        text id="body" font-size=(px)20 text-exclusion="ex" fill=(token)"color.k" {
          span "The quick brown fox jumps over the lazy dog and then keeps running far beyond the box edge to force wrapping across many lines of body text here"
        }
      }"#
    } else {
        r#"text id="body" x=(px)0 y=(px)40 w=(px)400 h=(px)560 font-size=(px)20 text-exclusion="ex" fill=(token)"color.k" {
        span "The quick brown fox jumps over the lazy dog and then keeps running far beyond the box edge to force wrapping across many lines of body text here"
      }"#
    };
    format!(
        r##"zenith version=1 {{
  project id="proj.rb" name="RB"
  tokens format="zenith-token-v1" {{
    token id="color.k" type="color" value="#000000"
  }}
  styles {{}}
  document id="doc.rb" title="RB" {{
    page id="page.rb" w=(px)600 h=(px)700{bleed_attr} {{
      rect id="ex" x=(px)0 y=(px)90 w=(px)250 h=(px)100 fill=(token)"color.k"
      {body}
    }}
  }}
}}
"##
    )
}

/// Glyph run origins of `body`, shifted back by `shift`.
fn body_runs(src: &str, shift: f64) -> Vec<(f64, f64)> {
    let result = compile(&parse(src), &default_provider());
    result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::DrawGlyphRun {
                x,
                y,
                source_node_id,
                ..
            } if source_node_id.as_deref() == Some("body") => Some((x - shift, y - shift)),
            _ => None,
        })
        .collect()
}

#[test]
fn runaround_wraps_the_same_on_a_bleed_page() {
    for layout in [false, true] {
        let plain = body_runs(&bleed_runaround_src("", layout), 0.0);
        let bled = body_runs(&bleed_runaround_src(" bleed=(px)30", layout), 30.0);
        assert!(plain.len() > 3, "{plain:?}");
        assert_eq!(plain, bled, "layout={layout}");
    }
}
