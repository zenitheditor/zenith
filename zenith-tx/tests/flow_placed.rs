//! `tx.flow_placed`: reparent, group, and ungroup report a node that a
//! row / column / grid frame places in a flow slot.

mod common;

use common::*;
use zenith_core::Severity;
use zenith_tx::{Op, Permissions, Position, Transaction, TxResult, TxStatus, run_transaction};

fn doc(body: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {{
    token id="color.k" type="color" value="#000000"
  }}
  styles {{ }}
  document id="doc1" title="T" {{
    page id="pg1" w=(px)800 h=(px)600 {{
      {body}
    }}
  }}
}}"##
    )
}

fn run(source: &str, ops: Vec<Op>) -> TxResult {
    let tx = Transaction {
        ops,
        permissions: Permissions::default(),
    };
    run_transaction(&parse(source), &tx).expect("run_transaction must not error")
}

fn flow_subjects(r: &TxResult) -> Vec<&str> {
    r.diagnostics
        .iter()
        .filter(|d| d.code == "tx.flow_placed")
        .filter_map(|d| d.subject_id.as_deref())
        .collect()
}

fn reparent(node: &str, new_parent: &str) -> Op {
    Op::Reparent {
        node: node.to_owned(),
        new_parent: new_parent.to_owned(),
        position: Position::default(),
    }
}

const ROW: &str = r#"frame id="cards.row2" x=(px)40 y=(px)300 w=(px)700 h=(px)140 layout="row" gap=(px)8 {
        rect id="c1" x=(px)0 y=(px)0 w=(px)200 h=(px)120 fill=(token)"color.k"
        rect id="c2" x=(px)0 y=(px)0 w=(px)200 h=(px)120 fill=(token)"color.k"
      }
      frame id="free" x=(px)40 y=(px)40 w=(px)300 h=(px)200 {
        rect id="f1" x=(px)10 y=(px)10 w=(px)50 h=(px)50 fill=(token)"color.k"
      }
      rect id="card.3" x=(px)526 y=(px)40 w=(px)219 h=(px)120 fill=(token)"color.k"
      rect id="pinned" x=(px)500 y=(px)200 w=(px)50 h=(px)50 position="absolute" fill=(token)"color.k""#;

#[test]
fn reparent_into_row_frame_fires() {
    let r = run(&doc(ROW), vec![reparent("card.3", "cards.row2")]);
    assert_eq!(r.status, TxStatus::Accepted, "{:?}", r.diagnostics);
    let d = r
        .diagnostics
        .iter()
        .find(|d| d.code == "tx.flow_placed")
        .unwrap_or_else(|| panic!("expected tx.flow_placed; got {:?}", r.diagnostics));
    assert_eq!(d.severity, Severity::Advisory);
    assert_eq!(d.subject_id.as_deref(), Some("card.3"));
    assert_eq!(
        d.message,
        "reparent: node \"card.3\" is placed by layout frame \"cards.row2\" (row); its x/y and \
         size are ignored and siblings reflow. To keep its page box, set position=\"absolute\" \
         or use set_geometry."
    );
}

#[test]
fn reparent_into_absolute_frame_is_silent() {
    let r = run(&doc(ROW), vec![reparent("card.3", "free")]);
    assert_eq!(r.status, TxStatus::Accepted, "{:?}", r.diagnostics);
    assert!(flow_subjects(&r).is_empty(), "{:?}", r.diagnostics);
}

#[test]
fn reparent_absolute_node_into_row_frame_is_silent() {
    let r = run(&doc(ROW), vec![reparent("pinned", "cards.row2")]);
    assert_eq!(r.status, TxStatus::Accepted, "{:?}", r.diagnostics);
    assert!(flow_subjects(&r).is_empty(), "{:?}", r.diagnostics);
}

#[test]
fn group_in_row_frame_fires_per_grouped_id() {
    let r = run(
        &doc(ROW),
        vec![Op::Group {
            node_ids: vec!["c1".to_owned(), "c2".to_owned()],
            group_id: "pair".to_owned(),
        }],
    );
    assert_eq!(r.status, TxStatus::Accepted, "{:?}", r.diagnostics);
    assert_eq!(flow_subjects(&r), ["c1", "c2"]);
    let msg = &r
        .diagnostics
        .iter()
        .find(|d| d.code == "tx.flow_placed")
        .expect("flow_placed")
        .message;
    assert!(
        msg.contains("\"pair\"") && msg.contains("\"cards.row2\" (row)"),
        "{msg}"
    );
}

#[test]
fn group_outside_flow_is_silent() {
    let r = run(
        &doc(ROW),
        vec![Op::Group {
            node_ids: vec!["f1".to_owned()],
            group_id: "solo".to_owned(),
        }],
    );
    assert_eq!(r.status, TxStatus::Accepted, "{:?}", r.diagnostics);
    assert!(flow_subjects(&r).is_empty(), "{:?}", r.diagnostics);
}

#[test]
fn ungroup_into_row_frame_fires_per_flow_child() {
    let start = doc(
        r#"frame id="row" x=(px)40 y=(px)40 w=(px)700 h=(px)140 layout="row" {
        group id="g" {
          rect id="a" w=(px)100 h=(px)100 fill=(token)"color.k"
          rect id="b" x=(px)0 y=(px)0 w=(px)100 h=(px)100 position="absolute" fill=(token)"color.k"
        }
      }"#,
    );
    let r = run(
        &start,
        vec![Op::Ungroup {
            group_id: "g".to_owned(),
        }],
    );
    assert!(
        matches!(
            r.status,
            TxStatus::Accepted | TxStatus::AcceptedWithWarnings
        ),
        "{:?}",
        r.diagnostics
    );
    assert_eq!(flow_subjects(&r), ["a"]);
    let msg = &r
        .diagnostics
        .iter()
        .find(|d| d.code == "tx.flow_placed")
        .expect("flow_placed")
        .message;
    assert!(msg.starts_with("ungroup: node \"a\""), "{msg}");
}

#[test]
fn ungroup_outside_flow_is_silent() {
    let start = doc(r#"group id="g" x=(px)10 y=(px)10 {
        rect id="a" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.k"
      }"#);
    let r = run(
        &start,
        vec![Op::Ungroup {
            group_id: "g".to_owned(),
        }],
    );
    assert_eq!(r.status, TxStatus::Accepted, "{:?}", r.diagnostics);
    assert!(flow_subjects(&r).is_empty(), "{:?}", r.diagnostics);
}
