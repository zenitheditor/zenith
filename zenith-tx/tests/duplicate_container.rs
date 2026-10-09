//! `duplicate_node` on containers: frame, group, instance, table. The copy gets
//! fresh ids for the whole subtree, references inside the subtree follow to the
//! copies, and references to nodes outside the subtree stay unchanged.

use zenith_core::{Document, KdlAdapter, KdlSource, Node, Severity, validate};
use zenith_tx::{Transaction, TxResult, TxStatus, run_transaction};

const DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.a" type="color" value="#ff0000"
  }
  styles { }
  components {
    component id="badge" {
      rect id="face" x=(px)0 y=(px)0 w=(px)20 h=(px)20 fill=(token)"color.a"
    }
  }
  document id="doc1" title="T" {
    page id="pg1" w=(px)600 h=(px)600 {
      ports {
        port node="a" id="p" anchor="top-right"
      }
      rect id="outside" x=(px)0 y=(px)0 w=(px)40 h=(px)40
      frame id="box" anchor-sibling="outside" anchor-edge="below" anchor-gap=(px)8 w=(px)200 h=(px)200 {
        rect id="a" x=(px)0 y=(px)0 w=(px)40 h=(px)40
        rect id="b" anchor-sibling="a" anchor-edge="below" anchor-gap=(px)8 w=(px)40 h=(px)40
        connector id="k.in" from="a#p" to="b"
        frame id="inner" x=(px)100 y=(px)0 w=(px)80 h=(px)80 {
          rect id="deep" x=(px)0 y=(px)0 w=(px)10 h=(px)10
          connector id="k.up" from="deep" to="a"
        }
        connector id="k.mixed" from="a" to="outside"
        connector id="k.self" from="box" to="b"
      }
      group id="grp" {
        rect id="g1" x=(px)300 y=(px)0 w=(px)20 h=(px)20
        rect id="g2" anchor-sibling="g1" anchor-edge="after" anchor-gap=(px)4 w=(px)20 h=(px)20
      }
      instance id="inst" component="badge" x=(px)300 y=(px)100
      table id="tbl" x=(px)300 y=(px)200 w=(px)200 h=(px)100 {
        column width=(px)100
        column width=(px)100
        row {
          cell {
            rect id="t1" x=(px)0 y=(px)0 w=(px)10 h=(px)10
            rect id="t2" anchor-sibling="t1" anchor-edge="after" w=(px)10 h=(px)10
          }
          cell {
            rect id="t3" x=(px)0 y=(px)0 w=(px)10 h=(px)10
          }
        }
      }
      rect id="after" anchor-sibling="box" anchor-edge="below" w=(px)10 h=(px)10
      connector id="link" from="a" to="box"
    }
  }
}"##;

fn parse(src: &str) -> Document {
    KdlAdapter
        .parse(src.as_bytes())
        .expect("test doc must parse")
}

fn run(src: &str, ops: &[&str]) -> TxResult {
    let doc = parse(src);
    let tx = Transaction::from_json(&format!(r#"{{"ops":[{}]}}"#, ops.join(","))).expect("tx");
    run_transaction(&doc, &tx).expect("run_transaction")
}

fn dup(node: &str, new_id: &str) -> String {
    format!(r#"{{"op":"duplicate_node","node":"{node}","new_id":"{new_id}"}}"#)
}

fn accepted(src: &str, ops: &[&str]) -> TxResult {
    let r = run(src, ops);
    assert_eq!(r.status, TxStatus::Accepted, "{:?}", r.diagnostics);
    assert_clean(&r.document_after);
    r
}

/// The validator reports no Error and no anchor diagnostic.
fn assert_clean(doc: &Document) {
    let report = validate(doc);
    for d in &report.diagnostics {
        assert!(
            d.severity != Severity::Error && !d.code.starts_with("anchor."),
            "unexpected diagnostic {}: {}",
            d.code,
            d.message
        );
    }
}

fn find<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Node> {
    for n in nodes {
        if n.id() == Some(id) {
            return Some(n);
        }
        if let Some(c) = n.children()
            && let Some(f) = find(c, id)
        {
            return Some(f);
        }
        if let Node::Table(t) = n {
            for row in &t.rows {
                for cell in &row.cells {
                    if let Some(f) = find(&cell.children, id) {
                        return Some(f);
                    }
                }
            }
        }
    }
    None
}

fn page_children(doc: &Document) -> &[Node] {
    &doc.body.pages[0].children
}

fn node<'a>(doc: &'a Document, id: &str) -> &'a Node {
    find(page_children(doc), id).unwrap_or_else(|| panic!("node {id} missing"))
}

fn rect_sibling<'a>(doc: &'a Document, id: &str) -> Option<&'a str> {
    match node(doc, id) {
        Node::Rect(r) => r.anchor_sibling.as_deref(),
        other => panic!("{id} is {}", other.kind_str()),
    }
}

fn connector_ends<'a>(doc: &'a Document, id: &str) -> (&'a str, &'a str) {
    match node(doc, id) {
        Node::Connector(c) => (
            c.from.as_deref().expect("from"),
            c.to.as_deref().expect("to"),
        ),
        other => panic!("{id} is {}", other.kind_str()),
    }
}

fn all_ids(nodes: &[Node], out: &mut Vec<String>) {
    for n in nodes {
        if let Some(id) = n.id() {
            out.push(id.to_owned());
        }
        if let Some(c) = n.children() {
            all_ids(c, out);
        }
        if let Node::Table(t) = n {
            for row in &t.rows {
                for cell in &row.cells {
                    all_ids(&cell.children, out);
                }
            }
        }
    }
}

#[test]
fn frame_copy_has_fresh_ids_and_inside_references() {
    let r = accepted(DOC, &[&dup("box", "box-copy")]);
    let doc = &r.document_after;
    // Position: directly after the original.
    let top: Vec<_> = page_children(doc).iter().filter_map(Node::id).collect();
    let at = top.iter().position(|i| *i == "box").expect("box");
    assert_eq!(top.get(at + 1), Some(&"box-copy"));
    // Every descendant has the suffix.
    for id in ["a-copy", "b-copy", "inner-copy", "deep-copy", "k.in-copy"] {
        node(doc, id);
    }
    // Inside references follow, outside references stay.
    assert_eq!(rect_sibling(doc, "b-copy"), Some("a-copy"));
    assert_eq!(rect_sibling(doc, "b"), Some("a"));
    assert_eq!(connector_ends(doc, "k.in-copy"), ("a-copy#p", "b-copy"));
    assert_eq!(connector_ends(doc, "k.up-copy"), ("deep-copy", "a-copy"));
    assert_eq!(connector_ends(doc, "k.mixed-copy"), ("a-copy", "outside"));
    assert_eq!(connector_ends(doc, "k.self-copy"), ("box-copy", "b-copy"));
    // The copy's own anchor to an outside sibling is kept.
    match node(doc, "box-copy") {
        Node::Frame(f) => assert_eq!(f.anchor_sibling.as_deref(), Some("outside")),
        other => panic!("{}", other.kind_str()),
    }
    // Nodes outside the subtree keep pointing at the original.
    assert_eq!(rect_sibling(doc, "after"), Some("box"));
    assert_eq!(connector_ends(doc, "link"), ("a", "box"));
    // The original is unchanged.
    assert_eq!(connector_ends(doc, "k.mixed"), ("a", "outside"));
    assert_eq!(r.affected_node_ids, vec!["box-copy".to_owned()]);
}

#[test]
fn page_port_follows_the_copied_node() {
    let r = accepted(DOC, &[&dup("box", "box-copy")]);
    let ports = &r.document_after.body.pages[0].ports;
    let nodes: Vec<_> = ports.iter().map(|p| p.node.as_str()).collect();
    assert_eq!(nodes, vec!["a", "a-copy"]);
}

#[test]
fn group_copy() {
    let r = accepted(DOC, &[&dup("grp", "grp-copy")]);
    let doc = &r.document_after;
    assert_eq!(rect_sibling(doc, "g2-copy"), Some("g1-copy"));
    assert_eq!(rect_sibling(doc, "g2"), Some("g1"));
}

#[test]
fn instance_copy() {
    let r = accepted(DOC, &[&dup("inst", "inst-copy")]);
    match node(&r.document_after, "inst-copy") {
        Node::Instance(i) => assert_eq!(i.component.as_deref(), Some("badge")),
        other => panic!("{}", other.kind_str()),
    }
}

#[test]
fn table_copy_re_ids_cell_children() {
    let r = accepted(DOC, &[&dup("tbl", "tbl-copy")]);
    let doc = &r.document_after;
    for id in ["t1-copy", "t2-copy", "t3-copy"] {
        node(doc, id);
    }
    assert_eq!(rect_sibling(doc, "t2-copy"), Some("t1-copy"));
    assert_eq!(rect_sibling(doc, "t2"), Some("t1"));
}

#[test]
fn nested_container_copy() {
    let r = accepted(DOC, &[&dup("inner", "inner-copy")]);
    let doc = &r.document_after;
    assert_eq!(connector_ends(doc, "k.up-copy"), ("deep-copy", "a"));
    assert_eq!(connector_ends(doc, "k.up"), ("deep", "a"));
}

#[test]
fn duplicate_twice_gives_distinct_ids() {
    // Same new_id suffix twice (second copy is of the original again).
    let r = accepted(DOC, &[&dup("box", "box-copy"), &dup("box", "box-copy2")]);
    let mut ids = Vec::new();
    all_ids(page_children(&r.document_after), &mut ids);
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(ids.len(), sorted.len(), "ids must be unique: {ids:?}");
    node(&r.document_after, "a-copy2");
}

#[test]
fn duplicate_a_copy_and_clashing_suffix() {
    // "a-copy" exists as a sibling already, so the descendant suffix moves on.
    let src = DOC.replace(
        "      rect id=\"after\"",
        "      rect id=\"a-copy\" x=(px)500 y=(px)500 w=(px)5 h=(px)5\n      rect id=\"after\"",
    );
    let r = accepted(&src, &[&dup("box", "box-copy")]);
    node(&r.document_after, "a-copy2");
    node(&r.document_after, "box-copy");
    assert_eq!(
        connector_ends(&r.document_after, "k.mixed-copy2"),
        ("a-copy2", "outside")
    );
    let copy_of_copy = accepted(&r.source_after, &[&dup("box-copy", "box-copy-copy")]);
    node(&copy_of_copy.document_after, "a-copy2-copy");
}

#[test]
fn new_id_without_source_prefix() {
    let r = accepted(DOC, &[&dup("grp", "other")]);
    let doc = &r.document_after;
    assert_eq!(rect_sibling(doc, "g2.other"), Some("g1.other"));
}

#[test]
fn locked_container_can_be_duplicated() {
    let src = DOC.replace("group id=\"grp\"", "group id=\"grp\" locked=#true");
    let r = accepted(&src, &[&dup("grp", "grp-copy")]);
    match node(&r.document_after, "grp-copy") {
        Node::Group(g) => assert_eq!(g.locked, Some(true)),
        other => panic!("{}", other.kind_str()),
    }
}

#[test]
fn colliding_new_id_is_rejected() {
    let r = run(DOC, &[&dup("box", "grp")]);
    assert_eq!(r.status, TxStatus::Rejected);
    assert!(r.diagnostics.iter().any(|d| d.code == "id.duplicate"));
    assert_eq!(r.source_after, r.source_before);
}

#[test]
fn removing_an_anchor_target_still_reports_unresolved_sibling() {
    let r = run(DOC, &[r#"{"op":"remove_node","node":"outside"}"#]);
    assert_eq!(r.status, TxStatus::Rejected);
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.code == "anchor.unresolved_sibling"),
        "{:?}",
        r.diagnostics
    );
}

#[test]
fn page_duplicate_keeps_ports_in_step() {
    let r = accepted(
        DOC,
        &[r#"{"op":"duplicate_page","page":"pg1","new_id":"pg2","id_suffix":".v2"}"#],
    );
    let ports = &r.document_after.body.pages[1].ports;
    assert_eq!(ports.len(), 1);
    assert_eq!(ports[0].node, "a.v2");
}
