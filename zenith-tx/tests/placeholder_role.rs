//! The text ops of `tx` clear `role="placeholder"` on the node they change.
//! Other ops and other roles leave it alone.

mod common;
use common::parse;
use zenith_tx::{Transaction, TxResult, TxStatus, run_transaction};

const DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.k" type="color" value="#000000"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)800 h=(px)600 {
      text id="a" x=(px)10 y=(px)10 w=(px)300 h=(px)40 role="placeholder" { span "Sample A" }
      text id="b" x=(px)10 y=(px)60 w=(px)300 h=(px)40 role="placeholder" { span "Sample B" }
      text id="c" x=(px)10 y=(px)110 w=(px)300 h=(px)40 role="placeholder" { span "Sample C" }
      text id="d" x=(px)10 y=(px)160 w=(px)300 h=(px)40 role="guide" { span "Guide" }
      shape id="s" x=(px)10 y=(px)210 w=(px)100 h=(px)40 role="placeholder" { span "Sample S" }
    }
  }
}"##;

fn run(json: &str) -> TxResult {
    let tx = Transaction::from_json(json).expect("transaction JSON parses");
    let r = run_transaction(&parse(DOC), &tx).expect("run_transaction must not error");
    assert!(
        matches!(
            r.status,
            TxStatus::Accepted | TxStatus::AcceptedWithWarnings
        ),
        "{:?}",
        r.diagnostics
    );
    r
}

fn line<'s>(src: &'s str, id: &str) -> &'s str {
    let needle = format!("id=\"{id}\"");
    src.lines()
        .find(|l| l.contains(&needle))
        .unwrap_or_else(|| panic!("no line for {id}"))
}

fn is_placeholder(src: &str, id: &str) -> bool {
    line(src, id).contains("role=\"placeholder\"")
}

#[test]
fn set_span_text_clears_the_mark_on_that_node_only() {
    let r = run(r#"{"ops":[{"op":"set_span_text","node":"a","span":0,"text":"Real"}]}"#);
    assert!(!is_placeholder(&r.source_after, "a"));
    assert!(is_placeholder(&r.source_after, "b"));
    assert!(is_placeholder(&r.source_after, "c"));
}

#[test]
fn replace_text_clears_the_mark_on_a_text_node_and_a_shape() {
    let r = run(
        r#"{"ops":[{"op":"replace_text","node":"b","spans":[{"text":"Real B"}]},{"op":"replace_text","node":"s","spans":[{"text":"Real S"}]}]}"#,
    );
    assert!(!is_placeholder(&r.source_after, "b"));
    assert!(!is_placeholder(&r.source_after, "s"));
    assert!(is_placeholder(&r.source_after, "a"));
}

#[test]
fn find_replace_clears_the_mark_where_it_changed_text() {
    let scoped = run(
        r#"{"ops":[{"op":"find_replace_text","find":"Sample C","replace":"Real C","node":"c"}]}"#,
    );
    assert!(!is_placeholder(&scoped.source_after, "c"));
    assert!(is_placeholder(&scoped.source_after, "a"));
    let doc_wide =
        run(r#"{"ops":[{"op":"find_replace_text","find":"Sample A","replace":"Real A"}]}"#);
    assert!(!is_placeholder(&doc_wide.source_after, "a"));
    assert!(is_placeholder(&doc_wide.source_after, "b"));
}

#[test]
fn other_ops_and_other_roles_keep_their_role() {
    let r = run(
        r#"{"ops":[{"op":"set_fill","node":"a","fill":"color.k"},{"op":"set_span_text","node":"d","span":0,"text":"Changed"}]}"#,
    );
    assert!(
        is_placeholder(&r.source_after, "a"),
        "a fill is not new content"
    );
    assert!(line(&r.source_after, "d").contains("role=\"guide\""));
}

#[test]
fn an_op_that_changes_nothing_keeps_the_mark() {
    let no_match = run(
        r#"{"ops":[{"op":"find_replace_text","find":"No such text","replace":"X","node":"a"}]}"#,
    );
    assert!(is_placeholder(&no_match.source_after, "a"));
    let tx = Transaction::from_json(
        r#"{"ops":[{"op":"set_span_text","node":"a","span":7,"text":"Nope"}]}"#,
    )
    .expect("transaction JSON parses");
    let r = run_transaction(&parse(DOC), &tx).expect("run_transaction must not error");
    assert_eq!(r.status, TxStatus::Rejected);
    assert!(is_placeholder(&r.source_before, "a"));
}
