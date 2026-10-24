//! Integration tests for instance overrides and component children:
//! `set_override`, `find_replace_text` on instances, and node ops that
//! address a component child by its id. Runs against
//! `examples/connector-ports.zen`, which declares one component and one
//! instance.

mod common;
use common::parse;
use zenith_core::{Document, Node, Severity};
use zenith_tx::{Transaction, TxResult, TxStatus, run_transaction};

const CARDS: &str = r##"zenith version=1 {
  project id="proj.c" name="C"
  tokens format="zenith-token-v1" {
    token id="color.card" type="color" value="#e7e5e4"
    token id="color.accent" type="color" value="#2563eb"
  }
  styles {}
  components {
    component id="card" {
      rect id="card.bg" x=(px)0 y=(px)0 w=(px)200 h=(px)200 fill=(token)"color.card"
      text id="card.title" x=(px)20 y=(px)20 w=(px)160 h=(px)30 { span "Title" }
      text id="card.body" x=(px)20 y=(px)60 w=(px)160 h=(px)60 { span "Body" }
    }
  }
  document id="doc.c" title="C" {
    page id="p1" w=(px)800 h=(px)400 {
      instance id="c1" component="card" x=(px)0 y=(px)0 {
        override ref="card.title" { span "Fast setup" }
      }
      instance id="c2" component="card" x=(px)300 y=(px)0 {
        override ref="card.title" { span "Shared library" }
        override ref="card.bg" visible=#false
      }
    }
  }
}
"##;

fn example(name: &str) -> String {
    let path = format!("{}/../examples/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).expect("example exists")
}

fn run(src: &str, ops: &str) -> TxResult {
    let tx = Transaction::from_json(&format!("{{\"ops\":{ops}}}")).expect("tx parses");
    run_transaction(&parse(src), &tx).expect("run_transaction")
}

fn accepted(result: &TxResult) {
    assert_ne!(
        result.status,
        TxStatus::Rejected,
        "diagnostics: {:?}",
        result.diagnostics
    );
}

/// The Error codes of `result`. The fixture leaves a token unused, which
/// only warns.
fn codes(result: &TxResult) -> Vec<&str> {
    result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.code.as_str())
        .collect()
}

fn instance_overrides<'d>(doc: &'d Document, page: usize, id: &str) -> &'d [zenith_core::Override] {
    doc.body.pages[page]
        .children
        .iter()
        .find_map(|n| match n {
            Node::Instance(i) if i.id == id => Some(i.overrides.as_slice()),
            _ => None,
        })
        .expect("instance on page")
}

#[test]
fn set_override_changes_one_instance_only() {
    let result = run(
        CARDS,
        r#"[{"op":"set_override","instance":"c2","ref":"card.body","text":"Logos live here."}]"#,
    );
    accepted(&result);
    assert_eq!(result.affected_node_ids, ["c2"]);
    assert!(
        result.source_after.contains(r#"span "Logos live here.""#),
        "{}",
        result.source_after
    );
    assert!(instance_overrides(&result.document_after, 0, "c1").len() == 1);
}

#[test]
fn set_override_fill_visible_and_removal_round_trip() {
    let result = run(
        CARDS,
        r#"[{"op":"set_override","instance":"c1","ref":"card.title","fill":"color.accent"},
            {"op":"set_override","instance":"c2","ref":"card.bg","visible":null}]"#,
    );
    accepted(&result);
    assert!(
        result
            .source_after
            .contains(r#"override ref="card.title" fill=(token)"color.accent" {"#),
        "{}",
        result.source_after
    );
    // The card.bg override had only `visible`: clearing it removes the override.
    assert!(!result.source_after.contains(r#"ref="card.bg""#));
}

#[test]
fn set_override_rejects_unknown_refs_and_bad_targets() {
    let result = run(
        CARDS,
        r#"[{"op":"set_override","instance":"c1","ref":"card.nope","visible":true}]"#,
    );
    assert_eq!(result.status, TxStatus::Rejected);
    assert_eq!(codes(&result), ["tx.unknown_node"]);

    let result = run(
        CARDS,
        r#"[{"op":"set_override","instance":"c1","ref":"card.bg","text":"x"}]"#,
    );
    assert_eq!(codes(&result), ["tx.wrong_node_type"]);

    let result = run(
        CARDS,
        r#"[{"op":"set_override","instance":"c1","ref":"card.bg"}]"#,
    );
    assert_eq!(codes(&result), ["tx.invalid_value"]);
}

#[test]
fn set_override_on_a_locked_instance_is_rejected() {
    let src = CARDS.replace(
        r#"instance id="c1" component="card" x=(px)0 y=(px)0 {"#,
        r#"instance id="c1" component="card" x=(px)0 y=(px)0 locked=#true {"#,
    );
    let result = run(
        &src,
        r#"[{"op":"set_override","instance":"c1","ref":"card.title","text":"x"}]"#,
    );
    assert_eq!(result.status, TxStatus::Rejected);
    assert_eq!(codes(&result), ["node.locked"]);
}

#[test]
fn node_ops_reach_component_children() {
    let result = run(
        CARDS,
        r#"[{"op":"set_geometry","node":"card.body","h":150},
            {"op":"set_fill","node":"card.bg","fill":"color.accent"}]"#,
    );
    accepted(&result);
    assert_eq!(result.affected_node_ids, ["card.body", "card.bg"]);
    assert!(
        result
            .source_after
            .contains(r#"text id="card.body" x=(px)20 y=(px)60 w=(px)160 h=(px)150"#),
        "{}",
        result.source_after
    );
}

#[test]
fn add_node_into_a_component_and_remove_it() {
    let result = run(
        CARDS,
        r#"[{"op":"add_node","parent":"card","source":"rect id=\"card.rule\" x=(px)20 y=(px)50 w=(px)160 h=(px)2 fill=(token)\"color.accent\""}]"#,
    );
    accepted(&result);
    assert!(result.source_after.contains(r#"rect id="card.rule""#));

    let result = run(CARDS, r#"[{"op":"remove_node","node":"card.body"}]"#);
    accepted(&result);
    assert!(!result.source_after.contains(r#"text id="card.body""#));
}

#[test]
fn find_replace_scoped_to_an_instance_adds_a_text_override() {
    let result = run(
        CARDS,
        r#"[{"op":"find_replace_text","find":"Body","replace":"Templates for teams","node":"c1"}]"#,
    );
    accepted(&result);
    assert_eq!(result.affected_node_ids, ["c1"]);
    let c1 = instance_overrides(&result.document_after, 0, "c1");
    assert!(c1.iter().any(|o| o.ref_id == "card.body"));
    // c2 and the component keep their text.
    assert!(
        !instance_overrides(&result.document_after, 0, "c2")
            .iter()
            .any(|o| o.ref_id == "card.body")
    );
    assert!(
        result
            .source_after
            .contains(r#"span "Templates for teams""#)
    );
    assert!(result.source_after.contains(r#"span "Body""#));
}

#[test]
fn find_replace_document_wide_edits_override_text() {
    let result = run(
        CARDS,
        r#"[{"op":"find_replace_text","find":"Shared","replace":"Common"}]"#,
    );
    accepted(&result);
    assert_eq!(result.affected_node_ids, ["c2"]);
    assert!(result.source_after.contains(r#"span "Common library""#));
}

#[test]
fn connector_ports_example_takes_an_override_and_a_component_edit() {
    let src = example("connector-ports.zen");
    let result = run(
        &src,
        r#"[{"op":"set_override","instance":"agent","ref":"body","fill":"color.node"},
            {"op":"find_replace_text","find":"agent","replace":"planner","node":"body"}]"#,
    );
    accepted(&result);
    assert_eq!(result.affected_node_ids, ["agent", "body"]);
    assert!(
        result
            .source_after
            .contains(r#"instance id="agent" component="agent.node" x=(px)120 y=(px)96 {"#),
        "{}",
        result.source_after
    );
    assert!(
        result
            .source_after
            .contains(r#"override ref="body" fill=(token)"color.node""#)
    );
    assert!(result.source_after.contains(r#"span "planner""#));
}
