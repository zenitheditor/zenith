use super::*;

// ── connector node (U2): arrowhead markers (marker-start / marker-end) ────────
//
// `marker-end="arrow"` adds a filled-triangle head whose tip sits exactly on the
// `to` anchor; `marker-start="arrow"` does the same at the `from` anchor. The head
// reuses the line's stroke color. Default (no marker) emits only the line.
/// `marker-end="arrow"` → one StrokePolyline (the line) PLUS one FillPolygon (the
/// arrowhead): 3 vertices (6 coords), with the tip sitting on the `to` anchor.
#[test]
fn connector_marker_end_emits_arrowhead_at_to_anchor() {
    let src = r##"zenith version=1 {
  project id="proj.cn" name="CN"
  tokens format="zenith-token-v1" {
token id="color.line" type="color" value="#1e3a8a"
  }
  styles {}
  document id="doc.cn" title="CN" {
page id="page.cn" w=(px)640 h=(px)360 {
  rect id="a" x=(px)40 y=(px)40 w=(px)100 h=(px)80 stroke=(token)"color.line"
  rect id="b" x=(px)300 y=(px)60 w=(px)100 h=(px)80 stroke=(token)"color.line"
  connector id="c1" from="a" to="b" from-anchor="right" to-anchor="left" stroke=(token)"color.line" marker-end="arrow"
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let cmds = &result.scene.commands;

    // The line is still emitted.
    let line = first_stroke_polyline_points(cmds);
    assert_eq!(line, vec![140.0, 80.0, 300.0, 100.0]);

    // Exactly one arrowhead, a 3-vertex triangle.
    let heads = all_fill_polygon_points(cmds);
    assert_eq!(
        heads.len(),
        1,
        "marker-end must emit exactly one FillPolygon; got {cmds:?}"
    );
    let head = &heads[0];
    assert_eq!(
        head.len(),
        6,
        "arrowhead must be a 3-point triangle (6 coords)"
    );

    // The tip sits on the `to` anchor (300, 100): one vertex must equal it.
    let to_anchor = (300.0, 100.0);
    let has_tip = head
        .as_chunks::<2>()
        .0
        .iter()
        .any(|p| (p[0] - to_anchor.0).abs() < 1e-9 && (p[1] - to_anchor.1).abs() < 1e-9);
    assert!(
        has_tip,
        "an arrowhead vertex must equal the to anchor; got {head:?}"
    );

    // Horizontal left→right travel: the tip is the rightmost vertex.
    let max_x = head
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| p[0])
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        (max_x - to_anchor.0).abs() < 1e-9,
        "head must point rightward: tip x is the max x; got {head:?}"
    );
}

/// `marker-start` AND `marker-end` both "arrow" → the line PLUS TWO FillPolygons,
/// one tip on the `from` anchor and one tip on the `to` anchor.
#[test]
fn connector_both_markers_emit_two_arrowheads() {
    let src = r##"zenith version=1 {
  project id="proj.cn" name="CN"
  tokens format="zenith-token-v1" {
token id="color.line" type="color" value="#1e3a8a"
  }
  styles {}
  document id="doc.cn" title="CN" {
page id="page.cn" w=(px)640 h=(px)360 {
  rect id="a" x=(px)40 y=(px)40 w=(px)100 h=(px)80 stroke=(token)"color.line"
  rect id="b" x=(px)300 y=(px)60 w=(px)100 h=(px)80 stroke=(token)"color.line"
  connector id="c1" from="a" to="b" from-anchor="right" to-anchor="left" stroke=(token)"color.line" marker-start="arrow" marker-end="arrow"
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let cmds = &result.scene.commands;

    assert_eq!(
        first_stroke_polyline_points(cmds),
        vec![140.0, 80.0, 300.0, 100.0]
    );

    let heads = all_fill_polygon_points(cmds);
    assert_eq!(
        heads.len(),
        2,
        "both markers must emit two FillPolygons; got {cmds:?}"
    );

    let from_anchor = (140.0, 80.0);
    let to_anchor = (300.0, 100.0);
    let tip_on = |head: &Vec<f64>, t: (f64, f64)| {
        head.as_chunks::<2>()
            .0
            .iter()
            .any(|p| (p[0] - t.0).abs() < 1e-9 && (p[1] - t.1).abs() < 1e-9)
    };
    assert!(
        heads.iter().any(|h| tip_on(h, from_anchor)),
        "one arrowhead tip must sit on the from anchor; got {heads:?}"
    );
    assert!(
        heads.iter().any(|h| tip_on(h, to_anchor)),
        "one arrowhead tip must sit on the to anchor; got {heads:?}"
    );
}

/// Default (no markers) → only the line, no FillPolygon (U1 regression).
#[test]
fn connector_without_markers_emits_no_arrowhead() {
    let src = r##"zenith version=1 {
  project id="proj.cn" name="CN"
  tokens format="zenith-token-v1" {
token id="color.line" type="color" value="#1e3a8a"
  }
  styles {}
  document id="doc.cn" title="CN" {
page id="page.cn" w=(px)640 h=(px)360 {
  rect id="a" x=(px)40 y=(px)40 w=(px)100 h=(px)80 stroke=(token)"color.line"
  rect id="b" x=(px)300 y=(px)60 w=(px)100 h=(px)80 stroke=(token)"color.line"
  connector id="c1" from="a" to="b" stroke=(token)"color.line"
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let cmds = &result.scene.commands;

    assert!(
        cmds.iter()
            .any(|c| matches!(c, SceneCommand::StrokePolyline { .. })),
        "connector must still emit its line; got {cmds:?}"
    );
    assert!(
        !cmds
            .iter()
            .any(|c| matches!(c, SceneCommand::FillPolygon { .. })),
        "a connector with no markers must emit no FillPolygon; got {cmds:?}"
    );
}
