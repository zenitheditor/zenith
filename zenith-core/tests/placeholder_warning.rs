//! `content.placeholder`: text nodes with `role="placeholder"`, or a span that
//! is exactly `<new text>`, give one document-level warning.

use zenith_core::{Diagnostic, KdlAdapter, KdlSource as _, Severity, validate};

/// A document with `policy` before `document`, `masters` and `page_body`.
fn diagnostics_of(policy: &str, masters: &str, page_body: &str) -> Vec<Diagnostic> {
    let src = format!(
        "zenith version=1 {{\n  project id=\"p\" name=\"P\"\n  tokens format=\"zenith-token-v1\" {{\n  }}\n  styles {{\n  }}\n{policy}{masters}  document id=\"d\" title=\"T\" {{\n    page id=\"pg\" w=(px)200 h=(px)100 {{\n{page_body}    }}\n  }}\n}}\n"
    );
    let doc = KdlAdapter.parse(src.as_bytes()).expect("parse");
    validate(&doc).diagnostics
}

fn placeholder(found: &[Diagnostic]) -> Vec<&Diagnostic> {
    found
        .iter()
        .filter(|d| d.code == "content.placeholder")
        .collect()
}

fn text(id: &str, attrs: &str, content: &str) -> String {
    format!(
        "      text id=\"{id}\" x=(px)0 y=(px)0 w=(px)50 h=(px)10 {attrs} {{ span \"{content}\" }}\n"
    )
}

#[test]
fn a_placeholder_text_node_gives_one_warning_with_no_subject() {
    let found = diagnostics_of("", "", &text("t", r#"role="placeholder""#, "Sample"));
    let found = placeholder(&found);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].severity, Severity::Warning);
    assert_eq!(found[0].subject_id, None, "the warning is document-level");
    assert!(found[0].span.is_some(), "the warning points at a node");
    assert!(found[0].message.contains("t"), "{}", found[0].message);
}

#[test]
fn many_placeholders_are_one_diagnostic_naming_the_first_few() {
    let nodes: String = (0..9)
        .map(|i| text(&format!("t{i}"), r#"role="placeholder""#, "S"))
        .collect();
    let found = diagnostics_of("", "", &nodes);
    let found = placeholder(&found);
    assert_eq!(found.len(), 1, "{found:?}");
    let message = &found[0].message;
    assert!(message.starts_with("9 text node(s)"), "{message}");
    assert!(
        message.contains("t0, t1, t2, t3, t4, t5 and 3 more"),
        "{message}"
    );
}

#[test]
fn the_fill_command_text_counts_without_the_role() {
    let found = diagnostics_of("", "", &text("t", "", "<new text>"));
    assert_eq!(placeholder(&found).len(), 1, "{found:?}");
}

#[test]
fn a_master_and_a_table_cell_count() {
    let master = "  masters {\n    master id=\"m\" {\n      text id=\"foot\" x=(px)0 y=(px)0 w=(px)50 h=(px)10 role=\"placeholder\" { span \"Foot\" }\n    }\n  }\n";
    let found = diagnostics_of("", master, "");
    assert_eq!(placeholder(&found).len(), 1, "master: {found:?}");

    let table = "      table id=\"tb\" x=(px)0 y=(px)0 w=(px)100 h=(px)40 {\n        row {\n          cell { text id=\"c1\" role=\"placeholder\" { span \"Cell\" } }\n        }\n      }\n";
    let found = diagnostics_of("", "", table);
    assert_eq!(placeholder(&found).len(), 1, "table cell: {found:?}");
}

#[test]
fn only_text_and_shape_nodes_count() {
    let rect = "      rect id=\"r\" x=(px)0 y=(px)0 w=(px)10 h=(px)10 role=\"placeholder\"\n";
    let found = diagnostics_of("", "", rect);
    assert!(placeholder(&found).is_empty(), "{found:?}");
}

#[test]
fn other_text_does_not_warn() {
    for attrs in ["", r#"role="guide""#] {
        let found = diagnostics_of("", "", &text("t", attrs, "Real"));
        assert!(placeholder(&found).is_empty(), "{attrs}: {found:?}");
    }
}

#[test]
fn a_policy_can_allow_and_deny_the_warning() {
    let body = text("t", r#"role="placeholder""#, "Sample");
    let allowed = diagnostics_of(
        "  diagnostics {\n    allow \"content.placeholder\"\n  }\n",
        "",
        &body,
    );
    assert!(placeholder(&allowed).is_empty(), "{allowed:?}");
    let denied = diagnostics_of(
        "  diagnostics {\n    deny \"content.placeholder\"\n  }\n",
        "",
        &body,
    );
    assert_eq!(placeholder(&denied).len(), 1);
    assert_eq!(placeholder(&denied)[0].severity, Severity::Error);
}
