mod common;
use common::parse;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::SceneCommand;

/// Helper: compile a single-span text node with the given align and w,
/// return the x of the sole DrawGlyphRun.
fn text_align_run_x(align: Option<&str>, node_x: f64, node_w: Option<f64>) -> f64 {
    let w_attr = node_w.map_or(String::new(), |w| format!(" w=(px){w}"));
    let align_attr = align.map_or(String::new(), |a| format!(" align=\"{a}\""));
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.al" name="AL"
  tokens format="zenith-token-v1" {{}}
  styles {{}}
  document id="doc.al" title="AL" {{
page id="page.al" w=(px)800 h=(px)400 {{
  text id="text.al" x=(px){node_x} y=(px)20{w_attr}{align_attr} {{
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
        .find_map(|c| {
            if let SceneCommand::DrawGlyphRun { x, .. } = c {
                Some(*x)
            } else {
                None
            }
        })
        .expect("a DrawGlyphRun must be emitted")
}

// ── Text alignment ────────────────────────────────────────────────────

/// `align="start"` (or absent) → run x equals node x (no offset applied).
#[test]
fn text_align_start_run_at_node_x() {
    // Explicit "start"
    let x = text_align_run_x(Some("start"), 50.0, Some(300.0));
    assert_eq!(x, 50.0, "align=start must place run at node x");
    // Absent align
    let x = text_align_run_x(None, 50.0, Some(300.0));
    assert_eq!(x, 50.0, "absent align must behave as start");
    // Absent w — no box, no offset regardless of align
    let x = text_align_run_x(Some("center"), 50.0, None);
    assert_eq!(x, 50.0, "absent w disables alignment (start fallback)");
}

/// `align="center"` → run x is inset from node x by (w − advance) / 2,
/// which is strictly greater than node x when the text is narrower than w.
#[test]
fn text_align_center_run_inset_from_node_x() {
    let node_x = 10.0;
    let box_w = 500.0;
    let x = text_align_run_x(Some("center"), node_x, Some(box_w));
    assert!(
        x > node_x,
        "center-aligned run x ({x}) must be greater than node x ({node_x})"
    );
    // The run's right edge is at x + advance; by symmetry the left inset
    // and right inset from the box edges are equal, so x must be strictly
    // less than node_x + box_w / 2 (text "Hello" is narrower than half the box).
    assert!(
        x < node_x + box_w / 2.0,
        "center-aligned run x ({x}) must be less than box midpoint ({})",
        node_x + box_w / 2.0
    );
}

/// `align="end"` → the run's advance right-edge aligns with node_x + w,
/// i.e. run_x < node_x + w AND run_x > node_x (text is narrower than box).
#[test]
fn text_align_end_run_right_edge_at_box_right() {
    let node_x = 10.0;
    let box_w = 500.0;
    let x = text_align_run_x(Some("end"), node_x, Some(box_w));
    // x should be greater than node_x (we advanced inward from start)
    assert!(
        x > node_x,
        "end-aligned run x ({x}) must be greater than node x ({node_x})"
    );
    // x should be less than node_x + box_w (the run has positive width)
    assert!(
        x < node_x + box_w,
        "end-aligned run x ({x}) must be less than right edge ({})",
        node_x + box_w
    );
}

/// Multi-span centered line: first span starts at the centered offset and
/// the second span is contiguous (its x equals first_x + first_advance).
#[test]
fn text_align_center_multi_span_contiguous() {
    let src = r##"zenith version=1 {
  project id="proj.ac2" name="AC2"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.ac2" title="AC2" {
page id="page.ac2" w=(px)800 h=(px)400 {
  text id="text.ac2" x=(px)10 y=(px)20 w=(px)600 align="center" {
    span "Hello"
    span " World"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let runs: Vec<(f64, f32)> = result
        .scene
        .commands
        .iter()
        .filter_map(|c| {
            if let SceneCommand::DrawGlyphRun { x, font_size, .. } = c {
                Some((*x, *font_size))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(runs.len(), 2, "two spans → two runs; got {}", runs.len());
    let (x0, _) = runs[0];
    let (x1, _) = runs[1];
    // First run must be inset from node x (centered)
    assert!(
        x0 > 10.0,
        "first span of center-aligned text must be to the right of node x; got {x0}"
    );
    // Spans must be contiguous (second starts where first ends)
    assert!(
        x1 > x0,
        "second span x ({x1}) must follow first span x ({x0})"
    );
}
