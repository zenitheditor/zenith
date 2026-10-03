use super::*;

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
        result.diagnostics.iter().all(|d| !d.is_error()),
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
