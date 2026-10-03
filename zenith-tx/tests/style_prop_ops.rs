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

// ── create_style / delete_style ───────────────────────────────────────────────

#[test]
fn create_style_accepted() {
    let doc = parse(STYLE_PROP_DOC);
    let mut properties = std::collections::BTreeMap::new();
    properties.insert("fill".to_owned(), "color.accent".to_owned());
    properties.insert("font-size".to_owned(), "size.md".to_owned());
    let tx = Transaction {
        ops: vec![Op::CreateStyle {
            id: "s.cta".to_owned(),
            properties,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");
    assert_eq!(result.status, TxStatus::Accepted);
    assert!(result.source_after.contains("s.cta"));
    assert!(result.source_after.contains("color.accent"));
}

#[test]
fn create_style_duplicate_rejected() {
    let doc = parse(STYLE_PROP_DOC);
    let tx = Transaction {
        ops: vec![Op::CreateStyle {
            id: "s.heading".to_owned(),
            properties: std::collections::BTreeMap::new(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");
    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.duplicate_id")
    );
}

#[test]
fn delete_style_accepted() {
    let doc = parse(STYLE_PROP_DOC);
    let tx = Transaction {
        ops: vec![Op::DeleteStyle {
            id: "s.heading".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");
    assert_eq!(result.status, TxStatus::Accepted);
    assert!(!result.source_after.contains("s.heading"));
}

// ── Enum-valued keys: align / v-align store a checked literal ────────────────

fn set_prop(property: &str, value: &str) -> zenith_tx::TxResult {
    let doc = parse(STYLE_PROP_DOC);
    let tx = Transaction {
        ops: vec![Op::SetStyleProperty {
            style_id: "s.heading".to_owned(),
            property: property.to_owned(),
            value: value.to_owned(),
        }],
        permissions: Permissions::default(),
    };
    run_transaction(&doc, &tx).expect("run_transaction should not error")
}

#[test]
fn set_style_property_accepts_enum_literals() {
    for (property, value, line) in [
        ("align", "center", "align \"center\""),
        ("v-align", "bottom", "v-align \"bottom\""),
        ("v_align", "middle", "v-align \"middle\""),
    ] {
        let result = set_prop(property, value);
        assert_eq!(result.status, TxStatus::Accepted, "{property}={value}");
        assert!(
            result.source_after.contains(line),
            "{property}: expected `{line}` in:\n{}",
            result.source_after
        );
        assert!(!result.source_after.contains("(token)\"center\""));
    }
}

#[test]
fn set_style_property_rejects_enum_value_outside_list() {
    for (property, value) in [("align", "left"), ("v-align", "center")] {
        let result = set_prop(property, value);
        assert_eq!(result.status, TxStatus::Rejected, "{property}={value}");
        let d = result
            .diagnostics
            .iter()
            .find(|d| d.code == "tx.invalid_value")
            .unwrap_or_else(|| panic!("tx.invalid_value for {property}={value}"));
        assert!(d.message.contains(value), "{}", d.message);
        assert_eq!(result.source_after, result.source_before);
    }
}

#[test]
fn set_style_property_shadow_stores_token_ref() {
    let src = STYLE_PROP_DOC.replace(
        "    token id=\"color.accent\" type=\"color\" value=\"#3b82f6\"\n",
        "    token id=\"color.accent\" type=\"color\" value=\"#3b82f6\"\n    \
         token id=\"shadow.card\" type=\"shadow\" {\n      \
         layer dx=(px)0 dy=(px)2 blur=(px)6 color=(token)\"color.accent\"\n    }\n",
    );
    assert!(src.contains("shadow.card"));
    let tx = Transaction {
        ops: vec![Op::SetStyleProperty {
            style_id: "s.heading".to_owned(),
            property: "shadow".to_owned(),
            value: "shadow.card".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&parse(&src), &tx).expect("run_transaction should not error");
    assert_eq!(result.status, TxStatus::Accepted);
    assert!(
        result
            .source_after
            .contains("shadow (token)\"shadow.card\""),
        "{}",
        result.source_after
    );
}

#[test]
fn create_style_checks_enum_keys() {
    let doc = parse(STYLE_PROP_DOC);
    let run = |value: &str| {
        let tx = Transaction {
            ops: vec![Op::CreateStyle {
                id: "s.new".to_owned(),
                properties: [("align".to_owned(), value.to_owned())]
                    .into_iter()
                    .collect(),
            }],
            permissions: Permissions::default(),
        };
        run_transaction(&doc, &tx).expect("run_transaction should not error")
    };
    let ok = run("justify");
    assert_eq!(ok.status, TxStatus::Accepted);
    assert!(
        ok.source_after.contains("align \"justify\""),
        "{}",
        ok.source_after
    );
    let bad = run("middle");
    assert_eq!(bad.status, TxStatus::Rejected);
    assert!(bad.diagnostics.iter().any(|d| d.code == "tx.invalid_value"));
}
