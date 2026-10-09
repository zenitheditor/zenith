//! `nudge_line_points`: endpoint deltas keep units. Connectors are derived.

use super::*;

const LINES: &str = r#"line id="rule" x1=(px)0 y1=(px)10 x2=(pt)75 y2=(px)10 stroke=(token)"color.k"
      rect id="a" x=(px)0 y=(px)100 w=(px)10 h=(px)10 fill=(token)"color.k"
      rect id="b" x=(px)200 y=(px)100 w=(px)10 h=(px)10 fill=(token)"color.k"
      connector id="e" from="a" to="b" stroke=(token)"color.k""#;

#[test]
fn moves_one_endpoint_and_keeps_units() {
    let src = doc(LINES);
    // 4 px = 3 pt.
    let r = run(
        &src,
        r#"{"op":"nudge_line_points","node":"rule","dx2":4,"dy2":-6}"#,
    );
    accepted(&r);
    assert_eq!(r.affected_node_ids, vec!["rule".to_owned()]);
    let line = line_of(&r.source_after, "rule");
    assert!(
        line.contains("x1=(px)0 y1=(px)10 x2=(pt)78 y2=(px)4"),
        "{line}"
    );
}

#[test]
fn moves_the_whole_line() {
    let src = doc(LINES);
    let r = run(
        &src,
        r#"{"op":"nudge_line_points","node":"rule","dx1":8,"dy1":2,"dx2":8,"dy2":2}"#,
    );
    accepted(&r);
    let line = line_of(&r.source_after, "rule");
    assert!(
        line.contains("x1=(px)8 y1=(px)12 x2=(pt)81 y2=(px)12"),
        "{line}"
    );
}

#[test]
fn connector_is_derived() {
    let src = doc(LINES);
    let r = run(&src, r#"{"op":"nudge_line_points","node":"e","dx1":1}"#);
    let message = rejected(&r, "tx.derived_geometry");
    assert!(message.contains("from/to targets"), "{message}");
}

#[test]
fn other_kinds_and_bad_input_are_rejected() {
    let src = doc(LINES);
    let r = run(&src, r#"{"op":"nudge_line_points","node":"a","dx1":1}"#);
    rejected(&r, "tx.unsupported_property");
    let r = run_op(
        &src,
        Op::NudgeLinePoints {
            node: "rule".into(),
            dx1: Some(f64::NAN),
            dy1: None,
            dx2: None,
            dy2: None,
        },
    );
    rejected(&r, "tx.invalid_value");
    let r = run(&src, r#"{"op":"nudge_line_points","node":"rule"}"#);
    accepted(&r);
    assert!(r.diagnostics.iter().any(|d| d.code == "tx.noop"));
}

#[test]
fn locked_line_is_rejected() {
    let src = doc(&LINES.replace(
        "stroke=(token)\"color.k\"\n",
        "stroke=(token)\"color.k\" locked=#true\n",
    ));
    let r = run(&src, r#"{"op":"nudge_line_points","node":"rule","dx1":1}"#);
    rejected(&r, "node.locked");
}

#[test]
fn json_round_trips() {
    let json = r#"{"op":"nudge_line_points","node":"rule","dx1":1.0,"dy2":2.0}"#;
    let op: Op = serde_json::from_str(json).expect("parses");
    let value = serde_json::to_value(&op).expect("serializes");
    assert_eq!(
        value,
        serde_json::json!({"op":"nudge_line_points","node":"rule","dx1":1.0,"dy2":2.0})
    );
}
