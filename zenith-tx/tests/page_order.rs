//! Integration tests for the page-order ops `move_page` and `swap_pages`,
//! run against `examples/multipage.zen` and a five-page fixture.

mod common;
use common::parse;
use zenith_tx::{Transaction, TxResult, TxStatus, run_transaction};

fn example(name: &str) -> String {
    let path = format!("{}/../examples/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).expect("example exists")
}

fn pages(n: usize) -> String {
    let body: String = (1..=n)
        .map(|i| format!("    page id=\"p{i}\" w=(px)100 h=(px)100 {{ }}\n"))
        .collect();
    format!(
        "zenith version=1 {{\n  project id=\"proj\" name=\"P\"\n  tokens format=\"zenith-token-v1\" {{ }}\n  styles {{ }}\n  document id=\"d\" {{\n{body}  }}\n}}\n"
    )
}

fn run(src: &str, ops: &str) -> TxResult {
    let tx = Transaction::from_json(&format!("{{\"ops\":{ops}}}")).expect("tx parses");
    run_transaction(&parse(src), &tx).expect("run_transaction")
}

fn order(result: &TxResult) -> Vec<&str> {
    result
        .document_after
        .body
        .pages
        .iter()
        .map(|p| p.id.as_str())
        .collect()
}

#[test]
fn swap_pages_trades_two_places_in_the_example() {
    let result = run(
        &example("multipage.zen"),
        r#"[{"op":"swap_pages","a":"page.one","b":"page.two"}]"#,
    );
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    assert_eq!(order(&result), ["page.two", "page.one"]);
    assert_eq!(result.affected_node_ids, ["page.one", "page.two"]);
}

#[test]
fn move_page_after_keeps_the_other_pages_in_order() {
    let result = run(
        &pages(5),
        r#"[{"op":"move_page","page":"p4","position":{"at":"after","id":"p1"}}]"#,
    );
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    assert_eq!(order(&result), ["p1", "p4", "p2", "p3", "p5"]);
}

#[test]
fn swap_is_not_a_move() {
    // The benchmark error: an agent meant to swap p2 and p4.
    let swap = run(&pages(5), r#"[{"op":"swap_pages","a":"p2","b":"p4"}]"#);
    assert_eq!(order(&swap), ["p1", "p4", "p3", "p2", "p5"]);
    let moved = run(
        &pages(5),
        r#"[{"op":"move_page","page":"p4","position":{"at":"before","id":"p2"}}]"#,
    );
    assert_eq!(order(&moved), ["p1", "p4", "p2", "p3", "p5"]);
}

#[test]
fn move_page_to_an_index_and_the_ends() {
    let result = run(
        &pages(4),
        r#"[{"op":"move_page","page":"p1","position":{"at":"index","index":2}},
            {"op":"move_page","page":"p4","position":{"at":"first"}}]"#,
    );
    assert_eq!(order(&result), ["p4", "p2", "p3", "p1"]);
}

#[test]
fn bad_page_ops_are_rejected_and_leave_the_order() {
    for ops in [
        r#"[{"op":"move_page","page":"p9","position":{"at":"first"}}]"#,
        r#"[{"op":"move_page","page":"p1","position":{"at":"index","index":3}}]"#,
        r#"[{"op":"move_page","page":"p1","position":{"at":"after","id":"p1"}}]"#,
        r#"[{"op":"swap_pages","a":"p1","b":"p1"}]"#,
        r#"[{"op":"swap_pages","a":"p1","b":"p9"}]"#,
    ] {
        let result = run(&pages(3), ops);
        assert_eq!(result.status, TxStatus::Rejected, "{ops}");
        assert_eq!(order(&result), ["p1", "p2", "p3"], "{ops}");
    }
}

#[test]
fn schema_examples_parse_as_ops() {
    for name in ["move_page", "swap_pages", "set_override", "duplicate_page"] {
        let example = zenith_tx::schema::op_example(name).expect("example");
        Transaction::from_json(&format!("{{\"ops\":[{example}]}}"))
            .unwrap_or_else(|e| panic!("{name}: {}", e.message));
    }
}

#[test]
fn duplicate_page_example_suffixes_child_ids() {
    let result = run(
        &example("multipage.zen"),
        r#"[{"op":"duplicate_page","page":"page.one","new_id":"page.one.2","id_suffix":".2"}]"#,
    );
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    assert!(result.source_after.contains(r#"text id="text.one.2""#));
}
