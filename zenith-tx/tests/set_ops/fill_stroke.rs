use super::*;

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

/// Rect, line, polygon carrying valid color + dimension tokens.
const STROKE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.rule" type="color" value="#334155"
    token id="color.on.rule" type="color" value="#ffffff"
    token id="size.stroke" type="dimension" value=(px)2
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="r1" x=(px)0 y=(px)0 w=(px)100 h=(px)100 stroke=(token)"color.rule" stroke-width=(token)"size.stroke"
      line id="ln1" x1=(px)0 y1=(px)0 x2=(px)100 y2=(px)100 stroke=(token)"color.rule"
      ellipse id="dot" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.rule"
      text id="lbl" x=(px)10 y=(px)10 w=(px)200 h=(px)40 fill=(token)"color.on.rule" {
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
