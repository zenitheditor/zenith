//! `set_anchor`, `nudge_anchor_gap`, and `detach_anchor` on a sibling
//! stack like `examples/stack.zen`.

use super::*;

const STACK: &str = r#"rect id="title" x=(px)32 y=(px)28 w=(px)380 h=(px)32 fill=(token)"color.k"
      rect id="accent" anchor-sibling="title" anchor-edge="after" anchor-gap=(px)10 anchor="center-left" w=(px)6 h=(px)26 fill=(token)"color.k"
      rect id="card.a" anchor-sibling="title" anchor-edge="below" anchor-gap=(px)18 w=(px)416 h=(px)64 fill=(token)"color.k"
      rect id="label.a" anchor-sibling="card.a" anchor="center" w=(px)384 h=(px)24 fill=(token)"color.k"
      rect id="card.b" anchor-sibling="card.a" anchor-edge="below" anchor-gap=(pt)12 w=(px)416 h=(px)64 fill=(token)"color.k"
      rect id="card.c" anchor-sibling="card.b" anchor-edge="above" w=(px)100 h=(px)10 fill=(token)"color.k"
      rect id="badge" anchor="top-right" w=(px)40 h=(px)20 fill=(token)"color.k"
      group id="grp" x=(px)5 y=(px)5 {
        rect id="inner" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.k"
      }"#;

#[test]
fn nudge_anchor_gap_moves_along_the_edge_axis_and_the_cascade_follows() {
    let src = doc(STACK);
    let r = run(&src, r#"{"op":"nudge_anchor_gap","node":"card.a","dy":6}"#);
    accepted(&r);
    assert_eq!(r.affected_node_ids, vec!["card.a".to_owned()]);
    assert!(line_of(&r.source_after, "card.a").contains("anchor-gap=(px)24"));
    // card.a, its label, and card.b below it all move down 6 px.
    for id in ["card.a", "label.a", "card.b"] {
        let (_, y0, _, _) = box_of(&src, id);
        let (_, y1, _, _) = box_of(&r.source_after, id);
        assert_eq!(y1 - y0, 6.0, "{id}");
    }
}

#[test]
fn nudge_anchor_gap_keeps_the_gap_unit_and_signs_by_edge() {
    let src = doc(STACK);
    // below: +dy. 6 px = 4.5 pt.
    let r = run(&src, r#"{"op":"nudge_anchor_gap","node":"card.b","dy":6}"#);
    accepted(&r);
    assert!(line_of(&r.source_after, "card.b").contains("anchor-gap=(pt)16.5"));
    // above: -dy. No gap counts as (px)0.
    let r = run(&src, r#"{"op":"nudge_anchor_gap","node":"card.c","dy":-4}"#);
    accepted(&r);
    assert!(line_of(&r.source_after, "card.c").contains("anchor-gap=(px)4"));
    // after: +dx.
    let r = run(&src, r#"{"op":"nudge_anchor_gap","node":"accent","dx":-3}"#);
    accepted(&r);
    assert!(line_of(&r.source_after, "accent").contains("anchor-gap=(px)7"));
}

#[test]
fn nudge_anchor_gap_rejects_cross_axis_and_non_edge_anchors() {
    let src = doc(STACK);
    let r = run(
        &src,
        r#"{"op":"nudge_anchor_gap","node":"card.a","dx":3,"dy":6}"#,
    );
    let message = rejected(&r, "tx.anchored");
    assert!(message.contains("dx 3"), "{message}");
    let r = run(&src, r#"{"op":"nudge_anchor_gap","node":"label.a","dy":6}"#);
    let message = rejected(&r, "tx.unsupported_property");
    assert!(message.contains("anchor-edge"), "{message}");
    let r = run(&src, r#"{"op":"nudge_anchor_gap","node":"card.a","dx":0}"#);
    accepted(&r);
    assert!(r.diagnostics.iter().any(|d| d.code == "tx.noop"));
}

#[test]
fn set_anchor_replaces_the_reference_and_gap() {
    let src = doc(STACK);
    let r = run(
        &src,
        r#"{"op":"set_anchor","node":"card.b","anchor_sibling":"title","anchor_edge":"below","anchor_gap":"(pt)3"}"#,
    );
    accepted(&r);
    let line = line_of(&r.source_after, "card.b");
    assert!(line.contains(r#"anchor-sibling="title""#), "{line}");
    assert!(line.contains("anchor-gap=(pt)3"), "{line}");
    // card.b now sits 4 px (3 pt) below the title, over card.a.
    let (_, ty, _, th) = box_of(&r.source_after, "title");
    let (_, by, _, _) = box_of(&r.source_after, "card.b");
    assert_eq!(by, ty + th + 4.0);
}

#[test]
fn set_anchor_null_removes_and_numbers_are_px() {
    let src = doc(STACK);
    let r = run(
        &src,
        r#"{"op":"set_anchor","node":"accent","anchor":null,"anchor_gap":2}"#,
    );
    accepted(&r);
    let line = line_of(&r.source_after, "accent");
    assert!(!line.contains("anchor=\"center-left\""), "{line}");
    assert!(line.contains("anchor-gap=(px)2"), "{line}");
}

#[test]
fn set_anchor_checks_values_and_target() {
    let src = doc(STACK);
    let r = run(
        &src,
        r#"{"op":"set_anchor","node":"badge","anchor":"middle"}"#,
    );
    let message = rejected(&r, "tx.invalid_value");
    assert!(message.contains(r#""middle""#), "{message}");
    let r = run(
        &src,
        r#"{"op":"set_anchor","node":"card.a","anchor_edge":"under"}"#,
    );
    rejected(&r, "tx.invalid_value");
    let r = run(
        &src,
        r#"{"op":"set_anchor","node":"card.a","anchor_sibling":"card.a"}"#,
    );
    let message = rejected(&r, "tx.invalid_value");
    assert!(message.contains("itself"), "{message}");
    let r = run(
        &src,
        r#"{"op":"set_anchor","node":"card.a","anchor_gap":"(pct)5"}"#,
    );
    rejected(&r, "tx.invalid_value");
    // A node in another container is not a sibling.
    let r = run(
        &src,
        r#"{"op":"set_anchor","node":"card.a","anchor_sibling":"inner"}"#,
    );
    rejected(&r, "anchor.unresolved_sibling");
    // A sibling cycle.
    let r = run(
        &src,
        r#"{"op":"set_anchor","node":"title","anchor_sibling":"card.a","anchor_edge":"below"}"#,
    );
    rejected(&r, "anchor.cycle");
}

#[test]
fn set_anchor_clearing_a_placing_anchor_without_xy_is_rejected() {
    let src = doc(STACK);
    let r = run(&src, r#"{"op":"set_anchor","node":"badge","anchor":null}"#);
    let message = rejected(&r, "tx.geometry_required");
    assert!(message.contains("detach_anchor"), "{message}");
}

#[test]
fn set_anchor_on_an_in_flow_child_is_layout_managed() {
    let src = doc(
        r#"frame id="col" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="column" {
        rect id="a" w=(px)40 h=(px)20 fill=(token)"color.k"
        rect id="b" w=(px)40 h=(px)20 fill=(token)"color.k"
      }"#,
    );
    let r = run(&src, r#"{"op":"set_anchor","node":"b","anchor":"center"}"#);
    rejected(&r, "tx.layout_managed");
}

#[test]
fn detach_anchor_keeps_every_box_in_place() {
    let src = doc(STACK);
    let before = boxes(&src);
    let r = run(&src, r#"{"op":"detach_anchor","node":"card.a"}"#);
    accepted(&r);
    let line = line_of(&r.source_after, "card.a");
    assert!(!line.contains("anchor"), "{line}");
    // title y 28 + h 32 + gap 18 = 78.
    assert!(line.contains("x=(px)32 y=(px)78"), "{line}");
    assert_eq!(boxes(&r.source_after), before);
    // The other anchors are untouched.
    assert!(line_of(&r.source_after, "card.b").contains(r#"anchor-sibling="card.a""#));
}

#[test]
fn detach_anchor_on_page_anchor_and_after_edge() {
    let src = doc(STACK);
    let before = boxes(&src);
    let r = run(
        &src,
        r#"{"op":"detach_anchor","node":"badge"},{"op":"detach_anchor","node":"accent"}"#,
    );
    accepted(&r);
    assert!(line_of(&r.source_after, "badge").contains("x=(px)440 y=(px)0"));
    assert_eq!(boxes(&r.source_after), before);
}

#[test]
fn detach_then_nudge_moves_by_the_drag_delta() {
    let src = doc(STACK);
    let r = run(
        &src,
        r#"{"op":"detach_anchor","node":"label.a"},{"op":"nudge_geometry","node":"label.a","dx":4,"dy":-2}"#,
    );
    accepted(&r);
    let (x0, y0, _, _) = box_of(&src, "label.a");
    let (x1, y1, _, _) = box_of(&r.source_after, "label.a");
    assert_eq!((x1 - x0, y1 - y0), (4.0, -2.0));
}

#[test]
fn detach_anchor_without_anchor_is_a_noop() {
    let src = doc(STACK);
    let r = run(&src, r#"{"op":"detach_anchor","node":"title"}"#);
    accepted(&r);
    assert!(r.diagnostics.iter().any(|d| d.code == "tx.noop"));
    let r = run(&src, r#"{"op":"detach_anchor","node":"nope"}"#);
    rejected(&r, "tx.unknown_node");
}

#[test]
fn anchor_ops_respect_locks() {
    let src = doc(&STACK.replace(
        r#"anchor-gap=(px)18 w=(px)416"#,
        r#"anchor-gap=(px)18 locked=#true w=(px)416"#,
    ));
    for op in [
        r#"{"op":"nudge_anchor_gap","node":"card.a","dy":1}"#,
        r#"{"op":"set_anchor","node":"card.a","anchor_gap":1}"#,
        r#"{"op":"detach_anchor","node":"card.a"}"#,
    ] {
        rejected(&run(&src, op), "node.locked");
    }
}

#[test]
fn detach_anchor_promises_the_page_box() {
    let op: Op = serde_json::from_str(r#"{"op":"detach_anchor","node":"card.a"}"#).expect("op");
    let subjects = op.position_preserving_subjects(&parse(&doc(STACK)));
    assert_eq!(subjects, Some(vec!["card.a".to_owned()]));
}
