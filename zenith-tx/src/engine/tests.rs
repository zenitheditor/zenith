//! Unit tests for the transaction engine.
//!
//! Moved verbatim from the original `engine.rs`; explicit `use` lines below
//! replace the parent module's private `use` imports that `super::*` no
//! longer re-exports after the split into submodules.

use super::run_transaction;
use crate::op::{Op, Permissions, Position, Transaction};
use crate::result::TxStatus;
use zenith_core::{Document, KdlAdapter, KdlSource};
/// Minimal valid document with a `text` node (align `start`) and a `rect`.
fn parse(src: &str) -> Document {
    KdlAdapter
        .parse(src.as_bytes())
        .expect("test doc must parse")
}

// ── Test documents ────────────────────────────────────────────────────────

const TEXT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      text id="label" x=(px)10 y=(px)10 w=(px)200 h=(px)40 align="start" {
        span "Hello"
      }
    }
  }
}"##;

const TWO_RECT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="a" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      rect id="b" x=(px)0 y=(px)0 w=(px)100 h=(px)100
    }
  }
}"##;

const MIXED_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="box1" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      text id="lbl" x=(px)10 y=(px)10 w=(px)200 h=(px)40 {
        span "Hi"
      }
    }
  }
}"##;

const ELLIPSE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      ellipse id="dot" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      text id="lbl" x=(px)10 y=(px)10 w=(px)200 h=(px)40 {
        span "Hi"
      }
    }
  }
}"##;

const IMAGE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  assets {
    asset id="asset.pic" kind="image" src="pic.png"
  }
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      image id="pic" asset="asset.pic" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      text id="lbl" x=(px)10 y=(px)10 w=(px)200 h=(px)40 {
        span "Hi"
      }
    }
  }
}"##;

// ── 1. SetTextAlign: accepted, affected ids, source diff ──────────────────

#[test]
fn set_text_align_accepted() {
    let doc = parse(TEXT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextAlign {
            node: "label".to_owned(),
            align: "center".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["label".to_owned()]);
    assert!(
        result.source_after.contains("center"),
        "source_after should contain align=\"center\""
    );
    assert!(
        !result.source_before.contains("center"),
        "source_before should not contain center"
    );
    assert_ne!(result.source_before, result.source_after);
}

// ── 2. from_json round-trip ───────────────────────────────────────────────

#[test]
fn from_json_round_trip() {
    let json = r#"{"ops":[{"op":"set_text_align","node":"label","align":"center"},{"op":"move_forward","node":"accent"}]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![
                Op::SetTextAlign {
                    node: "label".to_owned(),
                    align: "center".to_owned(),
                },
                Op::MoveForward {
                    node: "accent".to_owned()
                },
            ],
            permissions: Permissions::default(),
        }
    );
}

// ── 3. MoveForward: a moves after b ──────────────────────────────────────

#[test]
fn move_forward_reorders() {
    let doc = parse(TWO_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::MoveForward {
            node: "a".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["a".to_owned()]);

    // In source_after, "b" should appear before "a" (a is now last).
    let pos_a = result
        .source_after
        .find("id=\"a\"")
        .expect("a in source_after");
    let pos_b = result
        .source_after
        .find("id=\"b\"")
        .expect("b in source_after");
    assert!(pos_b < pos_a, "b should appear before a in source_after");

    // source_before has a before b.
    let pb_a = result
        .source_before
        .find("id=\"a\"")
        .expect("a in source_before");
    let pb_b = result
        .source_before
        .find("id=\"b\"")
        .expect("b in source_before");
    assert!(pb_a < pb_b, "a should appear before b in source_before");
}

// ── 4. Unknown node id → Rejected ────────────────────────────────────────

#[test]
fn unknown_node_rejected() {
    let doc = parse(TEXT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextAlign {
            node: "does_not_exist".to_owned(),
            align: "center".to_owned(),
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
        "expected tx.unknown_node diagnostic"
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── 5. SetTextAlign on a rect → wrong_node_type, Rejected ────────────────

#[test]
fn set_text_align_wrong_node_type() {
    let doc = parse(MIXED_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextAlign {
            node: "box1".to_owned(),
            align: "center".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.wrong_node_type"),
        "expected tx.wrong_node_type diagnostic"
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── 5b. SetTextAlign on an ellipse → wrong_node_type, Rejected ───────────

#[test]
fn set_text_align_on_ellipse_wrong_node_type() {
    let doc = parse(ELLIPSE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextAlign {
            node: "dot".to_owned(),
            align: "center".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.wrong_node_type" && d.message.contains("ellipse")),
        "expected tx.wrong_node_type diagnostic naming the ellipse kind"
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── 5c. SetTextAlign on an image → wrong_node_type, Rejected ─────────────

#[test]
fn set_text_align_on_image_wrong_node_type() {
    let doc = parse(IMAGE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextAlign {
            node: "pic".to_owned(),
            align: "center".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.wrong_node_type" && d.message.contains("image")),
        "expected tx.wrong_node_type diagnostic naming the image kind"
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── SetTextAlign: recursion into group children ───────────────────────────

const GROUP_TEXT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Nest"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      group id="grp1" {
        text id="nested.label" x=(px)10 y=(px)10 w=(px)200 h=(px)40 align="start" {
          span "Hello"
        }
      }
    }
  }
}"##;

#[test]
fn tx_set_text_align_targets_nested_text() {
    // A text node nested inside a group should now be reachable via
    // recursive descent; the tx engine is no longer limited to top-level
    // page children.
    let doc = parse(GROUP_TEXT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextAlign {
            node: "nested.label".to_owned(),
            align: "center".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["nested.label".to_owned()]);
    assert!(
        result.source_after.contains("center"),
        "source_after should contain align=\"center\""
    );
    assert!(!result.source_before.contains("center"));
    assert_ne!(result.source_before, result.source_after);
}

#[test]
fn tx_set_text_align_on_group_itself_wrong_type() {
    // Targeting the group's own id with SetTextAlign must yield
    // tx.wrong_node_type mentioning "group".
    let doc = parse(GROUP_TEXT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextAlign {
            node: "grp1".to_owned(),
            align: "center".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.wrong_node_type" && d.message.contains("group")),
        "expected tx.wrong_node_type diagnostic naming \"group\"; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── MoveForward: reorder among group siblings ─────────────────────────────

const GROUP_TWO_RECT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      group id="grp1" {
        rect id="a" x=(px)0 y=(px)0 w=(px)100 h=(px)100
        rect id="b" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      }
    }
  }
}"##;

#[test]
fn tx_move_forward_reorders_nested_child() {
    // Two rects (a then b) nested inside a group. MoveForward on "a"
    // should reorder them so b appears before a in source_after.
    let doc = parse(GROUP_TWO_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::MoveForward {
            node: "a".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["a".to_owned()]);

    // In source_after, "b" should appear before "a".
    let pos_a = result
        .source_after
        .find("id=\"a\"")
        .expect("a in source_after");
    let pos_b = result
        .source_after
        .find("id=\"b\"")
        .expect("b in source_after");
    assert!(pos_b < pos_a, "b should appear before a in source_after");

    // source_before has a before b.
    let pb_a = result
        .source_before
        .find("id=\"a\"")
        .expect("a in source_before");
    let pb_b = result
        .source_before
        .find("id=\"b\"")
        .expect("b in source_before");
    assert!(pb_a < pb_b, "a should appear before b in source_before");
}

// ── SetFill / SetVisible / SetLocked test documents ───────────────────────

/// Rect with fill token A; token B also declared so post-validate passes.
const FILL_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.a" type="color" value="#ff0000"
    token id="color.b" type="color" value="#0000ff"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="r1" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.a"
    }
  }
}"##;

/// Line node (no fill field).
const LINE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.a" type="color" value="#ff0000"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      line id="ln1" x1=(px)0 y1=(px)0 x2=(px)100 y2=(px)100 stroke=(token)"color.a"
    }
  }
}"##;

/// Page with one code node (fill via a declared token).
const CODE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.a" type="color" value="#ff0000"
    token id="color.b" type="color" value="#0000ff"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      code id="snip" x=(px)0 y=(px)0 w=(px)200 h=(px)100 fill=(token)"color.a" {
        content "fn main() {}"
      }
    }
  }
}"##;

/// Rect inside a group.
const NESTED_RECT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      group id="grp1" {
        rect id="inner" x=(px)0 y=(px)0 w=(px)50 h=(px)50
      }
    }
  }
}"##;

// ── SetFill tests ─────────────────────────────────────────────────────────

#[test]
fn set_fill_recolors_rect() {
    let doc = parse(FILL_DOC);
    let tx = Transaction {
        ops: vec![Op::SetFill {
            node: "r1".to_owned(),
            fill: "color.b".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["r1".to_owned()]);
    // TokenRef("color.b") serialises as fill=(token)"color.b"
    assert!(
        result.source_after.contains("(token)\"color.b\""),
        "source_after must reference color.b; got:\n{}",
        result.source_after
    );
    assert!(
        !result.source_after.contains("(token)\"color.a\""),
        "old token must not appear in source_after"
    );
    assert_ne!(result.source_before, result.source_after);
}

#[test]
fn set_fill_unsupported_on_line() {
    let doc = parse(LINE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetFill {
            node: "ln1".to_owned(),
            fill: "color.a".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unsupported_property" && d.message.contains("line")),
        "expected tx.unsupported_property mentioning \"line\"; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn set_fill_unknown_token_rejected() {
    // color.nope is not declared → post-validate emits token.unknown_reference → Rejected
    let doc = parse(FILL_DOC);
    let tx = Transaction {
        ops: vec![Op::SetFill {
            node: "r1".to_owned(),
            fill: "color.nope".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "token.unknown_reference"),
        "expected token.unknown_reference diagnostic; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── SetStroke / SetStrokeWidth tests ──────────────────────────────────────

/// Rect, line, polygon carrying valid color + dimension tokens.
const STROKE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.rule" type="color" value="#334155"
    token id="size.stroke" type="dimension" value=(px)2
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="r1" x=(px)0 y=(px)0 w=(px)100 h=(px)100 stroke=(token)"color.rule" stroke-width=(token)"size.stroke"
      line id="ln1" x1=(px)0 y1=(px)0 x2=(px)100 y2=(px)100 stroke=(token)"color.rule"
      ellipse id="dot" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.rule"
      text id="lbl" x=(px)10 y=(px)10 w=(px)200 h=(px)40 {
        span "Hi"
      }
      polygon id="poly1" stroke=(token)"color.rule" stroke-width=(token)"size.stroke" {
        point x=(px)10 y=(px)10
        point x=(px)90 y=(px)10
        point x=(px)50 y=(px)90
      }
    }
  }
}"##;

#[test]
fn set_stroke_recolors_rect() {
    let doc = parse(STROKE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetStroke {
            node: "r1".to_owned(),
            stroke: "color.rule".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["r1".to_owned()]);
    assert!(
        result.source_after.contains("stroke=(token)\"color.rule\""),
        "source_after must reference color.rule as stroke; got:\n{}",
        result.source_after
    );
}

#[test]
fn set_stroke_unknown_token_rejected() {
    // color.nope is not declared → post-validate emits token.unknown_reference.
    let doc = parse(STROKE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetStroke {
            node: "r1".to_owned(),
            stroke: "color.nope".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "token.unknown_reference"),
        "expected token.unknown_reference diagnostic; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn set_stroke_accepted_on_ellipse() {
    // Ellipse now supports stroke — set_stroke must be Accepted.
    let doc = parse(STROKE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetStroke {
            node: "dot".to_owned(),
            stroke: "color.rule".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "set_stroke on an ellipse must be Accepted; got: {:?}",
        result.diagnostics
    );
    assert!(
        result.source_after.contains("stroke=(token)\"color.rule\""),
        "formatted source must contain the new stroke property; got:\n{}",
        result.source_after
    );
}

#[test]
fn set_stroke_unknown_node_rejected() {
    let doc = parse(STROKE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetStroke {
            node: "nope".to_owned(),
            stroke: "color.rule".to_owned(),
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
        "expected tx.unknown_node diagnostic; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn set_stroke_width_on_polygon() {
    let doc = parse(STROKE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetStrokeWidth {
            node: "poly1".to_owned(),
            stroke_width: "size.stroke".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["poly1".to_owned()]);
    assert!(
        result
            .source_after
            .contains("stroke-width=(token)\"size.stroke\""),
        "source_after must reference size.stroke as stroke-width; got:\n{}",
        result.source_after
    );
}

#[test]
fn set_stroke_width_unsupported_on_text() {
    let doc = parse(STROKE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetStrokeWidth {
            node: "lbl".to_owned(),
            stroke_width: "size.stroke".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unsupported_property"
                && d.message
                    .contains("set_stroke_width is not supported on a text node")),
        "expected tx.unsupported_property naming text; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── SetVisible tests ──────────────────────────────────────────────────────

#[test]
fn set_visible_hides_node() {
    let doc = parse(TWO_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetVisible {
            node: "a".to_owned(),
            visible: false,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["a".to_owned()]);
    assert!(
        result.source_after.contains("visible=#false"),
        "source_after must contain visible=#false; got:\n{}",
        result.source_after
    );
    assert_ne!(result.source_before, result.source_after);
}

#[test]
fn set_visible_on_nested_node() {
    let doc = parse(NESTED_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetVisible {
            node: "inner".to_owned(),
            visible: false,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["inner".to_owned()]);
    assert!(
        result.source_after.contains("visible=#false"),
        "source_after must contain visible=#false for nested node; got:\n{}",
        result.source_after
    );
}

// ── SetLocked tests ───────────────────────────────────────────────────────

#[test]
fn set_locked_sets_lock() {
    let doc = parse(TWO_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetLocked {
            node: "b".to_owned(),
            locked: true,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["b".to_owned()]);
    assert!(
        result.source_after.contains("locked=#true"),
        "source_after must contain locked=#true; got:\n{}",
        result.source_after
    );
    assert_ne!(result.source_before, result.source_after);
}

// ── Unknown node targeting ────────────────────────────────────────────────

// `UnknownNode` has no `id` field, so `node_id_of` returns `None` for it.
// `subtree_contains` will never match an unknown node by id, and
// `find_node_any_mut` returns `None` → tx.unknown_node.
// We verify this by targeting a non-existent id that would match an unknown
// node if it had an id; since it doesn't, we just get tx.unknown_node.
#[test]
fn set_visible_on_nonexistent_id_is_unknown_node() {
    // Using TEXT_DOC — there is no node with id "does_not_exist".
    // The important thing: we get tx.unknown_node, not a panic or
    // tx.unsupported_property.
    let doc = parse(TEXT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetVisible {
            node: "does_not_exist".to_owned(),
            visible: false,
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
}

// ── from_json round-trip: new op variants ─────────────────────────────────

#[test]
fn from_json_new_ops_round_trip() {
    let json = r#"{"ops":[
            {"op":"set_fill","node":"r","fill":"c"},
            {"op":"set_visible","node":"r","visible":false},
            {"op":"set_locked","node":"r","locked":true},
            {"op":"set_stroke","node":"r","stroke":"s"},
            {"op":"set_stroke_width","node":"r","stroke_width":"sw"},
            {"op":"add_node","parent":"pg1","position":{"at":"after","id":"r"},"source":"rect id=\"r2\""},
            {"op":"remove_node","node":"r"}
        ]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![
                Op::SetFill {
                    node: "r".to_owned(),
                    fill: "c".to_owned(),
                },
                Op::SetVisible {
                    node: "r".to_owned(),
                    visible: false,
                },
                Op::SetLocked {
                    node: "r".to_owned(),
                    locked: true,
                },
                Op::SetStroke {
                    node: "r".to_owned(),
                    stroke: "s".to_owned(),
                },
                Op::SetStrokeWidth {
                    node: "r".to_owned(),
                    stroke_width: "sw".to_owned(),
                },
                Op::AddNode {
                    parent: "pg1".to_owned(),
                    position: Position::After { id: "r".to_owned() },
                    source: "rect id=\"r2\"".to_owned(),
                },
                Op::RemoveNode {
                    node: "r".to_owned(),
                },
            ],
            permissions: Permissions::default(),
        }
    );
}

#[test]
fn from_json_add_node_position_defaults_to_last() {
    // `position` omitted → serde default → Position::Last.
    let json = r#"{"ops":[{"op":"add_node","parent":"pg1","source":"rect id=\"r2\""}]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![Op::AddNode {
                parent: "pg1".to_owned(),
                position: Position::Last,
                source: "rect id=\"r2\"".to_owned(),
            }],
            permissions: Permissions::default(),
        }
    );
}

// ── SetGeometry / SetPoints test documents ────────────────────────────────

/// Rect at origin, 100×100. No tokens needed for geometry ops.
const RECT_GEOM_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="rect" x=(px)0 y=(px)0 w=(px)100 h=(px)100
    }
  }
}"##;

/// Polygon with exactly 3 points and a fill token (to keep post-validate happy).
const POLY_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#ff0000"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      polygon id="poly" fill=(token)"color.fill" {
        point x=(px)0 y=(px)0
        point x=(px)100 y=(px)0
        point x=(px)50 y=(px)80
      }
    }
  }
}"##;

// ── SetGeometry tests ─────────────────────────────────────────────────────

#[test]
fn set_geometry_moves_rect() {
    let doc = parse(RECT_GEOM_DOC);
    let tx = Transaction {
        ops: vec![Op::SetGeometry {
            node: "rect".to_owned(),
            x: Some(50.0),
            y: None,
            w: Some(200.0),
            h: None,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["rect".to_owned()]);

    // Changed fields appear in source_after.
    assert!(
        result.source_after.contains("x=(px)50"),
        "source_after must contain x=(px)50; got:\n{}",
        result.source_after
    );
    assert!(
        result.source_after.contains("w=(px)200"),
        "source_after must contain w=(px)200; got:\n{}",
        result.source_after
    );
    // Untouched fields stay at their original values.
    assert!(
        result.source_after.contains("y=(px)0"),
        "source_after must retain y=(px)0; got:\n{}",
        result.source_after
    );
    assert!(
        result.source_after.contains("h=(px)100"),
        "source_after must retain h=(px)100; got:\n{}",
        result.source_after
    );
    assert_ne!(result.source_before, result.source_after);
}

#[test]
fn set_geometry_unsupported_on_line() {
    let doc = parse(LINE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetGeometry {
            node: "ln1".to_owned(),
            x: Some(10.0),
            y: None,
            w: None,
            h: None,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unsupported_property" && d.message.contains("line")),
        "expected tx.unsupported_property mentioning \"line\"; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn set_geometry_no_fields_is_noop() {
    let doc = parse(RECT_GEOM_DOC);
    let tx = Transaction {
        ops: vec![Op::SetGeometry {
            node: "rect".to_owned(),
            x: None,
            y: None,
            w: None,
            h: None,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    // All-None must produce Accepted (advisory is not an error/warning) with
    // no affected nodes and identical source.
    assert_eq!(result.status, TxStatus::Accepted);
    assert!(
        result.affected_node_ids.is_empty(),
        "affected must be empty for a noop; got: {:?}",
        result.affected_node_ids
    );
    assert!(
        result.diagnostics.iter().any(|d| d.code == "tx.noop"),
        "expected tx.noop advisory; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── Code node tx tests ────────────────────────────────────────────────────

#[test]
fn set_visible_on_code_accepted() {
    let doc = parse(CODE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetVisible {
            node: "snip".to_owned(),
            visible: false,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["snip".to_owned()]);
    assert!(
        result.source_after.contains("visible=#false"),
        "source_after must contain visible=#false; got:\n{}",
        result.source_after
    );
    // Content blob must survive the edit untouched.
    assert!(result.source_after.contains("content \"fn main() {}\""));
}

#[test]
fn set_fill_on_code_accepted() {
    let doc = parse(CODE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetFill {
            node: "snip".to_owned(),
            fill: "color.b".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["snip".to_owned()]);
    assert!(
        result.source_after.contains("(token)\"color.b\""),
        "source_after must reference color.b; got:\n{}",
        result.source_after
    );
}

#[test]
fn set_geometry_supported_on_code() {
    let doc = parse(CODE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetGeometry {
            node: "snip".to_owned(),
            x: Some(10.0),
            y: None,
            w: None,
            h: None,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["snip".to_owned()]);
    assert!(
        result.source_after.contains("x=(px)10"),
        "source_after must contain x=(px)10; got:\n{}",
        result.source_after
    );
    assert_ne!(result.source_after, result.source_before);
}

#[test]
fn set_geometry_supported_on_text() {
    let doc = parse(TEXT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetGeometry {
            node: "label".to_owned(),
            x: Some(-200.0),
            y: None,
            w: None,
            h: None,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["label".to_owned()]);
    assert!(
        result.source_after.contains("x=(px)-200"),
        "source_after must contain x=(px)-200; got:\n{}",
        result.source_after
    );
    assert_ne!(result.source_after, result.source_before);
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

// ── SetPoints tests ───────────────────────────────────────────────────────

#[test]
fn set_points_replaces_polygon() {
    let doc = parse(POLY_DOC);
    // Replace the 3 original points with 3 different ones.
    let tx = Transaction {
        ops: vec![Op::SetPoints {
            node: "poly".to_owned(),
            points: vec![
                crate::op::OpPoint { x: 10.0, y: 20.0 },
                crate::op::OpPoint { x: 90.0, y: 20.0 },
                crate::op::OpPoint { x: 50.0, y: 70.0 },
            ],
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["poly".to_owned()]);

    // New coordinates appear in source_after.
    assert!(
        result.source_after.contains("x=(px)10"),
        "source_after must contain x=(px)10; got:\n{}",
        result.source_after
    );
    assert!(
        result.source_after.contains("y=(px)20"),
        "source_after must contain y=(px)20; got:\n{}",
        result.source_after
    );
    // Old distinctive coordinate (x=50, y=80) from original must be gone.
    assert!(
        !result.source_after.contains("y=(px)80"),
        "old y=(px)80 must not appear in source_after"
    );
    assert_ne!(result.source_before, result.source_after);
}

#[test]
fn set_points_too_few_rejected() {
    // Start from a valid 3-point polygon; replace with only 2 points →
    // post-validation rejects with shape.insufficient_points.
    let doc = parse(POLY_DOC);
    let tx = Transaction {
        ops: vec![Op::SetPoints {
            node: "poly".to_owned(),
            points: vec![
                crate::op::OpPoint { x: 0.0, y: 0.0 },
                crate::op::OpPoint { x: 100.0, y: 0.0 },
            ],
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "shape.insufficient_points"),
        "expected shape.insufficient_points diagnostic; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn set_points_unsupported_on_rect() {
    let doc = parse(RECT_GEOM_DOC);
    let tx = Transaction {
        ops: vec![Op::SetPoints {
            node: "rect".to_owned(),
            points: vec![
                crate::op::OpPoint { x: 0.0, y: 0.0 },
                crate::op::OpPoint { x: 100.0, y: 0.0 },
                crate::op::OpPoint { x: 50.0, y: 80.0 },
            ],
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unsupported_property" && d.message.contains("rect")),
        "expected tx.unsupported_property mentioning \"rect\"; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── JSON round-trip: reshape ops ─────────────────────────────────────────

#[test]
fn from_json_reshape_ops_round_trip() {
    use crate::op::OpPoint;

    let json_geo = r#"{"ops":[{"op":"set_geometry","node":"r","x":10.0,"w":200.0}]}"#;
    let tx_geo = Transaction::from_json(json_geo).expect("parse set_geometry JSON");
    assert_eq!(
        tx_geo,
        Transaction {
            ops: vec![Op::SetGeometry {
                node: "r".to_owned(),
                x: Some(10.0),
                y: None,
                w: Some(200.0),
                h: None,
            }],
            permissions: Permissions::default(),
        }
    );

    let json_pts = r#"{"ops":[{"op":"set_points","node":"p","points":[{"x":0.0,"y":0.0},{"x":1.0,"y":1.0}]}]}"#;
    let tx_pts = Transaction::from_json(json_pts).expect("parse set_points JSON");
    assert_eq!(
        tx_pts,
        Transaction {
            ops: vec![Op::SetPoints {
                node: "p".to_owned(),
                points: vec![OpPoint { x: 0.0, y: 0.0 }, OpPoint { x: 1.0, y: 1.0 },],
            }],
            permissions: Permissions::default(),
        }
    );
}

// ── 6. Invalid align value → tx.invalid_value, Rejected ──────────────────

#[test]
fn invalid_align_value_rejected() {
    let doc = parse(TEXT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextAlign {
            node: "label".to_owned(),
            align: "middle".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_value"),
        "expected tx.invalid_value diagnostic"
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── MoveBackward / MoveToFront / MoveToBack test documents ───────────────

/// Three rects a (index 0, bottom), b (index 1), c (index 2, top).
const THREE_RECT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="a" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      rect id="b" x=(px)10 y=(px)0 w=(px)100 h=(px)100
      rect id="c" x=(px)20 y=(px)0 w=(px)100 h=(px)100
    }
  }
}"##;

/// Group containing two rects: x (bottom) then y (top).
const GROUP_TWO_RECT_BACKWARD_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      group id="grp1" {
        rect id="x" x=(px)0 y=(px)0 w=(px)100 h=(px)100
        rect id="y" x=(px)10 y=(px)0 w=(px)100 h=(px)100
      }
    }
  }
}"##;

// ── MoveBackward tests ────────────────────────────────────────────────────

#[test]
fn move_backward_reorders() {
    // Doc: a (bottom) then b (top). MoveBackward on b → b moves before a.
    let doc = parse(TWO_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::MoveBackward {
            node: "b".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["b".to_owned()]);

    // In source_after, b should appear before a.
    let pos_a = result
        .source_after
        .find("id=\"a\"")
        .expect("a in source_after");
    let pos_b = result
        .source_after
        .find("id=\"b\"")
        .expect("b in source_after");
    assert!(pos_b < pos_a, "b should appear before a in source_after");
}

#[test]
fn move_backward_already_at_back_noop() {
    // Doc: a (bottom) then b. MoveBackward on "a" → already at back → noop advisory.
    let doc = parse(TWO_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::MoveBackward {
            node: "a".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert!(
        result.affected_node_ids.is_empty(),
        "affected must be empty for noop; got: {:?}",
        result.affected_node_ids
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.noop" && d.message.contains("back")),
        "expected tx.noop advisory mentioning \"back\"; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn move_backward_nested_child() {
    // Group with x (bottom) then y (top). MoveBackward on y → recursion into
    // group, y swaps with x.
    let doc = parse(GROUP_TWO_RECT_BACKWARD_DOC);
    let tx = Transaction {
        ops: vec![Op::MoveBackward {
            node: "y".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["y".to_owned()]);

    // In source_after, y should appear before x.
    let pos_x = result
        .source_after
        .find("id=\"x\"")
        .expect("x in source_after");
    let pos_y = result
        .source_after
        .find("id=\"y\"")
        .expect("y in source_after");
    assert!(pos_y < pos_x, "y should appear before x in source_after");
}

// ── MoveToFront tests ─────────────────────────────────────────────────────

#[test]
fn move_to_front_moves_to_top() {
    // THREE_RECT_DOC: a (0), b (1), c (2). MoveToFront on "a" → order becomes b, c, a.
    let doc = parse(THREE_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::MoveToFront {
            node: "a".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["a".to_owned()]);

    // In source_after: b appears before c, c appears before a.
    let pos_a = result
        .source_after
        .find("id=\"a\"")
        .expect("a in source_after");
    let pos_b = result
        .source_after
        .find("id=\"b\"")
        .expect("b in source_after");
    let pos_c = result
        .source_after
        .find("id=\"c\"")
        .expect("c in source_after");
    assert!(pos_b < pos_c, "b should appear before c in source_after");
    assert!(pos_c < pos_a, "c should appear before a in source_after");
}

#[test]
fn move_to_front_already_front_noop() {
    // THREE_RECT_DOC: c is already the last (topmost). MoveToFront on "c" → noop.
    let doc = parse(THREE_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::MoveToFront {
            node: "c".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert!(
        result.affected_node_ids.is_empty(),
        "affected must be empty for noop; got: {:?}",
        result.affected_node_ids
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.noop" && d.message.contains("front")),
        "expected tx.noop advisory mentioning \"front\"; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── MoveToBack tests ──────────────────────────────────────────────────────

#[test]
fn move_to_back_moves_to_bottom() {
    // THREE_RECT_DOC: a (0), b (1), c (2). MoveToBack on "c" → order becomes c, a, b.
    let doc = parse(THREE_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::MoveToBack {
            node: "c".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["c".to_owned()]);

    // In source_after: c appears before a, a appears before b.
    let pos_a = result
        .source_after
        .find("id=\"a\"")
        .expect("a in source_after");
    let pos_b = result
        .source_after
        .find("id=\"b\"")
        .expect("b in source_after");
    let pos_c = result
        .source_after
        .find("id=\"c\"")
        .expect("c in source_after");
    assert!(pos_c < pos_a, "c should appear before a in source_after");
    assert!(pos_a < pos_b, "a should appear before b in source_after");
}

// ── from_json round-trip: reorder ops ─────────────────────────────────────

#[test]
fn from_json_reorder_ops_round_trip() {
    let json = r#"{"ops":[
            {"op":"move_backward","node":"x"},
            {"op":"move_to_front","node":"x"},
            {"op":"move_to_back","node":"x"}
        ]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![
                Op::MoveBackward {
                    node: "x".to_owned()
                },
                Op::MoveToFront {
                    node: "x".to_owned()
                },
                Op::MoveToBack {
                    node: "x".to_owned()
                },
            ],
            permissions: Permissions::default(),
        }
    );
}

// ── AddNode / RemoveNode test documents ───────────────────────────────────

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

/// Page with a group that contains two rects.
const ADD_GROUP_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)320 h=(px)200 {
      group id="grp1" {
        rect id="g.a" x=(px)0 y=(px)0 w=(px)50 h=(px)50
        rect id="g.b" x=(px)0 y=(px)0 w=(px)50 h=(px)50
      }
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

// ── RemoveNode tests ──────────────────────────────────────────────────────

#[test]
fn remove_node_top_level() {
    let doc = parse(TWO_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::RemoveNode {
            node: "a".to_owned(),
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
    assert_eq!(result.affected_node_ids, vec!["a".to_owned()]);
    assert!(
        !result.source_after.contains("id=\"a\""),
        "node a must be gone from source_after; got:\n{}",
        result.source_after
    );
    assert!(result.source_after.contains("id=\"b\""), "b must remain");
}

#[test]
fn remove_node_nested_in_group() {
    let doc = parse(ADD_GROUP_DOC);
    let tx = Transaction {
        ops: vec![Op::RemoveNode {
            node: "g.a".to_owned(),
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
    assert_eq!(result.affected_node_ids, vec!["g.a".to_owned()]);
    assert!(
        !result.source_after.contains("id=\"g.a\""),
        "nested node g.a must be gone; got:\n{}",
        result.source_after
    );
    assert!(
        result.source_after.contains("id=\"g.b\""),
        "g.b must remain"
    );
}

#[test]
fn remove_node_unknown_rejected() {
    let doc = parse(TWO_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::RemoveNode {
            node: "nope".to_owned(),
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

// ── from_json round-trip: crud ops ────────────────────────────────────────

#[test]
fn from_json_crud_ops_round_trip() {
    let json = r#"{"ops":[
            {"op":"add_node","parent":"pg1","source":"rect id=\"r\""},
            {"op":"add_node","parent":"pg1","position":{"at":"index","index":2},"source":"rect id=\"r2\""},
            {"op":"remove_node","node":"r"}
        ]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![
                Op::AddNode {
                    parent: "pg1".to_owned(),
                    position: Position::Last,
                    source: r#"rect id="r""#.to_owned(),
                },
                Op::AddNode {
                    parent: "pg1".to_owned(),
                    position: Position::Index { index: 2 },
                    source: r#"rect id="r2""#.to_owned(),
                },
                Op::RemoveNode {
                    node: "r".to_owned(),
                },
            ],
            permissions: Permissions::default(),
        }
    );
}

// ── SetOpacity tests ──────────────────────────────────────────────────────

#[test]
fn set_opacity_on_rect() {
    let doc = parse(TWO_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetOpacity {
            node: "a".to_owned(),
            opacity: 0.5,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["a".to_owned()]);
    assert!(
        result.source_after.contains("opacity=0.5"),
        "source_after must contain opacity=0.5; got:\n{}",
        result.source_after
    );
    assert_ne!(result.source_before, result.source_after);
}

#[test]
fn set_opacity_clamped_above_one() {
    let doc = parse(TWO_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetOpacity {
            node: "a".to_owned(),
            opacity: 1.5,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    // 1.5 clamped to 1.0; formatter writes "1" (or "1.0") — just verify source
    // changed and the candidate has Some(1.0) by checking node in candidate.
    // We check the diagnostic list is clean (no errors) and affected is recorded.
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != zenith_core::Severity::Error),
        "no errors expected; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.affected_node_ids, vec!["a".to_owned()]);
}

#[test]
fn set_opacity_clamped_below_zero() {
    let doc = parse(TWO_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetOpacity {
            node: "a".to_owned(),
            opacity: -0.5,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["a".to_owned()]);
    assert!(
        result.source_after.contains("opacity=0"),
        "clamped-to-0 opacity must appear in source_after; got:\n{}",
        result.source_after
    );
}

#[test]
fn set_opacity_unknown_node_rejected() {
    let doc = parse(TWO_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetOpacity {
            node: "nope".to_owned(),
            opacity: 0.5,
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

// ── from_json round-trip: set_opacity + replace_text ─────────────────────

#[test]
fn from_json_set_opacity_replace_text_round_trip() {
    use crate::op::OpSpan;
    let json = r#"{"ops":[
            {"op":"set_opacity","node":"box","opacity":0.75},
            {"op":"replace_text","node":"lbl","spans":[
                {"text":"Hello","fill":"color.brand","italic":true},
                {"text":" world","underline":false}
            ]}
        ]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![
                Op::SetOpacity {
                    node: "box".to_owned(),
                    opacity: 0.75,
                },
                Op::ReplaceText {
                    node: "lbl".to_owned(),
                    spans: vec![
                        OpSpan {
                            text: "Hello".to_owned(),
                            fill: Some("color.brand".to_owned()),
                            font_weight: None,
                            italic: Some(true),
                            underline: None,
                            strikethrough: None,
                            vertical_align: None,
                            footnote_ref: None,
                        },
                        OpSpan {
                            text: " world".to_owned(),
                            fill: None,
                            font_weight: None,
                            italic: None,
                            underline: Some(false),
                            strikethrough: None,
                            vertical_align: None,
                            footnote_ref: None,
                        },
                    ],
                },
            ],
            permissions: Permissions::default(),
        }
    );
}

// ── ReplaceText tests ─────────────────────────────────────────────────────

#[test]
fn replace_text_updates_spans() {
    use crate::op::OpSpan;
    let doc = parse(TEXT_DOC);
    let tx = Transaction {
        ops: vec![Op::ReplaceText {
            node: "label".to_owned(),
            spans: vec![OpSpan {
                text: "Goodbye".to_owned(),
                fill: None,
                font_weight: None,
                italic: None,
                underline: None,
                strikethrough: None,
                vertical_align: None,
                footnote_ref: None,
            }],
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["label".to_owned()]);
    assert!(
        result.source_after.contains("Goodbye"),
        "source_after must contain new text; got:\n{}",
        result.source_after
    );
    assert!(
        !result.source_after.contains("Hello"),
        "old text must not appear in source_after"
    );
    assert_ne!(result.source_before, result.source_after);
}

#[test]
fn replace_text_on_rect_unsupported() {
    use crate::op::OpSpan;
    let doc = parse(MIXED_DOC);
    let tx = Transaction {
        ops: vec![Op::ReplaceText {
            node: "box1".to_owned(),
            spans: vec![OpSpan {
                text: "hi".to_owned(),
                fill: None,
                font_weight: None,
                italic: None,
                underline: None,
                strikethrough: None,
                vertical_align: None,
                footnote_ref: None,
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
            .any(|d| d.code == "tx.unsupported_property" && d.message.contains("rect")),
        "expected tx.unsupported_property naming rect; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn replace_text_span_with_fill_token() {
    use crate::op::OpSpan;
    // A doc that has both color tokens and a text node.
    const TEXT_WITH_TOKEN_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.a" type="color" value="#ff0000"
    token id="color.b" type="color" value="#0000ff"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      text id="lbl" x=(px)10 y=(px)10 w=(px)200 h=(px)40 {
        span "Original"
      }
    }
  }
}"##;
    let doc2 = parse(TEXT_WITH_TOKEN_DOC);
    let tx = Transaction {
        ops: vec![Op::ReplaceText {
            node: "lbl".to_owned(),
            spans: vec![OpSpan {
                text: "Branded".to_owned(),
                fill: Some("color.a".to_owned()),
                font_weight: None,
                italic: None,
                underline: None,
                strikethrough: None,
                vertical_align: None,
                footnote_ref: None,
            }],
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc2, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "expected Accepted; diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(result.affected_node_ids, vec!["lbl".to_owned()]);
    // The formatter should emit the span's fill token ref in source_after.
    assert!(
        result.source_after.contains("Branded"),
        "new text must appear in source_after; got:\n{}",
        result.source_after
    );
}

// ── DuplicateNode tests ───────────────────────────────────────────────────

/// Document with a single rect and a fill token (needed for post-validate).
const DUP_RECT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.a" type="color" value="#ff0000"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="orig" x=(px)10 y=(px)20 w=(px)80 h=(px)60 fill=(token)"color.a"
    }
  }
}"##;

/// Document with a group containing a rect (for container-rejection test).
const DUP_GROUP_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      group id="grp" {
        rect id="inner" x=(px)0 y=(px)0 w=(px)50 h=(px)50
      }
    }
  }
}"##;

/// Duplicate a leaf rect: parent now has 2 rects, clone right after original,
/// clone has new_id and same geometry/fill.
#[test]
fn duplicate_node_leaf_rect_accepted() {
    let doc = parse(DUP_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::DuplicateNode {
            node: "orig".to_owned(),
            new_id: "orig-copy".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "expected Accepted; diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(result.affected_node_ids, vec!["orig-copy".to_owned()]);

    // Both ids must appear in source_after.
    assert!(
        result.source_after.contains("id=\"orig\""),
        "original must still be present; got:\n{}",
        result.source_after
    );
    assert!(
        result.source_after.contains("id=\"orig-copy\""),
        "clone must be present; got:\n{}",
        result.source_after
    );

    // Clone must appear AFTER the original in source text.
    let pos_orig = result
        .source_after
        .find("id=\"orig\"")
        .expect("orig in source_after");
    let pos_copy = result
        .source_after
        .find("id=\"orig-copy\"")
        .expect("orig-copy in source_after");
    assert!(
        pos_orig < pos_copy,
        "clone should appear after original in source_after"
    );

    // Clone must carry the same geometry and fill as the original.
    // Count occurrences: both nodes should have x=(px)10, y=(px)20, etc.
    assert_eq!(
        result.source_after.matches("x=(px)10").count(),
        2,
        "both orig and clone should have x=(px)10; got:\n{}",
        result.source_after
    );
    assert_eq!(
        result.source_after.matches("w=(px)80").count(),
        2,
        "both orig and clone should have w=(px)80; got:\n{}",
        result.source_after
    );
    assert_eq!(
        result.source_after.matches("(token)\"color.a\"").count(),
        2,
        "both orig and clone should reference color.a; got:\n{}",
        result.source_after
    );

    // source_before has only one rect.
    assert_eq!(
        result.source_before.matches("id=\"orig").count(),
        1,
        "source_before should have only one orig* node"
    );
}

/// Duplicate with a new_id that already exists → post-validate rejects (id.duplicate).
#[test]
fn duplicate_node_colliding_new_id_rejected() {
    // TWO_RECT_DOC has rect "a" and rect "b"; duplicating "a" with new_id="b"
    // creates a second node with id "b" → id.duplicate from post-validate.
    let doc = parse(TWO_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::DuplicateNode {
            node: "a".to_owned(),
            new_id: "b".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Rejected,
        "colliding new_id must be rejected; diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        result.diagnostics.iter().any(|d| d.code == "id.duplicate"),
        "expected id.duplicate diagnostic; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

/// Attempting to duplicate a group → tx.unsupported_property (v0 scope).
#[test]
fn duplicate_node_container_group_rejected() {
    let doc = parse(DUP_GROUP_DOC);
    let tx = Transaction {
        ops: vec![Op::DuplicateNode {
            node: "grp".to_owned(),
            new_id: "grp-copy".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| { d.code == "tx.unsupported_property" && d.message.contains("group") }),
        "expected tx.unsupported_property mentioning group; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

/// Attempting to duplicate an unknown node id → tx.unknown_node.
#[test]
fn duplicate_node_unknown_id_rejected() {
    let doc = parse(TWO_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::DuplicateNode {
            node: "does_not_exist".to_owned(),
            new_id: "copy".to_owned(),
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

/// Extend the ops serde round-trip to include duplicate_node.
#[test]
fn from_json_duplicate_node_round_trip() {
    let json = r#"{"ops":[{"op":"duplicate_node","node":"orig","new_id":"orig-copy"}]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![Op::DuplicateNode {
                node: "orig".to_owned(),
                new_id: "orig-copy".to_owned(),
            }],
            permissions: Permissions::default(),
        }
    );
}

// ── DuplicatePage tests ───────────────────────────────────────────────────

/// Single page with two leaf nodes; used for duplicate_page tests.
const DUP_PAGE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.a" type="color" value="#ff0000"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="r1" x=(px)10 y=(px)20 w=(px)80 h=(px)60 fill=(token)"color.a"
      rect id="r2" x=(px)10 y=(px)20 w=(px)80 h=(px)60 fill=(token)"color.a"
    }
  }
}"##;

/// Duplicate a 1-page doc with 2 nodes: doc now has 2 pages, the copy has the
/// new page id, the copy's nodes carry the suffix, and the source is unchanged.
#[test]
fn duplicate_page_accepted() {
    let doc = parse(DUP_PAGE_DOC);
    let tx = Transaction {
        ops: vec![Op::DuplicatePage {
            page: "pg1".to_owned(),
            new_id: "pg2".to_owned(),
            id_suffix: ".v2".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "expected Accepted; diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(result.affected_node_ids, vec!["pg2".to_owned()]);

    // Both page ids present; the new page appears after the original.
    assert!(result.source_after.contains("page id=\"pg1\""));
    assert!(result.source_after.contains("page id=\"pg2\""));
    let pos_pg1 = result
        .source_after
        .find("page id=\"pg1\"")
        .expect("pg1 in source_after");
    let pos_pg2 = result
        .source_after
        .find("page id=\"pg2\"")
        .expect("pg2 in source_after");
    assert!(pos_pg1 < pos_pg2, "new page should follow the source page");

    // The copy's node ids are <orig><suffix>.
    assert!(
        result.source_after.contains("id=\"r1.v2\""),
        "clone node r1.v2 must be present; got:\n{}",
        result.source_after
    );
    assert!(
        result.source_after.contains("id=\"r2.v2\""),
        "clone node r2.v2 must be present; got:\n{}",
        result.source_after
    );

    // (b) The source page's nodes are NOT renamed — original ids still appear,
    // and they appear exactly once each (only the source carries them).
    assert_eq!(
        result.source_after.matches("id=\"r1\"").count(),
        1,
        "source node r1 must be unchanged and unique; got:\n{}",
        result.source_after
    );
    assert_eq!(
        result.source_after.matches("id=\"r2\"").count(),
        1,
        "source node r2 must be unchanged and unique; got:\n{}",
        result.source_after
    );

    // source_before has only one page.
    assert_eq!(
        result.source_before.matches("page id=").count(),
        1,
        "source_before should have only one page"
    );
}

/// Duplicate with an empty id_suffix → cloned node ids collide with the
/// originals → post-validation rejects via id.duplicate.
#[test]
fn duplicate_page_empty_suffix_rejected() {
    let doc = parse(DUP_PAGE_DOC);
    let tx = Transaction {
        ops: vec![Op::DuplicatePage {
            page: "pg1".to_owned(),
            new_id: "pg2".to_owned(),
            id_suffix: String::new(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Rejected,
        "empty suffix must be rejected; diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        result.diagnostics.iter().any(|d| d.code == "id.duplicate"),
        "expected id.duplicate diagnostic; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

/// Duplicate an unknown source page → tx.unknown_node, transaction rejected.
#[test]
fn duplicate_page_unknown_page_rejected() {
    let doc = parse(DUP_PAGE_DOC);
    let tx = Transaction {
        ops: vec![Op::DuplicatePage {
            page: "does_not_exist".to_owned(),
            new_id: "pg2".to_owned(),
            id_suffix: ".v2".to_owned(),
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

/// Serde round-trip for the duplicate_page op.
#[test]
fn from_json_duplicate_page_round_trip() {
    let json =
        r#"{"ops":[{"op":"duplicate_page","page":"page.x","new_id":"page.x2","id_suffix":".v2"}]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![Op::DuplicatePage {
                page: "page.x".to_owned(),
                new_id: "page.x2".to_owned(),
                id_suffix: ".v2".to_owned(),
            }],
            permissions: Permissions::default(),
        }
    );
}

// ── AddPage / DeletePage / ReorderPages tests ─────────────────────────────

/// A two-page document used to exercise the page-structure ops.
const TWO_PAGE_STRUCT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="r1" x=(px)0 y=(px)0 w=(px)100 h=(px)100
    }
    page id="pg2" w=(px)400 h=(px)300 {
      rect id="r2" x=(px)0 y=(px)0 w=(px)100 h=(px)100
    }
  }
}"##;

/// Page ids, in document order, parsed back out of `source_after`.
fn page_id_order(source: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut rest = source;
    while let Some(idx) = rest.find("page id=\"") {
        let after = &rest[idx + "page id=\"".len()..];
        if let Some(end) = after.find('"') {
            ids.push(after[..end].to_owned());
            rest = &after[end..];
        } else {
            break;
        }
    }
    ids
}

#[test]
fn add_page_append() {
    let doc = parse(TWO_PAGE_STRUCT_DOC);
    let tx = Transaction {
        ops: vec![Op::AddPage {
            id: "pg3".to_owned(),
            w: "(px)800".to_owned(),
            h: "(px)600".to_owned(),
            background: Some("color.bg".to_owned()),
            index: None,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(
        page_id_order(&result.source_after),
        vec!["pg1", "pg2", "pg3"],
        "new page must be appended last"
    );
    assert_eq!(result.affected_node_ids, vec!["pg3".to_owned()]);
}

#[test]
fn add_page_at_index() {
    let doc = parse(TWO_PAGE_STRUCT_DOC);
    let tx = Transaction {
        ops: vec![Op::AddPage {
            id: "pg.mid".to_owned(),
            w: "(px)800".to_owned(),
            h: "(px)600".to_owned(),
            background: None,
            index: Some(1),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(
        page_id_order(&result.source_after),
        vec!["pg1", "pg.mid", "pg2"],
        "new page must be inserted at index 1"
    );
}

#[test]
fn add_page_duplicate_id_rejected() {
    let doc = parse(TWO_PAGE_STRUCT_DOC);
    let tx = Transaction {
        ops: vec![Op::AddPage {
            id: "pg1".to_owned(),
            w: "(px)800".to_owned(),
            h: "(px)600".to_owned(),
            background: None,
            index: None,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");
    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.duplicate_id"),
        "expected tx.duplicate_id; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn add_page_out_of_range_index_rejected() {
    let doc = parse(TWO_PAGE_STRUCT_DOC);
    let tx = Transaction {
        ops: vec![Op::AddPage {
            id: "pg3".to_owned(),
            w: "(px)800".to_owned(),
            h: "(px)600".to_owned(),
            background: None,
            index: Some(5),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");
    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.out_of_range"),
        "expected tx.out_of_range; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn add_page_invalid_dimension_rejected() {
    let doc = parse(TWO_PAGE_STRUCT_DOC);
    let tx = Transaction {
        ops: vec![Op::AddPage {
            id: "pg3".to_owned(),
            w: "not-a-dim".to_owned(),
            h: "(px)600".to_owned(),
            background: None,
            index: None,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");
    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_value"),
        "expected tx.invalid_value; got: {:?}",
        result.diagnostics
    );
}

#[test]
fn delete_page_removes() {
    let doc = parse(TWO_PAGE_STRUCT_DOC);
    let tx = Transaction {
        ops: vec![Op::DeletePage {
            page: "pg1".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(
        page_id_order(&result.source_after),
        vec!["pg2"],
        "pg1 must be removed"
    );
    assert_eq!(result.affected_node_ids, vec!["pg1".to_owned()]);
}

#[test]
fn delete_page_unknown_rejected() {
    let doc = parse(TWO_PAGE_STRUCT_DOC);
    let tx = Transaction {
        ops: vec![Op::DeletePage {
            page: "nope".to_owned(),
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
fn reorder_pages_permutation() {
    let doc = parse(TWO_PAGE_STRUCT_DOC);
    let tx = Transaction {
        ops: vec![Op::ReorderPages {
            order: vec!["pg2".to_owned(), "pg1".to_owned()],
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(
        page_id_order(&result.source_after),
        vec!["pg2", "pg1"],
        "pages must be reordered to match `order`"
    );
}

#[test]
fn reorder_pages_missing_id_rejected() {
    let doc = parse(TWO_PAGE_STRUCT_DOC);
    let tx = Transaction {
        ops: vec![Op::ReorderPages {
            order: vec!["pg1".to_owned()],
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");
    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_value"),
        "expected tx.invalid_value; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn reorder_pages_extra_id_rejected() {
    let doc = parse(TWO_PAGE_STRUCT_DOC);
    let tx = Transaction {
        ops: vec![Op::ReorderPages {
            order: vec!["pg1".to_owned(), "pg2".to_owned(), "pg3".to_owned()],
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");
    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_value"),
        "expected tx.invalid_value; got: {:?}",
        result.diagnostics
    );
}

#[test]
fn reorder_pages_duplicate_id_rejected() {
    let doc = parse(TWO_PAGE_STRUCT_DOC);
    let tx = Transaction {
        ops: vec![Op::ReorderPages {
            order: vec!["pg1".to_owned(), "pg1".to_owned()],
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");
    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_value"),
        "expected tx.invalid_value; got: {:?}",
        result.diagnostics
    );
}

#[test]
fn from_json_add_page_round_trip() {
    let json =
        r#"{"ops":[{"op":"add_page","id":"page.new","w":"(px)1800","h":"(px)1200","index":1}]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![Op::AddPage {
                id: "page.new".to_owned(),
                w: "(px)1800".to_owned(),
                h: "(px)1200".to_owned(),
                background: None,
                index: Some(1),
            }],
            permissions: Permissions::default(),
        }
    );
}

#[test]
fn from_json_reorder_pages_round_trip() {
    let json = r#"{"ops":[{"op":"reorder_pages","order":["b","a"]}]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![Op::ReorderPages {
                order: vec!["b".to_owned(), "a".to_owned()],
            }],
            permissions: Permissions::default(),
        }
    );
}

// ── Group / Ungroup / Reparent test documents ─────────────────────────────

/// Two sibling rects on a page; used for group/reparent tests.
const TWO_SIBLING_RECTS: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="r1" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      rect id="r2" x=(px)0 y=(px)0 w=(px)100 h=(px)100
    }
  }
}"##;

/// A page with a group that already exists (for ungroup / reparent tests).
const PAGE_WITH_GROUP: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      group id="grp1" {
        rect id="r1" x=(px)0 y=(px)0 w=(px)100 h=(px)100
        rect id="r2" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      }
      rect id="r3" x=(px)0 y=(px)0 w=(px)50 h=(px)50
    }
  }
}"##;

/// A page with a group that has a non-zero x/y offset (advisory test).
const PAGE_WITH_OFFSET_GROUP: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      group id="grp1" x=(px)50 y=(px)20 {
        rect id="r1" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      }
    }
  }
}"##;

/// A page with a group nested inside another group (cycle check + reparent).
const NESTED_GROUPS: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      group id="outer" {
        group id="inner" {
          rect id="r1" x=(px)0 y=(px)0 w=(px)50 h=(px)50
        }
      }
    }
  }
}"##;

// ── Group tests ───────────────────────────────────────────────────────────

/// Group two sibling rects → parent now has one group containing both,
/// inserted at the position of the first (r1's original index = 0).
#[test]
fn group_two_sibling_rects() {
    let doc = parse(TWO_SIBLING_RECTS);
    let tx = Transaction {
        ops: vec![Op::Group {
            node_ids: vec!["r1".to_owned(), "r2".to_owned()],
            group_id: "grp-new".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        result.affected_node_ids.contains(&"grp-new".to_owned()),
        "grp-new must be in affected_node_ids"
    );
    // The page should now contain exactly one top-level node: the group.
    assert!(
        result.source_after.contains("id=\"grp-new\""),
        "source_after must contain the new group id"
    );
    // r1 and r2 are inside the group, not at the page level.
    // Both ids should still appear in the source (as group children).
    assert!(
        result.source_after.contains("id=\"r1\""),
        "r1 must appear inside the group"
    );
    assert!(
        result.source_after.contains("id=\"r2\""),
        "r2 must appear inside the group"
    );
    // r1 must appear before r2 in source_after (relative order preserved).
    let pos_r1 = result
        .source_after
        .find("id=\"r1\"")
        .expect("r1 in source_after");
    let pos_r2 = result
        .source_after
        .find("id=\"r2\"")
        .expect("r2 in source_after");
    assert!(pos_r1 < pos_r2, "r1 must precede r2 inside the group");
    // The group must appear before both rects in source_after (group wraps them).
    let pos_grp = result
        .source_after
        .find("id=\"grp-new\"")
        .expect("grp-new in source_after");
    assert!(pos_grp < pos_r1, "group node must open before its children");
}

/// Attempting to group nodes that do not share a parent → tx.invalid_parent.
#[test]
fn group_non_siblings_rejected() {
    let doc = parse(PAGE_WITH_GROUP);
    // r1 is inside grp1, r3 is a top-level sibling of grp1 → different parents.
    let tx = Transaction {
        ops: vec![Op::Group {
            node_ids: vec!["r1".to_owned(), "r3".to_owned()],
            group_id: "grp-bad".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

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

// ── Ungroup tests ─────────────────────────────────────────────────────────

/// Ungroup a group → its children move up to the parent in order, group gone.
#[test]
fn ungroup_splices_children_in_place() {
    let doc = parse(PAGE_WITH_GROUP);
    let tx = Transaction {
        ops: vec![Op::Ungroup {
            group_id: "grp1".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    // AcceptedWithWarnings is fine; exact status depends on post-validate.
    assert_ne!(
        result.status,
        TxStatus::Rejected,
        "ungroup must not be rejected; diagnostics: {:?}",
        result.diagnostics
    );
    // The group id should no longer appear in source_after.
    assert!(
        !result.source_after.contains("id=\"grp1\""),
        "group grp1 must be gone from source_after;\n{}",
        result.source_after
    );
    // r1 and r2 must still be present (now at page level).
    assert!(
        result.source_after.contains("id=\"r1\""),
        "r1 must appear in source_after"
    );
    assert!(
        result.source_after.contains("id=\"r2\""),
        "r2 must appear in source_after"
    );
    // r1 must appear before r2 (order preserved).
    let pos_r1 = result
        .source_after
        .find("id=\"r1\"")
        .expect("r1 in source_after");
    let pos_r2 = result
        .source_after
        .find("id=\"r2\"")
        .expect("r2 in source_after");
    assert!(pos_r1 < pos_r2, "r1 must precede r2 after ungroup");
    // r3 must still be present.
    assert!(
        result.source_after.contains("id=\"r3\""),
        "r3 must remain in source_after"
    );
}

/// Ungrouping a node that is not a group → tx.unsupported_property.
#[test]
fn ungroup_non_group_rejected() {
    let doc = parse(PAGE_WITH_GROUP);
    let tx = Transaction {
        ops: vec![Op::Ungroup {
            group_id: "r1".to_owned(), // r1 is a rect, not a group
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unsupported_property"),
        "expected tx.unsupported_property; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

/// Ungrouping a group with non-zero x/y emits an advisory but still applies.
#[test]
fn ungroup_with_offset_emits_advisory() {
    let doc = parse(PAGE_WITH_OFFSET_GROUP);
    let tx = Transaction {
        ops: vec![Op::Ungroup {
            group_id: "grp1".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    // Must not be rejected.
    assert_ne!(
        result.status,
        TxStatus::Rejected,
        "ungroup with offset must not be rejected; diagnostics: {:?}",
        result.diagnostics
    );
    // Advisory (tx.noop) must be present.
    assert!(
        result.diagnostics.iter().any(|d| d.code == "tx.noop"),
        "expected tx.noop advisory for offset group; got: {:?}",
        result.diagnostics
    );
    // Group must be gone; r1 must remain.
    assert!(
        !result.source_after.contains("id=\"grp1\""),
        "group must be dissolved"
    );
    assert!(
        result.source_after.contains("id=\"r1\""),
        "r1 must survive ungroup"
    );
}

// ── Reparent tests ────────────────────────────────────────────────────────

/// Move a top-level rect into an existing group.
#[test]
fn reparent_rect_into_group() {
    let doc = parse(PAGE_WITH_GROUP);
    // r3 is a top-level rect; move it into grp1.
    let tx = Transaction {
        ops: vec![Op::Reparent {
            node: "r3".to_owned(),
            new_parent: "grp1".to_owned(),
            position: Position::Last,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        result.affected_node_ids.contains(&"r3".to_owned()),
        "r3 must be in affected_node_ids"
    );
    // r3 must still be present somewhere in the output.
    assert!(
        result.source_after.contains("id=\"r3\""),
        "r3 must appear in source_after"
    );
    // grp1 must contain r3 (grp1 opens before r3 in the serialised form).
    let pos_grp = result
        .source_after
        .find("id=\"grp1\"")
        .expect("grp1 in source_after");
    let pos_r3 = result
        .source_after
        .find("id=\"r3\"")
        .expect("r3 in source_after");
    assert!(
        pos_grp < pos_r3,
        "r3 must appear after grp1 opens (inside it)"
    );
}

/// Reparent into a non-container (a rect) → tx.invalid_parent.
#[test]
fn reparent_into_non_container_rejected() {
    let doc = parse(PAGE_WITH_GROUP);
    let tx = Transaction {
        ops: vec![Op::Reparent {
            node: "r3".to_owned(),
            new_parent: "r1".to_owned(), // r1 is a rect, not a container
            position: Position::Last,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

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

/// Reparent a group into its own child group → cycle → tx.invalid_parent.
#[test]
fn reparent_into_own_subtree_rejected() {
    let doc = parse(NESTED_GROUPS);
    // Try to move `outer` into `inner` (inner is a descendant of outer).
    let tx = Transaction {
        ops: vec![Op::Reparent {
            node: "outer".to_owned(),
            new_parent: "inner".to_owned(),
            position: Position::Last,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_parent"),
        "expected tx.invalid_parent (cycle); got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── Serde round-trip: group / ungroup / reparent ──────────────────────────

#[test]
fn from_json_group_ungroup_reparent_round_trip() {
    let json = r#"{"ops":[
            {"op":"group","node_ids":["r1","r2"],"group_id":"grp-new"},
            {"op":"ungroup","group_id":"grp1"},
            {"op":"reparent","node":"r3","new_parent":"grp1","position":{"at":"first"}}
        ]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![
                Op::Group {
                    node_ids: vec!["r1".to_owned(), "r2".to_owned()],
                    group_id: "grp-new".to_owned(),
                },
                Op::Ungroup {
                    group_id: "grp1".to_owned(),
                },
                Op::Reparent {
                    node: "r3".to_owned(),
                    new_parent: "grp1".to_owned(),
                    position: Position::First,
                },
            ],
            permissions: Permissions::default(),
        }
    );
}

// ── AlignNodes tests ──────────────────────────────────────────────────────

/// Three sibling rects at different x positions (10, 50, 90) on a 400×300
/// page; all have the same width (80px).
const THREE_RECTS_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="r1" x=(px)10 y=(px)20 w=(px)80 h=(px)50
      rect id="r2" x=(px)50 y=(px)60 w=(px)80 h=(px)50
      rect id="r3" x=(px)90 y=(px)100 w=(px)80 h=(px)50
    }
  }
}"##;

/// Parse a node's x value from `source_after` by looking for
/// `id="<id>"` and then the first `x=(px)<value>` on the same node line.
/// This is intentionally naive — sufficient for deterministic test docs.
fn extract_px_attr(source: &str, node_id: &str, attr: &str) -> Option<f64> {
    // Find the line containing this node id.
    source
        .lines()
        .find(|line| line.contains(&format!("id=\"{node_id}\"")))
        .and_then(|line| {
            let needle = format!("{attr}=(px)");
            let start = line.find(&needle)? + needle.len();
            let rest = &line[start..];
            let end = rest
                .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
                .unwrap_or(rest.len());
            rest[..end].parse::<f64>().ok()
        })
}

// ── align "left" anchor "selection" → all get x = min(x) = 10 ───────────

#[test]
fn align_left_selection() {
    let doc = parse(THREE_RECTS_DOC);
    let tx = Transaction {
        ops: vec![Op::AlignNodes {
            node_ids: vec!["r1".to_owned(), "r2".to_owned(), "r3".to_owned()],
            align: "left".to_owned(),
            anchor: "selection".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    // All three nodes must be affected.
    assert!(result.affected_node_ids.contains(&"r1".to_owned()));
    assert!(result.affected_node_ids.contains(&"r2".to_owned()));
    assert!(result.affected_node_ids.contains(&"r3".to_owned()));

    // All three must have x = 10 (the minimum original x).
    for id in ["r1", "r2", "r3"] {
        let x = extract_px_attr(&result.source_after, id, "x")
            .unwrap_or_else(|| panic!("could not extract x for {id}"));
        assert!((x - 10.0).abs() < 1e-9, "expected x=10 for {id}, got {x}");
    }
}

// ── align "right" anchor "selection" → all right edges equal max(x+w) ───

#[test]
fn align_right_selection() {
    let doc = parse(THREE_RECTS_DOC);
    // ref_right = max(x+w) = max(90, 130, 170) = 170
    // each node: x = 170 - 80 = 90
    let tx = Transaction {
        ops: vec![Op::AlignNodes {
            node_ids: vec!["r1".to_owned(), "r2".to_owned(), "r3".to_owned()],
            align: "right".to_owned(),
            anchor: "selection".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    for id in ["r1", "r2", "r3"] {
        let x = extract_px_attr(&result.source_after, id, "x")
            .unwrap_or_else(|| panic!("could not extract x for {id}"));
        // ref_right=170, w=80 → x = 90
        assert!((x - 90.0).abs() < 1e-9, "expected x=90 for {id}, got {x}");
    }
}

// ── align "hcenter" anchor "page" → x = page_w/2 − w/2 ──────────────────

#[test]
fn align_hcenter_page() {
    let doc = parse(THREE_RECTS_DOC);
    // page_w=400, each rect w=80 → centered x = 400/2 − 80/2 = 200 − 40 = 160
    let tx = Transaction {
        ops: vec![Op::AlignNodes {
            node_ids: vec!["r1".to_owned(), "r2".to_owned(), "r3".to_owned()],
            align: "hcenter".to_owned(),
            anchor: "page".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    for id in ["r1", "r2", "r3"] {
        let x = extract_px_attr(&result.source_after, id, "x")
            .unwrap_or_else(|| panic!("could not extract x for {id}"));
        assert!((x - 160.0).abs() < 1e-9, "expected x=160 for {id}, got {x}");
    }
}

// ── node without geometry (group) in the set → skipped, others aligned ───

/// Doc with two rects and one group; the group has no resolvable bbox.
const RECTS_AND_GROUP_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="r1" x=(px)20 y=(px)0 w=(px)60 h=(px)40
      rect id="r2" x=(px)80 y=(px)0 w=(px)60 h=(px)40
      group id="grp1" { }
    }
  }
}"##;

#[test]
fn align_skips_non_geometry_node() {
    let doc = parse(RECTS_AND_GROUP_DOC);
    let tx = Transaction {
        ops: vec![Op::AlignNodes {
            node_ids: vec!["r1".to_owned(), "grp1".to_owned(), "r2".to_owned()],
            align: "left".to_owned(),
            anchor: "selection".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    // grp1 skipped → advisory, but the tx is still accepted.
    assert_eq!(
        result.status,
        TxStatus::AcceptedWithWarnings,
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unsupported_property" && d.message.contains("grp1")),
        "expected tx.unsupported_property advisory for grp1; got: {:?}",
        result.diagnostics
    );
    // r1 and r2 must still have been aligned (x=20, the minimum).
    for id in ["r1", "r2"] {
        let x = extract_px_attr(&result.source_after, id, "x")
            .unwrap_or_else(|| panic!("could not extract x for {id}"));
        assert!((x - 20.0).abs() < 1e-9, "expected x=20 for {id}, got {x}");
    }
    // grp1 must not appear in affected.
    assert!(
        !result.affected_node_ids.contains(&"grp1".to_owned()),
        "grp1 must not be in affected_node_ids"
    );
}

// ── unknown align value → tx.unsupported_property, rejected ──────────────

#[test]
fn align_nodes_unknown_align_rejected() {
    let doc = parse(THREE_RECTS_DOC);
    let tx = Transaction {
        ops: vec![Op::AlignNodes {
            node_ids: vec!["r1".to_owned()],
            align: "diagonal".to_owned(),
            anchor: "selection".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unsupported_property" && d.message.contains("diagonal")),
        "expected tx.unsupported_property naming \"diagonal\"; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── serde round-trip: align_nodes with default anchor ────────────────────

#[test]
fn from_json_align_nodes_round_trip() {
    // anchor is omitted → should deserialize to "selection" via serde default.
    let json = r#"{"ops":[{"op":"align_nodes","node_ids":["r1","r2"],"align":"left"}]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![Op::AlignNodes {
                node_ids: vec!["r1".to_owned(), "r2".to_owned()],
                align: "left".to_owned(),
                anchor: "selection".to_owned(),
            }],
            permissions: Permissions::default(),
        }
    );
}

// ── LOCKED-NODE enforcement ───────────────────────────────────────────────

#[test]
fn set_geometry_on_locked_node_rejected() {
    let doc = parse(TWO_RECT_DOC);

    // Two ops in one tx: lock the node, then try to move it. The lock check
    // reads the candidate state, so the earlier set_locked locks "a" for the
    // later set_geometry — which must therefore be rejected.
    let tx = Transaction {
        ops: vec![
            Op::SetLocked {
                node: "a".to_owned(),
                locked: true,
            },
            Op::SetGeometry {
                node: "a".to_owned(),
                x: Some(50.0),
                y: None,
                w: None,
                h: None,
            },
        ],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "node.locked" && d.subject_id.as_deref() == Some("a")),
        "expected a node.locked diagnostic naming 'a', got: {:?}",
        result.diagnostics
    );
    // Rejected ⇒ document unchanged.
    assert_eq!(result.source_before, result.source_after);
}

#[test]
fn set_geometry_on_locked_node_allowed_with_permission() {
    let doc = parse(TWO_RECT_DOC);

    let tx = Transaction {
        ops: vec![
            Op::SetLocked {
                node: "a".to_owned(),
                locked: true,
            },
            Op::SetGeometry {
                node: "a".to_owned(),
                x: Some(50.0),
                y: None,
                w: None,
                h: None,
            },
        ],
        permissions: Permissions {
            allow_locked: true,
            allow_raw_visual_literals: false,
        },
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert!(
        !result.diagnostics.iter().any(|d| d.code == "node.locked"),
        "no node.locked diagnostic expected with allow_locked, got: {:?}",
        result.diagnostics
    );
    // Geometry changed: x moved to 50.
    assert!(
        result.source_after.contains("(px)50"),
        "source_after should reflect the moved geometry: {}",
        result.source_after
    );
    assert_ne!(result.source_before, result.source_after);
}

#[test]
fn set_locked_can_unlock_a_locked_node() {
    let doc = parse(TWO_RECT_DOC);

    // Lock then unlock in one tx: set_locked is exempt from the lock guard,
    // so the unlock must be allowed even though the node is locked when it runs.
    let tx = Transaction {
        ops: vec![
            Op::SetLocked {
                node: "a".to_owned(),
                locked: true,
            },
            Op::SetLocked {
                node: "a".to_owned(),
                locked: false,
            },
        ],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert!(
        !result.diagnostics.iter().any(|d| d.code == "node.locked"),
        "set_locked must be exempt from the lock guard, got: {:?}",
        result.diagnostics
    );
}

// ── AlignNodes: explicit-dimension anchor "(px)N" ─────────────────────────

/// Three sibling text/code/rect nodes for overflow + dimension-anchor tests.
const TEXT_CODE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      text id="body" x=(px)10 y=(px)10 w=(px)200 h=(px)40 align="start" {
        span "Hello"
      }
      code id="snip" x=(px)10 y=(px)60 w=(px)200 h=(px)100 {
        content "fn main() {}"
      }
    }
  }
}"##;

// ── align "left" anchor "(px)120" → all left edges become 120 ─────────────

#[test]
fn align_left_dimension_anchor() {
    let doc = parse(THREE_RECTS_DOC);
    let tx = Transaction {
        ops: vec![Op::AlignNodes {
            node_ids: vec!["r1".to_owned(), "r2".to_owned(), "r3".to_owned()],
            align: "left".to_owned(),
            anchor: "(px)120".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    for id in ["r1", "r2", "r3"] {
        let x = extract_px_attr(&result.source_after, id, "x")
            .unwrap_or_else(|| panic!("could not extract x for {id}"));
        assert!((x - 120.0).abs() < 1e-9, "expected x=120 for {id}, got {x}");
    }
}

// ── vertical edge "top" anchor "(px)55" → all top edges (y) become 55 ─────

#[test]
fn align_top_dimension_anchor_sets_y() {
    let doc = parse(THREE_RECTS_DOC);
    let tx = Transaction {
        ops: vec![Op::AlignNodes {
            node_ids: vec!["r1".to_owned(), "r2".to_owned(), "r3".to_owned()],
            align: "top".to_owned(),
            anchor: "(px)55".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    for id in ["r1", "r2", "r3"] {
        let y = extract_px_attr(&result.source_after, id, "y")
            .unwrap_or_else(|| panic!("could not extract y for {id}"));
        assert!((y - 55.0).abs() < 1e-9, "expected y=55 for {id}, got {y}");
        // x must be untouched by a vertical-axis align.
    }
    // x of r2 should still be its original 50 (vertical align leaves x alone).
    let x2 = extract_px_attr(&result.source_after, "r2", "x").expect("x for r2");
    assert!(
        (x2 - 50.0).abs() < 1e-9,
        "expected x unchanged (50), got {x2}"
    );
}

// ── right edge "right" anchor "(px)200" → x = 200 - w ─────────────────────

#[test]
fn align_right_dimension_anchor() {
    let doc = parse(THREE_RECTS_DOC); // each w=80
    let tx = Transaction {
        ops: vec![Op::AlignNodes {
            node_ids: vec!["r1".to_owned(), "r2".to_owned(), "r3".to_owned()],
            align: "right".to_owned(),
            anchor: "(px)200".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    for id in ["r1", "r2", "r3"] {
        let x = extract_px_attr(&result.source_after, id, "x")
            .unwrap_or_else(|| panic!("could not extract x for {id}"));
        // right edge = 200, w = 80 → x = 120
        assert!((x - 120.0).abs() < 1e-9, "expected x=120 for {id}, got {x}");
    }
}

// ── invalid dimension anchor → tx.invalid_value, rejected ─────────────────

#[test]
fn align_invalid_dimension_anchor_rejected() {
    let doc = parse(THREE_RECTS_DOC);
    let tx = Transaction {
        ops: vec![Op::AlignNodes {
            node_ids: vec!["r1".to_owned()],
            align: "left".to_owned(),
            anchor: "(px)notanumber".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_value"),
        "expected tx.invalid_value; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── existing "page" / "selection" anchors still work ──────────────────────

#[test]
fn align_page_anchor_still_works() {
    let doc = parse(THREE_RECTS_DOC);
    let tx = Transaction {
        ops: vec![Op::AlignNodes {
            node_ids: vec!["r1".to_owned(), "r2".to_owned(), "r3".to_owned()],
            align: "left".to_owned(),
            anchor: "page".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    // page left edge = 0 → all x = 0.
    for id in ["r1", "r2", "r3"] {
        let x = extract_px_attr(&result.source_after, id, "x").expect("x");
        assert!((x - 0.0).abs() < 1e-9, "expected x=0 for {id}, got {x}");
    }
}

#[test]
fn align_selection_anchor_still_works() {
    let doc = parse(THREE_RECTS_DOC);
    let tx = Transaction {
        ops: vec![Op::AlignNodes {
            node_ids: vec!["r1".to_owned(), "r2".to_owned(), "r3".to_owned()],
            align: "left".to_owned(),
            anchor: "selection".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    // selection min x = 10.
    for id in ["r1", "r2", "r3"] {
        let x = extract_px_attr(&result.source_after, id, "x").expect("x");
        assert!((x - 10.0).abs() < 1e-9, "expected x=10 for {id}, got {x}");
    }
}

// ── SetTextOverflow tests ─────────────────────────────────────────────────

#[test]
fn set_text_overflow_on_text_accepted() {
    let doc = parse(TEXT_CODE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextOverflow {
            node_id: "body".to_owned(),
            overflow: "visible".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    assert_eq!(result.affected_node_ids, vec!["body".to_owned()]);
    assert!(
        result.source_after.contains("overflow=\"visible\""),
        "source_after should set overflow=\"visible\": {}",
        result.source_after
    );
}

#[test]
fn set_text_overflow_on_code_accepted() {
    let doc = parse(TEXT_CODE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextOverflow {
            node_id: "snip".to_owned(),
            overflow: "clip".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    assert_eq!(result.affected_node_ids, vec!["snip".to_owned()]);
    assert!(
        result.source_after.contains("overflow=\"clip\""),
        "source_after should set overflow=\"clip\": {}",
        result.source_after
    );
}

#[test]
fn set_text_overflow_invalid_value_rejected() {
    let doc = parse(TEXT_CODE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextOverflow {
            node_id: "body".to_owned(),
            overflow: "wrap".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_value" && d.message.contains("wrap")),
        "expected tx.invalid_value naming \"wrap\"; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn set_text_overflow_wrong_node_type_rejected() {
    let doc = parse(THREE_RECTS_DOC); // rects, no overflow field
    let tx = Transaction {
        ops: vec![Op::SetTextOverflow {
            node_id: "r1".to_owned(),
            overflow: "visible".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.wrong_node_type"),
        "expected tx.wrong_node_type; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn set_text_overflow_missing_node_rejected() {
    let doc = parse(TEXT_CODE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextOverflow {
            node_id: "nope".to_owned(),
            overflow: "fit".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unknown_node"),
        "expected tx.unknown_node; got: {:?}",
        result.diagnostics
    );
}

// ── DistributeNodes tests ─────────────────────────────────────────────────

/// Three rects unevenly placed on the x axis: positions 0, 30, 100, widths 20.
/// Span = (100+20) - 0 = 120. Σsizes = 60. gap = (120-60)/2 = 30.
/// Distributed leading edges: 0, 0+20+30=50, 50+20+30=100.
const DISTRIBUTE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="p1" x=(px)0 y=(px)0 w=(px)20 h=(px)20
      rect id="p2" x=(px)30 y=(px)0 w=(px)20 h=(px)20
      rect id="p3" x=(px)100 y=(px)0 w=(px)20 h=(px)20
    }
  }
}"##;

#[test]
fn distribute_horizontal_equal_gaps() {
    let doc = parse(DISTRIBUTE_DOC);
    let tx = Transaction {
        ops: vec![Op::DistributeNodes {
            node_ids: vec!["p1".to_owned(), "p2".to_owned(), "p3".to_owned()],
            axis: "horizontal".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );

    let x1 = extract_px_attr(&result.source_after, "p1", "x").expect("x1");
    let x2 = extract_px_attr(&result.source_after, "p2", "x").expect("x2");
    let x3 = extract_px_attr(&result.source_after, "p3", "x").expect("x3");

    // Endpoints fixed.
    assert!((x1 - 0.0).abs() < 1e-9, "x1 should stay 0, got {x1}");
    assert!((x3 - 100.0).abs() < 1e-9, "x3 should stay 100, got {x3}");
    // Middle node placed for equal gaps: 50.
    assert!((x2 - 50.0).abs() < 1e-9, "x2 should be 50, got {x2}");

    // Gaps between consecutive trailing/leading edges must be equal (= 30).
    let gap_a = x2 - (x1 + 20.0);
    let gap_b = x3 - (x2 + 20.0);
    assert!(
        (gap_a - gap_b).abs() < 1e-9,
        "gaps unequal: {gap_a} vs {gap_b}"
    );
    assert!((gap_a - 30.0).abs() < 1e-9, "gap should be 30, got {gap_a}");
}

#[test]
fn distribute_orders_by_position_first() {
    // Same geometry but listed out of order: p3, p1, p2. Result must match the
    // position-ordered distribution (p1 fixed at 0, p3 fixed at 100, p2 at 50).
    let doc = parse(DISTRIBUTE_DOC);
    let tx = Transaction {
        ops: vec![Op::DistributeNodes {
            node_ids: vec!["p3".to_owned(), "p1".to_owned(), "p2".to_owned()],
            axis: "horizontal".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    let x1 = extract_px_attr(&result.source_after, "p1", "x").expect("x1");
    let x2 = extract_px_attr(&result.source_after, "p2", "x").expect("x2");
    let x3 = extract_px_attr(&result.source_after, "p3", "x").expect("x3");
    assert!((x1 - 0.0).abs() < 1e-9, "x1={x1}");
    assert!((x2 - 50.0).abs() < 1e-9, "x2={x2}");
    assert!((x3 - 100.0).abs() < 1e-9, "x3={x3}");
}

#[test]
fn distribute_too_few_nodes_is_noop() {
    let doc = parse(DISTRIBUTE_DOC);
    let tx = Transaction {
        ops: vec![Op::DistributeNodes {
            node_ids: vec!["p1".to_owned(), "p2".to_owned()],
            axis: "horizontal".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    // Degenerate input → tx.noop advisory, document unchanged, still Accepted.
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    assert!(
        result.diagnostics.iter().any(|d| d.code == "tx.noop"),
        "expected tx.noop advisory; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
    assert!(result.affected_node_ids.is_empty());
}

#[test]
fn distribute_missing_node_rejected() {
    let doc = parse(DISTRIBUTE_DOC);
    let tx = Transaction {
        ops: vec![Op::DistributeNodes {
            node_ids: vec!["p1".to_owned(), "ghost".to_owned(), "p3".to_owned()],
            axis: "horizontal".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unknown_node" && d.message.contains("ghost")),
        "expected tx.unknown_node naming \"ghost\"; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn distribute_vertical_equal_gaps() {
    // Place three rects unevenly on y: 0, 30, 100, height 20. Same arithmetic.
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="q1" x=(px)0 y=(px)0 w=(px)20 h=(px)20
      rect id="q2" x=(px)0 y=(px)30 w=(px)20 h=(px)20
      rect id="q3" x=(px)0 y=(px)100 w=(px)20 h=(px)20
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::DistributeNodes {
            node_ids: vec!["q1".to_owned(), "q2".to_owned(), "q3".to_owned()],
            axis: "vertical".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    let y2 = extract_px_attr(&result.source_after, "q2", "y").expect("y2");
    assert!((y2 - 50.0).abs() < 1e-9, "y2 should be 50, got {y2}");
}

#[test]
fn distribute_unknown_axis_rejected() {
    let doc = parse(DISTRIBUTE_DOC);
    let tx = Transaction {
        ops: vec![Op::DistributeNodes {
            node_ids: vec!["p1".to_owned(), "p2".to_owned(), "p3".to_owned()],
            axis: "diagonal".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unsupported_property" && d.message.contains("diagonal")),
        "expected tx.unsupported_property naming \"diagonal\"; got: {:?}",
        result.diagnostics
    );
}

// ── serde round-trips for the new ops ─────────────────────────────────────

#[test]
fn from_json_set_text_overflow_round_trip() {
    let json = r#"{"ops":[{"op":"set_text_overflow","node_id":"body","overflow":"visible"}]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![Op::SetTextOverflow {
                node_id: "body".to_owned(),
                overflow: "visible".to_owned(),
            }],
            permissions: Permissions::default(),
        }
    );
}

#[test]
fn from_json_distribute_nodes_round_trip() {
    let json =
        r#"{"ops":[{"op":"distribute_nodes","node_ids":["p1","p2","p3"],"axis":"horizontal"}]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![Op::DistributeNodes {
                node_ids: vec!["p1".to_owned(), "p2".to_owned(), "p3".to_owned()],
                axis: "horizontal".to_owned(),
            }],
            permissions: Permissions::default(),
        }
    );
}

#[test]
fn from_json_align_dimension_anchor_round_trip() {
    let json = r#"{"ops":[{"op":"align_nodes","node_ids":["a","b","caption"],"align":"left","anchor":"(px)120"}]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![Op::AlignNodes {
                node_ids: vec!["a".to_owned(), "b".to_owned(), "caption".to_owned()],
                align: "left".to_owned(),
                anchor: "(px)120".to_owned(),
            }],
            permissions: Permissions::default(),
        }
    );
}
