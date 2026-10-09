//! `group` keeps members in document (paint) order whatever order `node_ids`
//! lists them in, and `ungroup` restores that order: a group → ungroup round
//! trip over adjacent siblings gives back the original source.

mod common;
use common::parse;
use zenith_tx::{Op, Permissions, Transaction, TxResult, TxStatus, run_transaction};

/// Four overlapping siblings, painted a (bottom) → d (top).
const DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.k" type="color" value="#000000"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="a" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.k"
      ellipse id="b" x=(px)20 y=(px)20 w=(px)100 h=(px)100 fill=(token)"color.k"
      ellipse id="c" x=(px)40 y=(px)40 w=(px)100 h=(px)100 fill=(token)"color.k"
      rect id="d" x=(px)60 y=(px)60 w=(px)100 h=(px)100 fill=(token)"color.k"
    }
  }
}"##;

fn run(src: &str, ops: Vec<Op>) -> TxResult {
    let tx = Transaction {
        ops,
        permissions: Permissions::default(),
    };
    let r = run_transaction(&parse(src), &tx).expect("run_transaction must not error");
    assert_eq!(r.status, TxStatus::Accepted, "{:?}", r.diagnostics);
    r
}

fn group(ids: &[&str], group_id: &str) -> Op {
    Op::Group {
        node_ids: ids.iter().map(|s| (*s).to_owned()).collect(),
        group_id: group_id.to_owned(),
    }
}

fn ungroup(group_id: &str) -> Op {
    Op::Ungroup {
        group_id: group_id.to_owned(),
    }
}

/// Node ids in source order, which is paint order (first is bottom).
fn id_order(src: &str, ids: &[&str]) -> Vec<String> {
    let mut found: Vec<(usize, &str)> = ids
        .iter()
        .filter_map(|id| src.find(&format!("id=\"{id}\"")).map(|at| (at, *id)))
        .collect();
    found.sort_unstable();
    found.into_iter().map(|(_, id)| id.to_owned()).collect()
}

#[test]
fn group_keeps_document_order_when_ids_are_listed_in_reverse() {
    let r = run(DOC, vec![group(&["c", "b"], "pair")]);
    assert_eq!(
        id_order(&r.source_after, &["a", "pair", "b", "c", "d"]),
        ["a", "pair", "b", "c", "d"],
        "{}",
        r.source_after
    );
}

#[test]
fn group_then_ungroup_round_trips_order_and_source() {
    let grouped = run(DOC, vec![group(&["c", "b"], "pair")]);
    let ungrouped = run(&grouped.source_after, vec![ungroup("pair")]);
    assert_eq!(
        id_order(&ungrouped.source_after, &["a", "b", "c", "d"]),
        ["a", "b", "c", "d"]
    );
    assert_eq!(
        ungrouped.source_after, grouped.source_before,
        "group → ungroup must restore the original source"
    );
}

#[test]
fn group_and_ungroup_in_one_transaction_round_trip() {
    let r = run(DOC, vec![group(&["d", "b", "c"], "trio"), ungroup("trio")]);
    assert_eq!(r.source_after, r.source_before);
}

/// The trial case: a ring added at the bottom of the page, then grouped with
/// the blob listed first. The ring stays below the blob because it is
/// earlier in the document, not because of the listed order.
#[test]
fn listed_order_never_overrides_paint_order() {
    let added = run(
        DOC,
        vec![Op::AddNode {
            parent: "pg1".to_owned(),
            position: zenith_tx::Position::First,
            source: r#"ellipse id="ring" x=(px)0 y=(px)0 w=(px)50 h=(px)50 fill=(token)"color.k""#
                .to_owned(),
        }],
    );
    let grouped = run(&added.source_after, vec![group(&["a", "ring"], "deco")]);
    assert_eq!(
        id_order(&grouped.source_after, &["deco", "ring", "a"]),
        ["deco", "ring", "a"]
    );
    let ungrouped = run(&grouped.source_after, vec![ungroup("deco")]);
    assert_eq!(ungrouped.source_after, added.source_after);
}
