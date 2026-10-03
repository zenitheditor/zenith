use super::*;

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

fn set_overflow(node_id: &str, overflow: &str) -> zenith_tx::TxResult {
    let doc = parse(TEXT_CODE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextOverflow {
            node_id: node_id.to_owned(),
            overflow: overflow.to_owned(),
        }],
        permissions: Permissions::default(),
    };
    run_transaction(&doc, &tx).expect("run_transaction must not error")
}

#[test]
fn set_text_overflow_accepts_every_text_mode() {
    for mode in ["clip", "visible", "fit", "autofit"] {
        let result = set_overflow("body", mode);
        assert_eq!(
            result.status,
            TxStatus::Accepted,
            "{mode}: {:?}",
            result.diagnostics
        );
        assert!(
            result
                .source_after
                .contains(&format!("overflow=\"{mode}\"")),
            "{mode}: {}",
            result.source_after
        );
    }
}

#[test]
fn set_text_overflow_on_code_accepts_clip_and_visible_only() {
    for mode in ["clip", "visible"] {
        let result = set_overflow("snip", mode);
        assert_eq!(
            result.status,
            TxStatus::Accepted,
            "{mode}: {:?}",
            result.diagnostics
        );
    }
    for mode in ["fit", "autofit"] {
        let result = set_overflow("snip", mode);
        assert_eq!(
            result.status,
            TxStatus::Rejected,
            "{mode} must be rejected on code"
        );
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.code == "tx.invalid_value"
                    && d.message.contains(mode)
                    && d.message.contains("must be one of: clip, visible")),
            "{mode}: {:?}",
            result.diagnostics
        );
        assert_eq!(result.source_after, result.source_before);
    }
}

#[test]
fn set_text_overflow_schema_names_the_core_lists() {
    let fields = zenith_tx::schema::op_fields("set_text_overflow").expect("op is described");
    let ty = fields
        .iter()
        .find(|f| f.name == "overflow")
        .map(|f| f.ty)
        .expect("overflow field");
    assert_eq!(
        ty,
        format!(
            "enum: {} (code: {})",
            zenith_core::schema::enums::TEXT_OVERFLOWS.join("|"),
            zenith_core::schema::enums::CODE_OVERFLOWS.join("|")
        )
    );
}
