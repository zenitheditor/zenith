use super::*;

// ── connector line-jumps (hops at connector-vs-connector crossings) ───────────
//
// A page-level `line-jumps="arc"` makes the horizontal connector hop over the
// vertical one at their crossing. Without the property the routes are
// byte-identical to today's plain connector routes.
#[test]
fn line_jumps_absent_is_byte_identical() {
    let doc = parse(&crossing_connectors_src(""));
    let result = compile(&doc, &default_provider());
    let strokes = open_center_strokes(&result.scene.commands);

    // Two connector polylines, plain straight routes, untouched.
    assert_eq!(
        strokes,
        vec![
            vec![120.0, 160.0, 520.0, 160.0], // horizontal a→b
            vec![320.0, 60.0, 320.0, 300.0],  // vertical c→d
        ],
        "without line-jumps both connectors keep their plain routes"
    );
}

#[test]
fn line_jumps_arc_horizontal_hops() {
    let doc = parse(&crossing_connectors_src(r#"line-jumps="arc""#));
    let result = compile(&doc, &default_provider());
    let strokes = open_center_strokes(&result.scene.commands);
    assert_eq!(strokes.len(), 2, "still two connector polylines");

    let horiz = &strokes[0];
    let vert = &strokes[1];

    // The horizontal connector gains a bump (more than the plain 4 coords);
    // the vertical one is unchanged.
    assert!(
        horiz.len() > 4,
        "horizontal connector should gain bump points: {horiz:?}"
    );
    assert_eq!(
        vert,
        &vec![320.0, 60.0, 320.0, 300.0],
        "vertical connector must be unchanged"
    );
    // Bump bulges above the line (smaller y) near the x=320 crossing.
    let min_y = horiz
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| p[1])
        .fold(f64::INFINITY, f64::min);
    assert!(min_y < 160.0, "bump must dip above the line: {horiz:?}");
}
#[test]
fn line_jumps_apply_to_nested_connectors() {
    let doc = parse(&nested_crossing_connectors_src(r#"line-jumps="arc""#));
    let result = compile(&doc, &default_provider());
    let strokes = open_center_strokes(&result.scene.commands);
    assert_eq!(
        strokes.len(),
        2,
        "still two connector polylines (one nested): {strokes:?}"
    );

    let horiz = &strokes[0];
    let vert = &strokes[1];

    // The horizontal connector hops over the nested vertical one: it gains a
    // bump (more than the plain 4 coords). The vertical (nested) one is unchanged.
    assert!(
        horiz.len() > 4,
        "horizontal connector should hop over the NESTED vertical one: {horiz:?}"
    );
    assert_eq!(
        vert,
        &vec![320.0, 60.0, 320.0, 300.0],
        "nested vertical connector must keep its plain route"
    );
    let min_y = horiz
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| p[1])
        .fold(f64::INFINITY, f64::min);
    assert!(min_y < 160.0, "bump must dip above the line: {horiz:?}");
}

/// A self-loop (`from` and `to` name the SAME node) routes as a rectangular loop
/// off the box edge — a 4-point path that bulges above the top edge by default —
/// not a degenerate zero-length line.
#[test]
fn connector_self_loop_routes_a_loop() {
    let src = r##"zenith version=1 {
  project id="proj.cn" name="CN"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#dbeafe"
token id="color.line" type="color" value="#1e3a8a"
token id="size.stroke" type="dimension" value=(px)2
  }
  styles {}
  document id="doc.cn" title="CN" {
page id="page.cn" w=(px)400 h=(px)300 {
  rect id="a" x=(px)100 y=(px)120 w=(px)120 h=(px)60 fill=(token)"color.fill"
  connector id="c1" from="a" to="a" stroke=(token)"color.line" stroke-width=(token)"size.stroke"
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let pts = first_stroke_polyline_points(&result.scene.commands);

    // a: x=100 y=120 w=120 h=60 → top edge y=120, center x=160.
    // Default top loop: two feet on y=120, bulging up by 28px to y=92.
    assert_eq!(
        pts.len(),
        8,
        "self-loop must be a 4-point loop; got {pts:?}"
    );
    // Both feet sit on the top edge; both bulge points are strictly above it.
    assert_eq!(pts[1], 120.0);
    assert_eq!(pts[7], 120.0);
    assert!(
        pts[3] < 120.0 && pts[5] < 120.0,
        "loop must bulge above the top edge; got {pts:?}"
    );
}
