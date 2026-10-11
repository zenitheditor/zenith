//! `set_layout` margin item fields: set, clear, reject a negative or a
//! non-box target, and the margin route in the `tx.layout_managed` message.

mod common;
use common::parse;
use zenith_scene::layout_boxes;
use zenith_tx::{Transaction, TxResult, TxStatus, run_transaction};

/// A column frame `stack` with three in-flow rects, a grid frame `tiles`,
/// and a line.
const DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.k" type="color" value="#000000"
    token id="space.md" type="dimension" value=(px)16
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)800 h=(px)600 {
      frame id="stack" x=(px)10 y=(px)20 w=(px)300 h=(px)400 layout="column" gap=(px)10 align="start" {
        rect id="a" w=(px)40 h=(px)20 fill=(token)"color.k"
        rect id="b" w=(px)60 h=(px)30 fill=(token)"color.k"
        rect id="c" w=(px)50 h=(px)10 fill=(token)"color.k"
      }
      frame id="tiles" x=(px)400 y=(px)20 w=(px)200 h=(px)100 layout="grid" columns=2 {
        rect id="t1" fill=(token)"color.k"
        rect id="t2" fill=(token)"color.k"
      }
      line id="rule" x1=(px)0 y1=(px)590 x2=(px)800 y2=(px)590 stroke=(token)"color.k"
    }
  }
}"##;

fn run_on(src: &str, json: &str) -> TxResult {
    let tx = Transaction::from_json(json).expect("transaction JSON parses");
    run_transaction(&parse(src), &tx).expect("run_transaction must not error")
}

fn run_json(json: &str) -> TxResult {
    run_on(DOC, json)
}

fn accepted(r: &TxResult) {
    assert!(
        matches!(
            r.status,
            TxStatus::Accepted | TxStatus::AcceptedWithWarnings
        ),
        "expected accepted; got {:?}: {:?}",
        r.status,
        r.diagnostics
    );
}

fn rejected_with<'r>(r: &'r TxResult, code: &str) -> &'r zenith_core::Diagnostic {
    assert_eq!(r.status, TxStatus::Rejected, "{:?}", r.diagnostics);
    assert_eq!(r.source_after, r.source_before);
    r.diagnostics
        .iter()
        .find(|d| d.code == code)
        .unwrap_or_else(|| panic!("expected {code}; got {:?}", r.diagnostics))
}

fn line_of<'s>(src: &'s str, id: &str) -> &'s str {
    let needle = format!("id=\"{id}\"");
    src.lines()
        .find(|l| l.contains(&needle))
        .unwrap_or_else(|| panic!("no line for {id}"))
}

fn boxes(src: &str) -> std::collections::BTreeMap<String, zenith_scene::LayoutBox> {
    layout_boxes(&parse(src), 0, &zenith_core::default_provider())
}

#[test]
fn margin_top_moves_one_child_and_the_siblings_after_it() {
    let r = run_json(r#"{"ops":[{"op":"set_layout","node":"b","margin_top":40}]}"#);
    accepted(&r);
    assert_eq!(r.affected_node_ids, vec!["b".to_owned()]);
    assert!(line_of(&r.source_after, "b").contains("margin-top=(px)40"));
    let before = boxes(&r.source_before);
    let after = boxes(&r.source_after);
    let dy = |id: &str| after[id].y - before[id].y;
    assert_eq!(dy("a"), 0.0);
    assert_eq!(dy("b"), 40.0);
    assert_eq!(dy("c"), 40.0);
    assert_eq!(after["b"].h, before["b"].h);
    assert_eq!(after["stack"], before["stack"]);
}

#[test]
fn every_margin_field_writes_its_attribute() {
    let r = run_json(
        r#"{"ops":[{"op":"set_layout","node":"a","margin":4,"margin_x":"(px)6","margin_y":"space.md","margin_top":"(pt)3","margin_right":1,"margin_bottom":2,"margin_left":"(token)space.md"}]}"#,
    );
    accepted(&r);
    let a = line_of(&r.source_after, "a");
    for want in [
        "margin=(px)4",
        "margin-x=(px)6",
        "margin-y=(token)\"space.md\"",
        "margin-top=(pt)3",
        "margin-right=(px)1",
        "margin-bottom=(px)2",
        "margin-left=(token)\"space.md\"",
    ] {
        assert!(a.contains(want), "missing {want} in {a}");
    }
}

#[test]
fn null_clears_a_margin() {
    let set = run_json(r#"{"ops":[{"op":"set_layout","node":"b","margin_top":40,"margin":2}]}"#);
    accepted(&set);
    let r = run_on(
        &set.source_after,
        r#"{"ops":[{"op":"set_layout","node":"b","margin_top":null}]}"#,
    );
    accepted(&r);
    let b = line_of(&r.source_after, "b");
    assert!(!b.contains("margin-top="), "{b}");
    assert!(b.contains("margin=(px)2"), "{b}");
}

#[test]
fn negative_margin_is_rejected() {
    let r = run_json(r#"{"ops":[{"op":"set_layout","node":"b","margin_top":-5}]}"#);
    let d = rejected_with(&r, "tx.invalid_value");
    assert!(d.message.contains("margin_top"), "{}", d.message);
    assert!(d.message.contains("0 or more"), "{}", d.message);
}

#[test]
fn margin_on_a_line_is_unsupported() {
    let r = run_json(r#"{"ops":[{"op":"set_layout","node":"rule","margin":4}]}"#);
    let d = rejected_with(&r, "tx.unsupported_property");
    assert!(d.message.contains("margin"), "{}", d.message);
}

#[test]
fn layout_managed_names_the_margin_route_in_a_stack() {
    for json in [
        r#"{"ops":[{"op":"nudge_geometry","node":"b","dy":40}]}"#,
        r#"{"ops":[{"op":"set_geometry","node":"b","y":60}]}"#,
    ] {
        let r = run_json(json);
        let d = rejected_with(&r, "tx.layout_managed");
        for want in [
            "set_layout with margin_top",
            "keeps the frame's gap",
            "set_layout with position=\"absolute\"",
            "move_backward",
        ] {
            assert!(d.message.contains(want), "missing {want} in {}", d.message);
        }
    }
}

#[test]
fn layout_managed_omits_the_margin_route_in_a_grid() {
    let r = run_json(r#"{"ops":[{"op":"nudge_geometry","node":"t2","dy":10}]}"#);
    let d = rejected_with(&r, "tx.layout_managed");
    assert!(!d.message.contains("margin"), "{}", d.message);
}
