//! `set_fill`, `set_stroke` and `set_stroke_width` on a `frame`, whose box
//! paints a fill and a stroke under its children (a button is a frame).

mod common;
use common::parse;
use zenith_tx::{Transaction, TxResult, TxStatus, run_transaction};

const DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#111111"
    token id="color.hot" type="color" value="#e11d48"
    token id="width.thin" type="dimension" value=(px)2
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      frame id="button" x=(px)10 y=(px)10 w=(px)120 h=(px)40 fill=(token)"color.bg" {
        rect id="inner" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.bg"
      }
      group id="g" {
        rect id="member" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.bg"
      }
    }
  }
}"##;

fn run(json: &str) -> TxResult {
    let tx = Transaction::from_json(json).expect("transaction JSON parses");
    run_transaction(&parse(DOC), &tx).expect("run_transaction must not error")
}

fn line_of<'s>(src: &'s str, id: &str) -> &'s str {
    let needle = format!("id=\"{id}\"");
    src.lines()
        .find(|l| l.contains(&needle))
        .unwrap_or_else(|| panic!("no line for {id}"))
}

#[test]
fn set_fill_recolors_a_frame() {
    let r = run(r#"{"ops":[{"op":"set_fill","node":"button","fill":"color.hot"}]}"#);
    assert_ne!(r.status, TxStatus::Rejected, "{:?}", r.diagnostics);
    assert!(line_of(&r.source_after, "button").contains("fill=(token)\"color.hot\""));
    assert!(line_of(&r.source_after, "inner").contains("fill=(token)\"color.bg\""));
}

#[test]
fn set_stroke_and_width_apply_to_a_frame() {
    let r = run(r#"{"ops":[
            {"op":"set_stroke","node":"button","stroke":"color.hot"},
            {"op":"set_stroke_width","node":"button","stroke_width":"width.thin"}
        ]}"#);
    assert_ne!(r.status, TxStatus::Rejected, "{:?}", r.diagnostics);
    let line = line_of(&r.source_after, "button");
    assert!(line.contains("stroke=(token)\"color.hot\""), "{line}");
    assert!(
        line.contains("stroke-width=(token)\"width.thin\""),
        "{line}"
    );
}

#[test]
fn a_group_still_has_no_fill() {
    let r = run(r#"{"ops":[{"op":"set_fill","node":"g","fill":"color.hot"}]}"#);
    assert_eq!(r.status, TxStatus::Rejected);
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.code == "tx.unsupported_property"),
        "{:?}",
        r.diagnostics
    );
}
