//! `nudge_geometry`: px deltas in authored space, units kept, tokens,
//! computed sizes, and anchors guarded.

use super::*;

const BOXES: &str = r#"rect id="px" x=(px)10 y=(px)20 w=(px)100 h=(px)50 fill=(token)"color.k"
      rect id="pt" x=(pt)30 y=(pt)15 w=(pt)72 h=(pt)36 fill=(token)"color.k"
      rect id="tok" x=(token)"space.x" y=(px)0 w=(token)"space.pt" h=(px)10 fill=(token)"color.k"
      rect id="spun" x=(px)200 y=(px)200 w=(px)40 h=(px)20 rotate=(deg)30 fill=(token)"color.k"
      rect id="locked" x=(px)0 y=(px)300 w=(px)10 h=(px)10 fill=(token)"color.k" locked=#true
      group id="g" {
        rect id="g.r" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.k"
      }
      frame id="hugger" x=(px)300 y=(px)10 layout="column" {
        rect id="hugger.r" w=(px)20 h=(px)20 fill=(token)"color.k"
      }
      line id="rule" x1=(px)0 y1=(px)350 x2=(px)100 y2=(px)350 stroke=(token)"color.k""#;

#[test]
fn px_move_and_resize_add_the_delta() {
    let src = doc(BOXES);
    let r = run(
        &src,
        r#"{"op":"nudge_geometry","node":"px","dx":5,"dy":-2.5,"dw":20,"dh":-10}"#,
    );
    accepted(&r);
    assert_eq!(r.affected_node_ids, vec!["px".to_owned()]);
    let line = line_of(&r.source_after, "px");
    assert!(
        line.contains("x=(px)15 y=(px)17.5 w=(px)120 h=(px)40"),
        "{line}"
    );
}

#[test]
fn pt_values_stay_pt() {
    let src = doc(BOXES);
    // 3 px = 2.25 pt, 12 px = 9 pt.
    let r = run(
        &src,
        r#"{"op":"nudge_geometry","node":"pt","dx":3,"dy":12,"dw":-12}"#,
    );
    accepted(&r);
    let line = line_of(&r.source_after, "pt");
    assert!(
        line.contains("x=(pt)32.25 y=(pt)24 w=(pt)63 h=(pt)36"),
        "{line}"
    );
}

#[test]
fn omitted_deltas_leave_attributes_untouched() {
    let src = doc(BOXES);
    let r = run(&src, r#"{"op":"nudge_geometry","node":"tok","dy":4}"#);
    accepted(&r);
    let line = line_of(&r.source_after, "tok");
    assert!(
        line.contains(r#"x=(token)"space.x" y=(px)4 w=(token)"space.pt" h=(px)10"#),
        "{line}"
    );
}

#[test]
fn token_bound_axis_is_rejected_with_token_and_axis() {
    let src = doc(BOXES);
    let r = run(
        &src,
        r#"{"op":"nudge_geometry","node":"tok","dx":5,"dy":1}"#,
    );
    let message = rejected(&r, "tx.token_bound");
    assert!(message.contains(r#""space.x""#), "{message}");
    assert!(message.contains("x of node \"tok\""), "{message}");
    assert!(message.contains("detach=true"), "{message}");
    // The y delta is not applied either: the op is all or nothing.
    assert!(line_of(&r.source_after, "tok").contains("y=(px)0"));
}

#[test]
fn detach_replaces_a_token_with_px_of_the_resolved_value() {
    let src = doc(BOXES);
    // space.x = 40 px. space.pt = 12 pt = 16 px.
    let r = run(
        &src,
        r#"{"op":"nudge_geometry","node":"tok","dx":5,"dw":4,"detach":true}"#,
    );
    accepted(&r);
    let line = line_of(&r.source_after, "tok");
    assert!(
        line.contains("x=(px)45 y=(px)0 w=(px)20 h=(px)10"),
        "{line}"
    );
}

#[test]
fn absent_size_is_computed() {
    let src = doc(BOXES);
    // A layout frame without w/h hugs its children.
    let r = run(&src, r#"{"op":"nudge_geometry","node":"hugger","dh":10}"#);
    let message = rejected(&r, "tx.computed_size");
    assert!(message.contains("no authored h"), "{message}");
    assert!(message.contains("set_geometry"), "{message}");
}

#[test]
fn hug_and_fill_sizes_are_computed() {
    let src = doc(
        r#"frame id="col" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="column" {
        rect id="hugged" w="hug" h=(px)20 fill=(token)"color.k"
        rect id="sized" w=(px)40 h=(px)20 fill=(token)"color.k"
      }"#,
    );
    let r = run(&src, r#"{"op":"nudge_geometry","node":"hugged","dw":5}"#);
    let message = rejected(&r, "tx.computed_size");
    assert!(message.contains(r#""hug""#), "{message}");
    // A px size of an in-flow child resizes. The frame places it.
    let r = run(&src, r#"{"op":"nudge_geometry","node":"sized","dw":5}"#);
    accepted(&r);
    assert!(line_of(&r.source_after, "sized").contains("w=(px)45"));
    // Moving it is the frame's job.
    let r = run(&src, r#"{"op":"nudge_geometry","node":"sized","dx":5}"#);
    rejected(&r, "tx.layout_managed");
}

#[test]
fn anchored_axis_is_rejected_and_points_at_the_anchor_ops() {
    let src = doc(
        r#"rect id="a" x=(px)10 y=(px)10 w=(px)100 h=(px)40 fill=(token)"color.k"
      rect id="b" anchor-sibling="a" anchor-edge="below" anchor-gap=(px)8 w=(px)100 h=(px)40 fill=(token)"color.k"
      rect id="c" anchor="center" y=(px)300 w=(px)20 h=(px)20 fill=(token)"color.k""#,
    );
    let r = run(&src, r#"{"op":"nudge_geometry","node":"b","dy":5}"#);
    let message = rejected(&r, "tx.anchored");
    assert!(message.contains(r#"anchor-sibling="a""#), "{message}");
    assert!(message.contains("nudge_anchor_gap"), "{message}");
    assert!(message.contains("detach_anchor"), "{message}");
    // An authored y overrides the anchor on that axis: it nudges.
    let r = run(&src, r#"{"op":"nudge_geometry","node":"c","dy":5}"#);
    accepted(&r);
    assert!(line_of(&r.source_after, "c").contains("y=(px)305"));
    let r = run(&src, r#"{"op":"nudge_geometry","node":"c","dx":5}"#);
    rejected(&r, "tx.anchored");
}

#[test]
fn locked_node_is_rejected_unless_permitted() {
    let src = doc(BOXES);
    let r = run(&src, r#"{"op":"nudge_geometry","node":"locked","dx":1}"#);
    rejected(&r, "node.locked");
    let r = run_tx(
        &src,
        r#"{"ops":[{"op":"nudge_geometry","node":"locked","dx":1}],"permissions":{"allow_locked":true}}"#,
    );
    accepted(&r);
    assert!(line_of(&r.source_after, "locked").contains("x=(px)1"));
}

#[test]
fn negative_and_non_finite_sizes_are_rejected() {
    let src = doc(BOXES);
    let r = run(&src, r#"{"op":"nudge_geometry","node":"px","dw":-101}"#);
    let message = rejected(&r, "tx.invalid_geometry");
    assert!(message.contains("negative"), "{message}");
    // A size may shrink to exactly 0.
    let r = run(&src, r#"{"op":"nudge_geometry","node":"px","dh":-50}"#);
    accepted(&r);
    assert!(line_of(&r.source_after, "px").contains("h=(px)0"));
    let r = run_op(
        &src,
        Op::NudgeGeometry {
            node: "px".into(),
            dx: Some(f64::NAN),
            dy: None,
            dw: Some(f64::INFINITY),
            dh: None,
            detach: false,
        },
    );
    let message = rejected(&r, "tx.invalid_value");
    assert!(message.contains("not finite"), "{message}");
}

#[test]
fn rotated_node_nudges_in_its_unrotated_space() {
    let src = doc(BOXES);
    let r = run(
        &src,
        r#"{"op":"nudge_geometry","node":"spun","dx":10,"dw":6}"#,
    );
    accepted(&r);
    let line = line_of(&r.source_after, "spun");
    assert!(
        line.contains("x=(px)210 y=(px)200 w=(px)46 h=(px)20") && line.contains("rotate=(deg)30"),
        "{line}"
    );
    // The compiled box (unrotated bounds) moves by the authored delta.
    let (x0, _, w0, _) = box_of(&src, "spun");
    let (x1, _, w1, _) = box_of(&r.source_after, "spun");
    assert_eq!((x1 - x0, w1 - w0), (10.0, 6.0));
}

#[test]
fn absent_group_position_counts_as_zero() {
    let src = doc(BOXES);
    let r = run(&src, r#"{"op":"nudge_geometry","node":"g","dx":7,"dy":-3}"#);
    accepted(&r);
    assert!(line_of(&r.source_after, "g").contains("x=(px)7 y=(px)-3"));
    // A group without w/h sizes to its children.
    let r = run(&src, r#"{"op":"nudge_geometry","node":"g","dw":7}"#);
    rejected(&r, "tx.computed_size");
}

#[test]
fn values_without_px_are_unresolved() {
    let src = doc(r#"rect id="pc" x=(pct)10 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.k""#);
    let r = run(&src, r#"{"op":"nudge_geometry","node":"pc","dx":1}"#);
    let message = rejected(&r, "tx.value_unresolved");
    assert!(message.contains("(pct)10"), "{message}");
}

#[test]
fn kinds_without_a_box_name_the_right_op() {
    let src = doc(BOXES);
    let r = run(&src, r#"{"op":"nudge_geometry","node":"rule","dx":1}"#);
    let message = rejected(&r, "tx.unsupported_property");
    assert!(message.contains("nudge_line_points"), "{message}");
    let src = doc(
        r#"rect id="a" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.k"
      rect id="b" x=(px)100 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.k"
      connector id="e" from="a" to="b" stroke=(token)"color.k""#,
    );
    let r = run(&src, r#"{"op":"nudge_geometry","node":"e","dx":1}"#);
    rejected(&r, "tx.derived_geometry");
}

#[test]
fn no_deltas_is_a_noop_and_unknown_node_is_rejected() {
    let src = doc(BOXES);
    let r = run(&src, r#"{"op":"nudge_geometry","node":"px"}"#);
    accepted(&r);
    assert!(r.diagnostics.iter().any(|d| d.code == "tx.noop"));
    assert_eq!(r.source_after, r.source_before);
    let r = run(&src, r#"{"op":"nudge_geometry","node":"nope","dx":1}"#);
    rejected(&r, "tx.unknown_node");
}

#[test]
fn json_round_trips_and_omits_absent_deltas() {
    let json = r#"{"op":"nudge_geometry","node":"px","dx":1.5,"dh":-2}"#;
    let op: Op = serde_json::from_str(json).expect("parses");
    assert_eq!(
        op,
        Op::NudgeGeometry {
            node: "px".into(),
            dx: Some(1.5),
            dy: None,
            dw: None,
            dh: Some(-2.0),
            detach: false,
        }
    );
    let value = serde_json::to_value(&op).expect("serializes");
    assert_eq!(
        value,
        serde_json::json!({"op":"nudge_geometry","node":"px","dx":1.5,"dh":-2.0,"detach":false})
    );
    let back: Op = serde_json::from_value(value).expect("re-parses");
    assert_eq!(back, op);
}

#[test]
fn dry_run_diff_touches_only_the_nudged_line() {
    let src = doc(BOXES);
    let r = run(&src, r#"{"op":"nudge_geometry","node":"pt","dx":3}"#);
    accepted(&r);
    let before: Vec<&str> = r.source_before.lines().collect();
    let after: Vec<&str> = r.source_after.lines().collect();
    assert_eq!(before.len(), after.len());
    let changed: Vec<(&str, &str)> = before
        .iter()
        .zip(&after)
        .filter(|(b, a)| b != a)
        .map(|(b, a)| (*b, *a))
        .collect();
    assert_eq!(changed.len(), 1, "{changed:?}");
    let (old, new) = changed.first().copied().expect("one changed line");
    assert!(old.contains("x=(pt)30"), "{old}");
    assert!(new.contains("x=(pt)32.25"), "{new}");
    assert_eq!(old.replace("x=(pt)30", "x=(pt)32.25"), new);
}
