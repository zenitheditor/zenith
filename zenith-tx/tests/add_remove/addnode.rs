use super::*;

/// Page with one rect; an accent color token declared so added rects that
/// reference it pass post-validation.
const ADD_BASE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.accent" type="color" value="#3b82f6"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)320 h=(px)200 {
      rect id="base" x=(px)0 y=(px)0 w=(px)100 h=(px)100
    }
  }
}"##;

// ── AddNode tests ─────────────────────────────────────────────────────────

#[test]
fn add_node_into_page_last() {
    let doc = parse(ADD_BASE_DOC);
    let tx = Transaction {
        ops: vec![Op::AddNode {
            parent: "pg1".to_owned(),
            position: Position::Last,
            source:
                r#"rect id="box" x=(px)10 y=(px)10 w=(px)100 h=(px)80 fill=(token)"color.accent""#
                    .to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    assert_eq!(result.affected_node_ids, vec!["box".to_owned()]);
    assert!(
        result.source_after.contains("id=\"box\""),
        "source_after must contain the new rect; got:\n{}",
        result.source_after
    );
    // "box" inserted last → appears after "base".
    let pos_base = result.source_after.find("id=\"base\"").expect("base");
    let pos_box = result.source_after.find("id=\"box\"").expect("box");
    assert!(pos_base < pos_box, "box should come after base");
}

#[test]
fn add_node_into_group_first() {
    let doc = parse(ADD_GROUP_DOC);
    let tx = Transaction {
        ops: vec![Op::AddNode {
            parent: "grp1".to_owned(),
            position: Position::First,
            source: r#"rect id="g.new" x=(px)0 y=(px)0 w=(px)10 h=(px)10"#.to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    assert_eq!(result.affected_node_ids, vec!["g.new".to_owned()]);
    // First child of the group → appears before g.a.
    let pos_new = result.source_after.find("id=\"g.new\"").expect("g.new");
    let pos_a = result.source_after.find("id=\"g.a\"").expect("g.a");
    assert!(pos_new < pos_a, "g.new should be first in the group");
}

#[test]
fn add_node_before_and_after_sibling() {
    // Insert before g.b.
    let doc = parse(ADD_GROUP_DOC);
    let tx = Transaction {
        ops: vec![Op::AddNode {
            parent: "grp1".to_owned(),
            position: Position::Before {
                id: "g.b".to_owned(),
            },
            source: r#"rect id="g.mid" x=(px)0 y=(px)0 w=(px)10 h=(px)10"#.to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    let pos_a = result.source_after.find("id=\"g.a\"").expect("g.a");
    let pos_mid = result.source_after.find("id=\"g.mid\"").expect("g.mid");
    let pos_b = result.source_after.find("id=\"g.b\"").expect("g.b");
    assert!(
        pos_a < pos_mid && pos_mid < pos_b,
        "order should be a, mid, b"
    );

    // Insert after g.a.
    let doc = parse(ADD_GROUP_DOC);
    let tx = Transaction {
        ops: vec![Op::AddNode {
            parent: "grp1".to_owned(),
            position: Position::After {
                id: "g.a".to_owned(),
            },
            source: r#"rect id="g.mid" x=(px)0 y=(px)0 w=(px)10 h=(px)10"#.to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    let pos_a = result.source_after.find("id=\"g.a\"").expect("g.a");
    let pos_mid = result.source_after.find("id=\"g.mid\"").expect("g.mid");
    let pos_b = result.source_after.find("id=\"g.b\"").expect("g.b");
    assert!(
        pos_a < pos_mid && pos_mid < pos_b,
        "order should be a, mid, b"
    );
}

#[test]
fn add_node_index_clamped() {
    // index well beyond len → clamped to last.
    let doc = parse(ADD_GROUP_DOC);
    let tx = Transaction {
        ops: vec![Op::AddNode {
            parent: "grp1".to_owned(),
            position: Position::Index { index: 99 },
            source: r#"rect id="g.tail" x=(px)0 y=(px)0 w=(px)10 h=(px)10"#.to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    let pos_b = result.source_after.find("id=\"g.b\"").expect("g.b");
    let pos_tail = result.source_after.find("id=\"g.tail\"").expect("g.tail");
    assert!(pos_b < pos_tail, "clamped insert should be last");
}

#[test]
fn add_node_duplicate_id_rejected() {
    let doc = parse(ADD_BASE_DOC);
    let before = run_transaction(
        &doc,
        &Transaction {
            ops: vec![Op::AddNode {
                parent: "pg1".to_owned(),
                position: Position::Last,
                source: r#"rect id="base" x=(px)0 y=(px)0 w=(px)20 h=(px)20"#.to_owned(),
            }],
            permissions: Permissions::default(),
        },
    )
    .expect("run_transaction should not error");

    assert_eq!(before.status, TxStatus::Rejected);
    assert_eq!(before.source_after, before.source_before);
}

#[test]
fn add_node_malformed_fragment_rejected() {
    let doc = parse(ADD_BASE_DOC);
    let tx = Transaction {
        ops: vec![Op::AddNode {
            parent: "pg1".to_owned(),
            position: Position::Last,
            source: "not valid kdl {{{".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_node_spec"),
        "expected tx.invalid_node_spec; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn add_node_unknown_parent_rejected() {
    let doc = parse(ADD_BASE_DOC);
    let tx = Transaction {
        ops: vec![Op::AddNode {
            parent: "nope".to_owned(),
            position: Position::Last,
            source: r#"rect id="box" x=(px)0 y=(px)0 w=(px)10 h=(px)10"#.to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_parent"),
        "expected tx.invalid_parent; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn add_node_parent_is_leaf_rejected() {
    // "base" is a rect (a leaf) — not a valid container.
    let doc = parse(ADD_BASE_DOC);
    let tx = Transaction {
        ops: vec![Op::AddNode {
            parent: "base".to_owned(),
            position: Position::Last,
            source: r#"rect id="box" x=(px)0 y=(px)0 w=(px)10 h=(px)10"#.to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_parent"),
        "expected tx.invalid_parent; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn add_node_before_missing_sibling_rejected() {
    let doc = parse(ADD_GROUP_DOC);
    let tx = Transaction {
        ops: vec![Op::AddNode {
            parent: "grp1".to_owned(),
            position: Position::Before {
                id: "nope".to_owned(),
            },
            source: r#"rect id="g.new" x=(px)0 y=(px)0 w=(px)10 h=(px)10"#.to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unknown_node"),
        "expected tx.unknown_node; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn add_code_node_into_page_accepted() {
    let doc = parse(CODE_DOC);
    let tx = Transaction {
        ops: vec![Op::AddNode {
            parent: "pg1".to_owned(),
            position: Position::Last,
            source:
                r#"code id="snip2" x=(px)0 y=(px)0 w=(px)100 h=(px)40 { content "let x = 1;" }"#
                    .to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    assert_eq!(result.affected_node_ids, vec!["snip2".to_owned()]);
    assert!(
        result.source_after.contains("id=\"snip2\""),
        "source_after must contain the new code node; got:\n{}",
        result.source_after
    );
    assert!(result.source_after.contains("content \"let x = 1;\""));
}

#[test]
fn add_path_direct_into_page() {
    let doc = parse(ADD_BASE_DOC);
    let tx = Transaction {
        ops: vec![Op::AddPath {
            parent: "pg1".to_owned(),
            id: "path.direct".to_owned(),
            position: Position::Last,
            closed: Some(true),
            anchors: vec![
                OpPathAnchor {
                    x: 0.0,
                    y: 0.0,
                    kind: Some("corner".to_owned()),
                    in_x: None,
                    in_y: None,
                    out_x: None,
                    out_y: None,
                },
                OpPathAnchor {
                    x: 100.0,
                    y: 0.0,
                    kind: Some("smooth".to_owned()),
                    in_x: Some(80.0),
                    in_y: Some(0.0),
                    out_x: Some(100.0),
                    out_y: Some(20.0),
                },
                OpPathAnchor {
                    x: 100.0,
                    y: 80.0,
                    kind: None,
                    in_x: None,
                    in_y: None,
                    out_x: None,
                    out_y: None,
                },
            ],
            subpaths: Vec::new(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    assert_eq!(result.affected_node_ids, vec!["path.direct".to_owned()]);
    assert!(
        result
            .source_after
            .contains("path id=\"path.direct\" closed=#true")
    );
    assert!(
        result
            .source_after
            .contains("anchor x=(px)0 y=(px)0 kind=\"corner\"")
    );
    assert!(result.source_after.contains(
        "anchor x=(px)100 y=(px)0 kind=\"smooth\" in-x=(px)80 in-y=(px)0 out-x=(px)100 out-y=(px)20"
    ));
    let pos_base = result.source_after.find("id=\"base\"").expect("base");
    let pos_path = result
        .source_after
        .find("id=\"path.direct\"")
        .expect("path.direct");
    assert!(pos_base < pos_path, "path.direct should come after base");
}

#[test]
fn add_path_compound_into_group_first() {
    let doc = parse(ADD_GROUP_DOC);
    let tx = Transaction {
        ops: vec![Op::AddPath {
            parent: "grp1".to_owned(),
            id: "path.compound".to_owned(),
            position: Position::First,
            closed: None,
            anchors: Vec::new(),
            subpaths: vec![
                OpPathSubpath {
                    closed: Some(true),
                    anchors: vec![
                        path_anchor(0.0, 0.0),
                        path_anchor(40.0, 0.0),
                        path_anchor(40.0, 40.0),
                    ],
                },
                OpPathSubpath {
                    closed: Some(true),
                    anchors: vec![
                        path_anchor(10.0, 10.0),
                        path_anchor(20.0, 10.0),
                        path_anchor(20.0, 20.0),
                    ],
                },
            ],
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    assert_eq!(result.affected_node_ids, vec!["path.compound".to_owned()]);
    assert!(result.source_after.contains("path id=\"path.compound\""));
    assert!(result.source_after.contains("subpath closed=#true"));
    assert!(
        !result
            .source_after
            .contains("path id=\"path.compound\" closed=")
    );
    let pos_path = result
        .source_after
        .find("id=\"path.compound\"")
        .expect("path.compound");
    let pos_a = result.source_after.find("id=\"g.a\"").expect("g.a");
    assert!(
        pos_path < pos_a,
        "path.compound should be first in the group"
    );
}

#[test]
fn add_path_rejects_direct_and_compound_payload() {
    let doc = parse(ADD_BASE_DOC);
    let tx = Transaction {
        ops: vec![Op::AddPath {
            parent: "pg1".to_owned(),
            id: "path.bad".to_owned(),
            position: Position::Last,
            closed: None,
            anchors: vec![path_anchor(0.0, 0.0), path_anchor(10.0, 0.0)],
            subpaths: vec![OpPathSubpath {
                closed: None,
                anchors: vec![path_anchor(0.0, 0.0), path_anchor(10.0, 0.0)],
            }],
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_node_spec"),
        "expected tx.invalid_node_spec; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn add_path_rejects_empty_payload() {
    let doc = parse(ADD_BASE_DOC);
    let tx = Transaction {
        ops: vec![Op::AddPath {
            parent: "pg1".to_owned(),
            id: "path.empty".to_owned(),
            position: Position::Last,
            closed: None,
            anchors: Vec::new(),
            subpaths: Vec::new(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_node_spec"),
        "expected tx.invalid_node_spec; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn add_path_json_defaults_position_and_subpaths() {
    let tx = Transaction::from_json(
        r#"{"ops":[{"op":"add_path","parent":"pg1","id":"path.json","closed":false,"anchors":[{"x":0,"y":0},{"x":10,"y":0}]}]}"#,
    )
    .expect("transaction JSON should parse");

    assert_eq!(
        tx.ops,
        vec![Op::AddPath {
            parent: "pg1".to_owned(),
            id: "path.json".to_owned(),
            position: Position::Last,
            closed: Some(false),
            anchors: vec![path_anchor(0.0, 0.0), path_anchor(10.0, 0.0)],
            subpaths: Vec::new(),
        }]
    );
}

fn path_anchor(x: f64, y: f64) -> OpPathAnchor {
    OpPathAnchor {
        x,
        y,
        kind: None,
        in_x: None,
        in_y: None,
        out_x: None,
        out_y: None,
    }
}
