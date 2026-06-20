//! Integration tests for the `set_style_property` transaction op.

mod common;
use common::*;
use zenith_tx::{Op, Permissions, Transaction, TxStatus, run_transaction};

// ── 1. Accepted: set font-family on an existing style ────────────────────────

#[test]
fn set_style_property_accepted() {
    let doc = parse(STYLE_PROP_DOC);
    let tx = Transaction {
        ops: vec![Op::SetStyleProperty {
            style_id: "s.heading".to_owned(),
            property: "font-family".to_owned(),
            value: "font.body".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["s.heading".to_owned()]);
    // The serialized output must contain the updated property.
    assert!(
        result.source_after.contains("font-family"),
        "source_after should contain font-family"
    );
    assert!(
        result.source_after.contains("font.body"),
        "source_after should contain the token id font.body"
    );
    assert_ne!(result.source_before, result.source_after);
}

// ── 2. Underscore spelling canonicalized ─────────────────────────────────────

#[test]
fn set_style_property_underscore_spelling() {
    let doc = parse(STYLE_PROP_DOC);
    let tx = Transaction {
        ops: vec![Op::SetStyleProperty {
            style_id: "s.heading".to_owned(),
            property: "font_family".to_owned(), // underscore form
            value: "font.body".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["s.heading".to_owned()]);
    // After canonicalization the stored key should be the hyphenated form.
    assert!(
        result.source_after.contains("font-family"),
        "canonicalized key font-family should appear in source_after"
    );
}

// ── 3. Unknown style_id → Rejected with tx.unknown_style ─────────────────────

#[test]
fn set_style_property_unknown_style() {
    let doc = parse(STYLE_PROP_DOC);
    let tx = Transaction {
        ops: vec![Op::SetStyleProperty {
            style_id: "s.nonexistent".to_owned(),
            property: "font-family".to_owned(),
            value: "font.body".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unknown_style"),
        "expected tx.unknown_style diagnostic"
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── 4. Unrecognized property → Rejected with tx.unsupported_property ─────────

#[test]
fn set_style_property_bogus_property() {
    let doc = parse(STYLE_PROP_DOC);
    let tx = Transaction {
        ops: vec![Op::SetStyleProperty {
            style_id: "s.heading".to_owned(),
            property: "bogus".to_owned(),
            value: "font.body".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unsupported_property"),
        "expected tx.unsupported_property diagnostic"
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── 5. Nonexistent token → Rejected by post-validate token.unknown_reference ─

#[test]
fn set_style_property_unknown_token_ref() {
    let doc = parse(STYLE_PROP_DOC);
    let tx = Transaction {
        ops: vec![Op::SetStyleProperty {
            style_id: "s.heading".to_owned(),
            property: "font-family".to_owned(),
            value: "font.does.not.exist".to_owned(), // no such token
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    // Post-validation should catch the dangling token reference.
    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "token.unknown_reference"),
        "expected token.unknown_reference diagnostic; got: {:?}",
        result
            .diagnostics
            .iter()
            .map(|d| &d.code)
            .collect::<Vec<_>>()
    );
}

// ── 6. Pre-existing properties on the style are preserved ────────────────────

#[test]
fn set_style_property_preserves_other_props() {
    let doc = parse(STYLE_PROP_DOC);
    // STYLE_PROP_DOC already has `font-size` on s.heading; we add `font-family`.
    let tx = Transaction {
        ops: vec![Op::SetStyleProperty {
            style_id: "s.heading".to_owned(),
            property: "font-family".to_owned(),
            value: "font.body".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    // The original font-size property should still be present in the output.
    assert!(
        result.source_after.contains("font-size"),
        "source_after should still contain the pre-existing font-size property"
    );
}
