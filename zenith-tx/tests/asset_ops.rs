//! Integration tests for the `add_asset` and `set_asset` transaction ops.

mod common;
use common::*;
use zenith_tx::{AddAssetMetadata, Op, Permissions, Transaction, TxStatus, run_transaction};

// ── Fixtures ──────────────────────────────────────────────────────────────────

/// Document with one image asset and an image node referencing it, plus a
/// second asset (font) and a rect for wrong-node-type tests.
const ASSET_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  assets {
    asset id="asset.pic" kind="image" src="images/pic.png"
    asset id="asset.font" kind="font" src="fonts/body.ttf"
  }
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      image id="img1" asset="asset.pic" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      rect id="box1" x=(px)0 y=(px)0 w=(px)50 h=(px)50
    }
  }
}"##;

fn add_asset_op(id: &str, kind: &str, src: &str) -> Op {
    Op::AddAsset {
        id: id.to_owned(),
        kind: kind.to_owned(),
        src: src.to_owned(),
        sha256: None,
        metadata: Box::new(AddAssetMetadata::default()),
    }
}

// ── add_asset: accepted ───────────────────────────────────────────────────────

/// (a) add_asset with a new id is accepted; source_after contains the new id.
#[test]
fn add_asset_accepted() {
    let doc = parse(IMAGE_DOC);
    let tx = Transaction {
        ops: vec![add_asset_op("asset.hero", "image", "images/hero.png")],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "expected Accepted; diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(result.affected_node_ids, vec!["asset.hero".to_owned()]);
    assert!(
        result.source_after.contains("id=\"asset.hero\""),
        "source_after must contain the new asset id; got:\n{}",
        result.source_after
    );
    assert!(
        result.source_after.contains("src=\"images/hero.png\""),
        "source_after must contain the new asset src; got:\n{}",
        result.source_after
    );
}

#[test]
fn add_asset_accepted_from_legacy_json() {
    let doc = parse(IMAGE_DOC);
    let tx: Transaction = serde_json::from_str(
        r#"{"ops":[{"op":"add_asset","id":"asset.hero","kind":"image","src":"images/hero.png","sha256":"abc123"}],"permissions":{}}"#,
    )
    .expect("legacy add_asset JSON should deserialize");
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "expected Accepted; diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        result.source_after.contains(
            r#"asset id="asset.hero" kind="image" src="images/hero.png" sha256="abc123""#
        ),
        "source_after must contain the legacy asset declaration; got:\n{}",
        result.source_after
    );
}

#[test]
fn add_asset_with_provenance_formats_asset_declaration() {
    let doc = parse(IMAGE_DOC);
    let tx = Transaction {
        ops: vec![Op::AddAsset {
            id: "asset.ai".to_owned(),
            kind: "image".to_owned(),
            src: "images/ai.png".to_owned(),
            sha256: Some("abc123".to_owned()),
            metadata: Box::new(AddAssetMetadata {
                ai_prompt: Some("A geometric poster".to_owned()),
                ai_model: Some("gpt-image-1".to_owned()),
                ai_provider: Some("openai".to_owned()),
                ai_seed: Some(42),
                ai_generation_date: Some("2026-07-07".to_owned()),
                ai_license: Some("project-owned".to_owned()),
                ai_source_rights: Some("original".to_owned()),
                ai_safety_status: Some("reviewed".to_owned()),
                ai_reuse_policy: Some("internal".to_owned()),
                ..AddAssetMetadata::default()
            }),
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
    assert!(
        result.source_after.contains(
            r#"asset id="asset.ai" kind="image" src="images/ai.png" sha256="abc123" ai-prompt="A geometric poster" ai-model="gpt-image-1" ai-provider="openai" ai-seed=42 ai-generation-date="2026-07-07" ai-license="project-owned" ai-source-rights="original" ai-safety-status="reviewed" ai-reuse-policy="internal""#
        ),
        "source_after must contain the canonical asset provenance fields; got:\n{}",
        result.source_after
    );
}

#[test]
fn add_asset_with_producer_provenance_formats_asset_declaration() {
    let doc = parse(IMAGE_DOC);
    let tx = Transaction {
        ops: vec![Op::AddAsset {
            id: "asset.baked".to_owned(),
            kind: "image".to_owned(),
            src: "images/baked.png".to_owned(),
            sha256: Some("def456".to_owned()),
            metadata: Box::new(AddAssetMetadata {
                producer_kind: Some("zpx-bake".to_owned()),
                producer_source: Some("painting.zpx".to_owned()),
                ..AddAssetMetadata::default()
            }),
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
    assert!(
        result.source_after.contains(
            r#"asset id="asset.baked" kind="image" src="images/baked.png" sha256="def456" producer-kind="zpx-bake" producer-source="painting.zpx""#
        ),
        "source_after must contain the producer provenance fields; got:\n{}",
        result.source_after
    );
}

// ── add_asset: duplicate id rejected ─────────────────────────────────────────

/// (b) add_asset with an id that already exists → Rejected (tx.duplicate_id).
#[test]
fn add_asset_duplicate_id_rejected() {
    let doc = parse(IMAGE_DOC);
    // IMAGE_DOC already declares "asset.pic".
    let tx = Transaction {
        ops: vec![add_asset_op("asset.pic", "image", "other/pic.png")],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Rejected,
        "expected Rejected; diagnostics: {:?}",
        result.diagnostics
    );
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

// ── add_asset: invalid src rejected via post-validation ──────────────────────

/// (c) add_asset with src="../escape.png" → Rejected (asset.invalid_src from
/// post-validation).
#[test]
fn add_asset_invalid_src_rejected() {
    let doc = parse(IMAGE_DOC);
    let tx = Transaction {
        ops: vec![add_asset_op("asset.escape", "image", "../escape.png")],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Rejected,
        "expected Rejected; diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "asset.invalid_src"),
        "expected asset.invalid_src; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── set_asset: changes image node's asset ────────────────────────────────────

/// (d) set_asset on an image node with a valid 2nd asset → Accepted; source_after
/// shows the new asset reference.
#[test]
fn set_asset_changes_image_node() {
    // ASSET_DOC already has asset.pic and asset.font; we add asset.hero first,
    // then set_asset the image to it.
    let doc = parse(ASSET_DOC);
    let tx = Transaction {
        ops: vec![
            add_asset_op("asset.hero", "image", "images/hero.png"),
            Op::SetAsset {
                node_id: "img1".to_owned(),
                asset_id: "asset.hero".to_owned(),
            },
        ],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "expected Accepted; diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        result.affected_node_ids.contains(&"img1".to_owned()),
        "img1 must be in affected; got: {:?}",
        result.affected_node_ids
    );
    assert!(
        result.source_after.contains("asset=\"asset.hero\""),
        "source_after must show the updated asset ref; got:\n{}",
        result.source_after
    );
}

// ── set_asset: font-kind asset rejected ──────────────────────────────────────

/// (e) set_asset targeting a font-kind asset → Rejected (tx.invalid_value).
#[test]
fn set_asset_font_kind_rejected() {
    // ASSET_DOC has asset.font with kind="font".
    let doc = parse(ASSET_DOC);
    let tx = Transaction {
        ops: vec![Op::SetAsset {
            node_id: "img1".to_owned(),
            asset_id: "asset.font".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Rejected,
        "expected Rejected; diagnostics: {:?}",
        result.diagnostics
    );
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

// ── set_asset: non-image node rejected ───────────────────────────────────────

/// (f) set_asset on a non-image node (a rect) → Rejected (tx.wrong_node_type).
#[test]
fn set_asset_on_rect_rejected() {
    let doc = parse(ASSET_DOC);
    let tx = Transaction {
        ops: vec![Op::SetAsset {
            node_id: "box1".to_owned(),
            asset_id: "asset.pic".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Rejected,
        "expected Rejected; diagnostics: {:?}",
        result.diagnostics
    );
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

// ── set_asset: unknown asset_id allowed through → post-validation rejects ────

/// (g) set_asset with an unknown asset_id → Rejected (asset.unknown_reference
/// from post-validation).
#[test]
fn set_asset_unknown_asset_id_rejected() {
    let doc = parse(ASSET_DOC);
    let tx = Transaction {
        ops: vec![Op::SetAsset {
            node_id: "img1".to_owned(),
            asset_id: "asset.does_not_exist".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Rejected,
        "expected Rejected; diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "asset.unknown_reference"),
        "expected asset.unknown_reference; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}
