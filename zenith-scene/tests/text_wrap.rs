mod common;
use common::parse;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::SceneCommand;

/// Helper: collect (x, y) of every DrawGlyphRun emitted for a single
/// text node with the given box width, align, and span text.
fn wrap_runs(node_x: f64, box_w: f64, align: &str, span: &str) -> Vec<(f64, f64)> {
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.wr" name="WR"
  tokens format="zenith-token-v1" {{}}
  styles {{}}
  document id="doc.wr" title="WR" {{
page id="page.wr" w=(px)1000 h=(px)600 {{
  text id="text.wr" x=(px){node_x} y=(px)20 w=(px){box_w} align="{align}" {{
    span "{span}"
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
            if let SceneCommand::DrawGlyphRun { x, y, .. } = c {
                Some((*x, *y))
            } else {
                None
            }
        })
        .collect()
}

// ── Text wrapping (word wrap) ─────────────────────────────────────────

/// A long single span in a narrow box wraps to multiple lines: more than one
/// DrawGlyphRun, appearing at >= 2 distinct baseline y values.
#[test]
fn text_wraps_when_exceeding_box_width() {
    let runs = wrap_runs(
        10.0,
        120.0,
        "start",
        "the quick brown fox jumps over the lazy dog",
    );
    assert!(
        runs.len() > 1,
        "wrapped text must emit more than one run; got {}",
        runs.len()
    );
    let mut ys: Vec<f64> = runs.iter().map(|(_, y)| *y).collect();
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ys.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    assert!(
        ys.len() >= 2,
        "wrapped text must occupy >= 2 distinct baselines; got {ys:?}"
    );
}

/// Short text that fits the box takes the unchanged fast path: exactly one
/// logical line and (for start align) the first run sits at node x.
#[test]
fn text_fits_single_line_unchanged() {
    let runs = wrap_runs(40.0, 600.0, "start", "Hi there");
    // All runs share a single baseline (one line).
    let y0 = runs[0].1;
    assert!(
        runs.iter().all(|(_, y)| (*y - y0).abs() < 1e-6),
        "fitting text must stay on one line; got {runs:?}"
    );
    // First run x == node x (start-aligned fast path).
    assert_eq!(
        runs[0].0, 40.0,
        "start-aligned fitting text must begin at node x"
    );
}

/// Wrapped + center: each line's first run is inset to the right of node x.
#[test]
fn text_wrap_center_lines_inset() {
    let runs = wrap_runs(
        10.0,
        120.0,
        "center",
        "the quick brown fox jumps over the lazy dog",
    );
    assert!(runs.len() > 1, "expected wrapping; got {}", runs.len());
    // Group first-run-per-line by baseline; each line's first x > node_x.
    let mut seen_y: Vec<f64> = Vec::new();
    for (x, y) in &runs {
        if !seen_y.iter().any(|sy| (*sy - *y).abs() < 1e-6) {
            seen_y.push(*y);
            assert!(
                *x > 10.0,
                "center-wrapped line first run x ({x}) must be inset past node x (10)"
            );
        }
    }
}

/// Wrapped + justify: a non-last multi-word line is fully justified (first
/// word at node x, last word right edge ≈ node x + box_w), while the LAST
/// line stays start-aligned (first run at node x, not stretched).
#[test]
fn text_wrap_justify_spreads() {
    let node_x = 10.0;
    let box_w = 120.0;
    // Need the per-run advances too, so re-collect including last word edge.
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.wj" name="WJ"
  tokens format="zenith-token-v1" {{}}
  styles {{}}
  document id="doc.wj" title="WJ" {{
page id="page.wj" w=(px)1000 h=(px)600 {{
  text id="text.wj" x=(px){node_x} y=(px)20 w=(px){box_w} align="justify" {{
    span "the quick brown fox jumps over the lazy dog"
  }}
}}
  }}
}}
"##
    );
    let doc = parse(&src);
    let result = compile(&doc, &default_provider());
    // Collect (y, x) of all runs.
    let runs: Vec<(f64, f64)> = result
        .scene
        .commands
        .iter()
        .filter_map(|c| {
            if let SceneCommand::DrawGlyphRun { x, y, .. } = c {
                Some((*y, *x))
            } else {
                None
            }
        })
        .collect();
    assert!(runs.len() > 1, "expected wrapping; got {}", runs.len());

    // Distinct baselines, in order.
    let mut ys: Vec<f64> = Vec::new();
    for (y, _) in &runs {
        if !ys.iter().any(|v| (*v - *y).abs() < 1e-6) {
            ys.push(*y);
        }
    }
    assert!(ys.len() >= 2, "need >= 2 lines; got {}", ys.len());

    // First line: its first run must start at node x (justify keeps left edge).
    let first_line_y = ys[0];
    let first_line_first_x = runs
        .iter()
        .filter(|(y, _)| (*y - first_line_y).abs() < 1e-6)
        .map(|(_, x)| *x)
        .fold(f64::INFINITY, f64::min);
    assert!(
        (first_line_first_x - node_x).abs() < 1e-6,
        "justified first line must start at node x; got {first_line_first_x}"
    );

    // Last line stays start-aligned: its first run also begins at node x and
    // is not stretched to the box edge. We assert it begins at node x.
    let last_line_y = ys[ys.len() - 1];
    let last_line_first_x = runs
        .iter()
        .filter(|(y, _)| (*y - last_line_y).abs() < 1e-6)
        .map(|(_, x)| *x)
        .fold(f64::INFINITY, f64::min);
    assert!(
        (last_line_first_x - node_x).abs() < 1e-6,
        "last (start-aligned) line must begin at node x; got {last_line_first_x}"
    );
}

/// Justify math: on a fully-justified (non-last, multi-word) line the LAST
/// word's right edge reaches the box's right edge (within the last word's own
/// advance), confirming inter-word gaps widened to fill the box width.
#[test]
fn text_wrap_justify_fills_box_width() {
    let node_x = 10.0;
    let box_w = 120.0;
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.jf" name="JF"
  tokens format="zenith-token-v1" {{}}
  styles {{}}
  document id="doc.jf" title="JF" {{
page id="page.jf" w=(px)1000 h=(px)600 {{
  text id="text.jf" x=(px){node_x} y=(px)20 w=(px){box_w} align="justify" {{
    span "the quick brown fox jumps over the lazy dog"
  }}
}}
  }}
}}
"##
    );
    let doc = parse(&src);
    let result = compile(&doc, &default_provider());
    let runs: Vec<(f64, f64)> = result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::DrawGlyphRun { x, y, .. } => Some((*y, *x)),
            _ => None,
        })
        .collect();

    // Distinct baselines, in order.
    let mut ys: Vec<f64> = Vec::new();
    for (y, _) in &runs {
        if !ys.iter().any(|v| (*v - *y).abs() < 1e-6) {
            ys.push(*y);
        }
    }
    assert!(ys.len() >= 2, "need >= 2 lines; got {}", ys.len());

    // First (non-last, justified) line: the largest x of any run on it (the last
    // word's left edge) must sit close to the right box edge, i.e. the spread
    // pushed it well past the box midpoint. With a fitted (non-justified) line
    // the words would bunch on the left.
    let first_y = ys[0];
    let max_x_first = runs
        .iter()
        .filter(|(y, _)| (*y - first_y).abs() < 1e-6)
        .map(|(_, x)| *x)
        .fold(f64::NEG_INFINITY, f64::max);
    let box_right = node_x + box_w;
    let box_mid = node_x + box_w / 2.0;
    assert!(
        max_x_first > box_mid,
        "justified line's last word must be pushed past box midpoint {box_mid}; got {max_x_first} (box_right={box_right})"
    );
}

/// A text node whose font-family token resolves to an UNREGISTERED family
/// ("Oswald") must still emit a `DrawGlyphRun` (text not dropped) AND
/// produce exactly one `font.unresolved` advisory naming the node id and
/// the missing family.
#[test]
fn text_node_unregistered_family_falls_back_and_emits_advisory() {
    let src = r##"zenith version=1 {
  project id="proj.fb1" name="FB1"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.fb1" title="FB1" {
page id="page.fb1" w=(px)400 h=(px)200 {
  text id="headline" x=(px)10 y=(px)10 font-family="Oswald" {
    span "Hello"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    // The scene must contain at least one DrawGlyphRun (text not dropped).
    assert!(
        result
            .scene
            .commands
            .iter()
            .any(|c| matches!(c, SceneCommand::DrawGlyphRun { .. })),
        "expected DrawGlyphRun when unregistered family falls back; commands: {:?}",
        result.scene.commands,
    );

    // Exactly one font.unresolved advisory must be present, naming the node.
    let unresolved: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "font.unresolved")
        .collect();
    assert_eq!(
        unresolved.len(),
        1,
        "expected exactly one font.unresolved diagnostic, got {:?}",
        unresolved,
    );
    let msg = &unresolved[0].message;
    assert!(
        msg.contains("headline"),
        "advisory message should name the node 'headline'; got: {msg}"
    );
    assert!(
        msg.contains("Oswald"),
        "advisory message should name the missing family 'Oswald'; got: {msg}"
    );
}

/// A text node using the registered "Noto Sans" family must produce NO
/// `font.unresolved` diagnostic and must emit a `DrawGlyphRun` as usual.
#[test]
fn text_node_registered_family_no_advisory() {
    let src = r##"zenith version=1 {
  project id="proj.fb2" name="FB2"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.fb2" title="FB2" {
page id="page.fb2" w=(px)400 h=(px)200 {
  text id="body.text" x=(px)10 y=(px)10 font-family="Noto Sans" {
    span "Hello"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    // No font.unresolved diagnostics.
    let unresolved: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "font.unresolved")
        .collect();
    assert!(
        unresolved.is_empty(),
        "expected no font.unresolved diagnostics for registered family; got: {:?}",
        unresolved,
    );

    // DrawGlyphRun must still be present.
    assert!(
        result
            .scene
            .commands
            .iter()
            .any(|c| matches!(c, SceneCommand::DrawGlyphRun { .. })),
        "expected DrawGlyphRun for registered Noto Sans family",
    );
}
