use super::*;

// ── connector node (U3): orthogonal routing + multi-segment marker orientation ─
//
// `route="orthogonal"` replaces the straight diagonal with a right-angle elbow
// path: an H–V–H (or V–H–V) 4-point Z-route when both anchors share an
// orientation, or a 3-point L-corner when they differ. The first segment leaves
// `from` perpendicular to its edge and the last enters `to` perpendicular to its
// edge, so arrowheads land axis-aligned. Straight routing is unchanged.

/// 4-point Z-route with the vertical riser at the mid x and right angles.
#[test]
fn connector_orthogonal_horizontal_boxes_makes_z_route() {
    let src = r##"zenith version=1 {
  project id="proj.co" name="CO"
  tokens format="zenith-token-v1" {
token id="color.line" type="color" value="#1e3a8a"
  }
  styles {}
  document id="doc.co" title="CO" {
page id="page.co" w=(px)640 h=(px)360 {
  rect id="a" x=(px)40 y=(px)40 w=(px)100 h=(px)80 stroke=(token)"color.line"
  rect id="b" x=(px)300 y=(px)60 w=(px)100 h=(px)80 stroke=(token)"color.line"
  connector id="c1" from="a" to="b" route="orthogonal" stroke=(token)"color.line"
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let pts = first_stroke_polyline_points(&result.scene.commands);

    // a right-mid = (140,80) [Horizontal], b left-mid = (300,100) [Horizontal].
    // mid x = (140+300)/2 = 220 → [140,80, 220,80, 220,100, 300,100].
    assert_eq!(
        pts,
        vec![140.0, 80.0, 220.0, 80.0, 220.0, 100.0, 300.0, 100.0],
        "horizontal-anchored orthogonal route must be an H–V–H Z-route; got {pts:?}"
    );
    // Right-angle elbow: the two riser points share the mid x.
    assert_eq!(pts[2], pts[4], "elbow x's must be equal (right angles)");
    assert_eq!(pts[2], (140.0 + 300.0) / 2.0, "riser sits at the mid x");
    // First segment is horizontal (leaves a's right edge), last is horizontal
    // (enters b's left edge).
    assert_eq!(pts[1], 80.0, "first segment leaves horizontally at fy");
    assert_eq!(pts[7], 100.0, "last segment enters horizontally at ty");
}

/// Two boxes stacked vertically, `route="orthogonal"` with auto anchors → a
/// V–H–V 4-point Z-route with the horizontal run at the mid y.
#[test]
fn connector_orthogonal_stacked_boxes_makes_vertical_z() {
    let src = r##"zenith version=1 {
  project id="proj.co" name="CO"
  tokens format="zenith-token-v1" {
token id="color.line" type="color" value="#1e3a8a"
  }
  styles {}
  document id="doc.co" title="CO" {
page id="page.co" w=(px)640 h=(px)480 {
  rect id="a" x=(px)40 y=(px)40 w=(px)100 h=(px)80 stroke=(token)"color.line"
  rect id="b" x=(px)60 y=(px)300 w=(px)100 h=(px)80 stroke=(token)"color.line"
  connector id="c1" from="a" to="b" route="orthogonal" stroke=(token)"color.line"
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let pts = first_stroke_polyline_points(&result.scene.commands);

    // a center (90,80), b center (110,340): dy dominates → Vertical anchors.
    // a bottom-mid = (90,120), b top-mid = (110,300). mid y = (120+300)/2 = 210
    // → [90,120, 90,210, 110,210, 110,300].
    assert_eq!(
        pts,
        vec![90.0, 120.0, 90.0, 210.0, 110.0, 210.0, 110.0, 300.0],
        "vertical-anchored orthogonal route must be a V–H–V Z-route; got {pts:?}"
    );
    // Right-angle elbow: the two crossbar points share the mid y.
    assert_eq!(pts[3], pts[5], "elbow y's must be equal (right angles)");
    assert_eq!(pts[3], (120.0 + 300.0) / 2.0, "crossbar sits at the mid y");
    // First segment vertical (leaves a's bottom edge), last vertical (enters b's top).
    assert_eq!(pts[0], pts[2], "first segment leaves vertically at fx");
    assert_eq!(pts[4], pts[6], "last segment enters vertically at tx");
}

/// `route="orthogonal"` + `marker-end="arrow"`: the arrowhead tip sits on the
/// `to` anchor and is axis-aligned to the (horizontal) last segment — its two
/// base vertices share the same x.
#[test]
fn connector_orthogonal_with_marker_end_arrowhead_is_axis_aligned() {
    let src = r##"zenith version=1 {
  project id="proj.co" name="CO"
  tokens format="zenith-token-v1" {
token id="color.line" type="color" value="#1e3a8a"
  }
  styles {}
  document id="doc.co" title="CO" {
page id="page.co" w=(px)640 h=(px)360 {
  rect id="a" x=(px)40 y=(px)40 w=(px)100 h=(px)80 stroke=(token)"color.line"
  rect id="b" x=(px)300 y=(px)60 w=(px)100 h=(px)80 stroke=(token)"color.line"
  connector id="c1" from="a" to="b" route="orthogonal" stroke=(token)"color.line" marker-end="arrow"
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let cmds = &result.scene.commands;

    // Last orthogonal segment is (220,100)→(300,100): horizontal entry into b.
    let heads = all_fill_polygon_points(cmds);
    assert_eq!(
        heads.len(),
        1,
        "marker-end must emit one FillPolygon; got {cmds:?}"
    );
    let head = &heads[0];
    assert_eq!(head.len(), 6, "arrowhead must be a 3-point triangle");

    // Tip on the `to` anchor (300,100).
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

    // Axis-aligned to a horizontal entry: the two base vertices (everything but
    // the tip) share the same x.
    let base: Vec<(f64, f64)> = head
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| (p[0], p[1]))
        .filter(|p| (p.0 - to_anchor.0).abs() >= 1e-9 || (p.1 - to_anchor.1).abs() >= 1e-9)
        .collect();
    assert_eq!(base.len(), 2, "triangle must have two base vertices");
    assert!(
        (base[0].0 - base[1].0).abs() < 1e-9,
        "horizontal entry → base vertices share the same x; got {base:?}"
    );
}

/// `route="straight"` (and the omitted default) still emits a 2-point line whose
/// marker endpoints are the raw anchors — U1/U2 byte-for-byte regression guard.
#[test]
fn connector_straight_route_unchanged_regression() {
    let src = r##"zenith version=1 {
  project id="proj.co" name="CO"
  tokens format="zenith-token-v1" {
token id="color.line" type="color" value="#1e3a8a"
  }
  styles {}
  document id="doc.co" title="CO" {
page id="page.co" w=(px)640 h=(px)360 {
  rect id="a" x=(px)40 y=(px)40 w=(px)100 h=(px)80 stroke=(token)"color.line"
  rect id="b" x=(px)300 y=(px)60 w=(px)100 h=(px)80 stroke=(token)"color.line"
  connector id="c1" from="a" to="b" route="straight" stroke=(token)"color.line" marker-end="arrow"
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let cmds = &result.scene.commands;

    // Straight 2-point line between the raw anchors.
    let line = first_stroke_polyline_points(cmds);
    assert_eq!(
        line,
        vec![140.0, 80.0, 300.0, 100.0],
        "straight route must remain a 2-point line; got {line:?}"
    );

    // Marker endpoints are the raw anchors: tip on the `to` anchor.
    let heads = all_fill_polygon_points(cmds);
    assert_eq!(
        heads.len(),
        1,
        "marker-end must emit one FillPolygon; got {cmds:?}"
    );
    let to_anchor = (300.0, 100.0);
    let has_tip = heads[0]
        .as_chunks::<2>()
        .0
        .iter()
        .any(|p| (p[0] - to_anchor.0).abs() < 1e-9 && (p[1] - to_anchor.1).abs() < 1e-9);
    assert!(
        has_tip,
        "straight marker tip must sit on the to anchor; got {:?}",
        heads[0]
    );
}
