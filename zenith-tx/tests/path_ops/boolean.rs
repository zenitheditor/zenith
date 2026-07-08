use super::*;
use zenith_core::{Node, PathNode};
use zenith_tx::op::OpPathBooleanOperation;

const BOOLEAN_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#ff0000"
    token id="color.stroke" type="color" value="#0000ff"
    token id="size.stroke" type="dimension" value=(px)2
  }
  styles {
    style id="style.path" {
      fill (token)"color.fill"
    }
  }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="source" name="Source" closed=#true fill=(token)"color.fill" stroke=(token)"color.stroke" stroke-width=(token)"size.stroke" stroke-alignment="inside" stroke-linejoin="round" stroke-miter-limit=6 fill-rule="evenodd" opacity=0.75 visible=#false locked=#true style="style.path" {
        anchor x=(px)0 y=(px)0
        anchor x=(px)40 y=(px)0
        anchor x=(px)40 y=(px)40
        anchor x=(px)0 y=(px)40
      }
      path id="target" closed=#true {
        anchor x=(px)20 y=(px)-10
        anchor x=(px)60 y=(px)-10
        anchor x=(px)60 y=(px)30
        anchor x=(px)20 y=(px)30
      }
    }
  }
}"##;

fn path_boolean_tx(operation: OpPathBooleanOperation, new_id: &str) -> Transaction {
    Transaction {
        ops: vec![Op::PathBoolean {
            node: "source".to_owned(),
            target: "target".to_owned(),
            new_id: new_id.to_owned(),
            operation,
            tolerance: 0.1,
        }],
        permissions: Permissions::default(),
    }
}

fn assert_accepted_boolean(operation: OpPathBooleanOperation, new_id: &str) {
    let doc = parse(BOOLEAN_DOC);
    let result = run_transaction(&doc, &path_boolean_tx(operation, new_id))
        .expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec![new_id.to_owned()]);
    assert!(result.source_after.contains("path id=\"source\""));
    assert!(result.source_after.contains("path id=\"target\""));
    assert_sibling_order(&result.source_after, "source", new_id, "target");
    match operation {
        OpPathBooleanOperation::Union
        | OpPathBooleanOperation::Intersect
        | OpPathBooleanOperation::Subtract => {
            assert!(
                result
                    .source_after
                    .contains(&format!("path id=\"{new_id}\" closed=#true"))
            );
        }
        OpPathBooleanOperation::Exclude => {
            assert!(
                result
                    .source_after
                    .contains(&format!("path id=\"{new_id}\""))
            );
        }
    }
}

fn assert_sibling_order(source: &str, first: &str, second: &str, third: &str) {
    let first_index = source
        .find(&format!("path id=\"{first}\""))
        .expect("first path should exist");
    let second_index = source
        .find(&format!("path id=\"{second}\""))
        .expect("second path should exist");
    let third_index = source
        .find(&format!("path id=\"{third}\""))
        .expect("third path should exist");
    assert!(first_index < second_index && second_index < third_index);
}

fn count_occurrences(source: &str, needle: &str) -> usize {
    source.match_indices(needle).count()
}

fn output_path(source: &str, id: &str) -> PathNode {
    let doc = parse(source);
    for page in &doc.body.pages {
        for node in &page.children {
            if let Node::Path(path) = node
                && path.id == id
            {
                return path.clone();
            }
        }
    }
    panic!("output path should exist");
}

fn assert_rejected(doc_source: &str, op: Op, code: &str) {
    let doc = parse(doc_source);
    let tx = Transaction {
        ops: vec![op],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == code),
        "expected diagnostic {code}; got {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn path_boolean_union_creates_sibling_after_source() {
    assert_accepted_boolean(OpPathBooleanOperation::Union, "source.union");
}

#[test]
fn path_boolean_intersect_creates_sibling_after_source() {
    assert_accepted_boolean(OpPathBooleanOperation::Intersect, "source.intersect");
}

#[test]
fn path_boolean_subtract_creates_sibling_after_source() {
    assert_accepted_boolean(OpPathBooleanOperation::Subtract, "source.subtract");
}

#[test]
fn path_boolean_exclude_creates_sibling_after_source() {
    assert_accepted_boolean(OpPathBooleanOperation::Exclude, "source.exclude");
}

#[test]
fn path_boolean_preserves_source_paint_and_clears_authoring_metadata() {
    let doc = parse(BOOLEAN_DOC);
    let result = run_transaction(
        &doc,
        &path_boolean_tx(OpPathBooleanOperation::Intersect, "source.paint"),
    )
    .expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    let path_line = result
        .source_after
        .lines()
        .find(|line| line.contains("path id=\"source.paint\""))
        .expect("output path line should exist");
    assert!(path_line.contains("fill=(token)\"color.fill\""));
    assert!(path_line.contains("stroke=(token)\"color.stroke\""));
    assert!(path_line.contains("stroke-width=(token)\"size.stroke\""));
    assert!(path_line.contains("stroke-alignment=\"inside\""));
    assert!(path_line.contains("stroke-linejoin=\"round\""));
    assert!(path_line.contains("stroke-miter-limit=6"));
    assert!(path_line.contains("fill-rule=\"evenodd\""));
    assert!(path_line.contains("opacity=0.75"));
    assert!(path_line.contains("visible=#false"));
    assert!(path_line.contains("style=\"style.path\""));
    assert!(!path_line.contains("name="));
    assert!(!path_line.contains("locked="));
}

#[test]
fn path_boolean_rejects_open_path() {
    assert_rejected(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="source" {
        anchor x=(px)0 y=(px)0
        anchor x=(px)40 y=(px)0
      }
      path id="target" closed=#true {
        anchor x=(px)0 y=(px)0
        anchor x=(px)10 y=(px)0
        anchor x=(px)0 y=(px)10
      }
    }
  }
}"##,
        Op::PathBoolean {
            node: "source".to_owned(),
            target: "target".to_owned(),
            new_id: "out".to_owned(),
            operation: OpPathBooleanOperation::Union,
            tolerance: 0.1,
        },
        "tx.invalid_geometry",
    );
}

#[test]
fn path_boolean_rejects_compound_path() {
    assert_rejected(
        COMPOUND_PATH_DOC,
        Op::PathBoolean {
            node: "compound".to_owned(),
            target: "compound".to_owned(),
            new_id: "out".to_owned(),
            operation: OpPathBooleanOperation::Union,
            tolerance: 0.1,
        },
        "tx.invalid_geometry",
    );
}

#[test]
fn path_boolean_rejects_rotated_path() {
    assert_rejected(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="source" closed=#true rotate=(deg)10 {
        anchor x=(px)0 y=(px)0
        anchor x=(px)40 y=(px)0
        anchor x=(px)40 y=(px)40
      }
      path id="target" closed=#true {
        anchor x=(px)10 y=(px)10
        anchor x=(px)50 y=(px)10
        anchor x=(px)10 y=(px)50
      }
    }
  }
}"##,
        Op::PathBoolean {
            node: "source".to_owned(),
            target: "target".to_owned(),
            new_id: "out".to_owned(),
            operation: OpPathBooleanOperation::Union,
            tolerance: 0.1,
        },
        "tx.invalid_geometry",
    );
}

#[test]
fn path_boolean_rejects_non_path_target() {
    assert_rejected(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="source" closed=#true {
        anchor x=(px)0 y=(px)0
        anchor x=(px)40 y=(px)0
        anchor x=(px)40 y=(px)40
      }
      rect id="target" x=(px)0 y=(px)0 w=(px)10 h=(px)10
    }
  }
}"##,
        Op::PathBoolean {
            node: "source".to_owned(),
            target: "target".to_owned(),
            new_id: "out".to_owned(),
            operation: OpPathBooleanOperation::Union,
            tolerance: 0.1,
        },
        "tx.unsupported_property",
    );
}

#[test]
fn path_boolean_rejects_invalid_tolerance() {
    assert_rejected(
        BOOLEAN_DOC,
        Op::PathBoolean {
            node: "source".to_owned(),
            target: "target".to_owned(),
            new_id: "out".to_owned(),
            operation: OpPathBooleanOperation::Union,
            tolerance: 0.0,
        },
        "tx.invalid_geometry",
    );
}

#[test]
fn path_boolean_rejects_empty_result() {
    assert_rejected(
        BOOLEAN_DOC,
        Op::PathBoolean {
            node: "source".to_owned(),
            target: "source".to_owned(),
            new_id: "out".to_owned(),
            operation: OpPathBooleanOperation::Subtract,
            tolerance: 0.1,
        },
        "tx.invalid_geometry",
    );
}

#[test]
fn path_boolean_disjoint_union_creates_compound_result() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="source" closed=#true {
        anchor x=(px)0 y=(px)0
        anchor x=(px)10 y=(px)0
        anchor x=(px)10 y=(px)10
        anchor x=(px)0 y=(px)10
      }
      path id="target" closed=#true {
        anchor x=(px)30 y=(px)0
        anchor x=(px)40 y=(px)0
        anchor x=(px)40 y=(px)10
        anchor x=(px)30 y=(px)10
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::PathBoolean {
            node: "source".to_owned(),
            target: "target".to_owned(),
            new_id: "out".to_owned(),
            operation: OpPathBooleanOperation::Union,
            tolerance: 0.1,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["out".to_owned()]);
    let path_line = result
        .source_after
        .lines()
        .find(|line| line.contains("path id=\"out\""))
        .expect("output path line should exist");
    assert!(path_line.contains("fill-rule=\"evenodd\""));
    assert!(!path_line.contains("closed=#true"));
    assert_eq!(
        count_occurrences(&result.source_after, "subpath closed=#true"),
        2
    );
    let out = output_path(&result.source_after, "out");
    assert_eq!(out.closed, None);
    assert_eq!(out.fill_rule.as_deref(), Some("evenodd"));
    assert_eq!(out.fill, None);
    assert_eq!(out.stroke, None);
    assert!(out.anchors.is_empty());
    assert_eq!(out.subpaths.len(), 2);
    for subpath in &out.subpaths {
        assert_eq!(subpath.closed, Some(true));
        assert!(subpath.anchors.iter().all(|anchor| anchor.kind.is_none()
            && anchor.in_x.is_none()
            && anchor.in_y.is_none()
            && anchor.out_x.is_none()
            && anchor.out_y.is_none()));
    }
}

#[test]
fn path_boolean_exclude_overlapping_rectangles_creates_compound_result() {
    let doc = parse(BOOLEAN_DOC);
    let tx = Transaction {
        ops: vec![Op::PathBoolean {
            node: "source".to_owned(),
            target: "target".to_owned(),
            new_id: "out".to_owned(),
            operation: OpPathBooleanOperation::Exclude,
            tolerance: 0.1,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["out".to_owned()]);
    assert!(result.source_after.contains("path id=\"out\""));
    assert_eq!(
        count_occurrences(&result.source_after, "subpath closed=#true"),
        2
    );
    let out = output_path(&result.source_after, "out");
    assert_eq!(out.closed, None);
    assert_eq!(out.fill_rule.as_deref(), Some("evenodd"));
    assert!(out.fill.is_some());
    assert!(out.stroke.is_some());
    assert!(out.stroke_width.is_some());
    assert_eq!(out.stroke_alignment.as_deref(), Some("inside"));
    assert_eq!(out.stroke_linejoin.as_deref(), Some("round"));
    assert_eq!(out.stroke_miter_limit, Some(6.0));
    assert_eq!(out.opacity, Some(0.75));
    assert_eq!(out.visible, Some(false));
    assert_eq!(out.style.as_deref(), Some("style.path"));
    assert_eq!(out.name, None);
    assert_eq!(out.locked, None);
    assert_eq!(out.rotate, None);
    assert_eq!(out.subpaths.len(), 2);
}

#[test]
fn path_boolean_compound_result_uses_evenodd_fill_rule() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="source" closed=#true fill-rule="nonzero" {
        anchor x=(px)0 y=(px)0
        anchor x=(px)10 y=(px)0
        anchor x=(px)10 y=(px)10
        anchor x=(px)0 y=(px)10
      }
      path id="target" closed=#true {
        anchor x=(px)30 y=(px)0
        anchor x=(px)40 y=(px)0
        anchor x=(px)40 y=(px)10
        anchor x=(px)30 y=(px)10
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::PathBoolean {
            node: "source".to_owned(),
            target: "target".to_owned(),
            new_id: "out".to_owned(),
            operation: OpPathBooleanOperation::Union,
            tolerance: 0.1,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    let path_line = result
        .source_after
        .lines()
        .find(|line| line.contains("path id=\"out\""))
        .expect("output path line should exist");
    assert!(path_line.contains("fill-rule=\"evenodd\""));
    let out = output_path(&result.source_after, "out");
    assert_eq!(out.closed, None);
    assert_eq!(out.fill_rule.as_deref(), Some("evenodd"));
    assert!(out.anchors.is_empty());
    assert_eq!(out.name, None);
    assert_eq!(out.locked, None);
    assert_eq!(out.rotate, None);
}

#[test]
fn path_boolean_duplicate_new_id_rejected_by_post_validation() {
    assert_rejected(
        BOOLEAN_DOC,
        Op::PathBoolean {
            node: "source".to_owned(),
            target: "target".to_owned(),
            new_id: "target".to_owned(),
            operation: OpPathBooleanOperation::Intersect,
            tolerance: 0.1,
        },
        "id.duplicate",
    );
}
