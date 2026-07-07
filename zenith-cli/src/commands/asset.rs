//! Pure logic for `zenith asset`.

use std::path::Path;
use std::sync::Arc;

use serde::Serialize;
use zenith_core::{AssetKind, KdlAdapter, KdlSource as _};
use zenith_producers::{
    AssetProducer, FileImportProducer, FileImportProvenance, ProduceRequest, ProducedAsset,
    Provenance, ZpxBakeProducer,
};
use zenith_tx::{Op, Permissions, Transaction, TxResult, TxStatus, run_transaction};

use crate::commands::serialize_pretty;
use crate::json_types::DiagnosticJson;

#[derive(Debug)]
pub struct AssetImportErr {
    pub message: String,
    pub exit_code: u8,
}

#[derive(Debug)]
pub struct AssetImportInput<'a> {
    pub id: &'a str,
    pub src: &'a str,
    pub kind: &'a str,
    pub source_label: &'a str,
}

#[derive(Debug)]
pub struct AssetImportOutcome {
    pub result: TxResult,
    pub produced: ProducedAsset,
    pub human: String,
    pub json_str: String,
    pub exit_code: u8,
}

pub fn import_run(
    doc_src: &str,
    input_bytes: &[u8],
    input: AssetImportInput<'_>,
) -> Result<AssetImportOutcome, AssetImportErr> {
    validate_asset_src(input.src)?;
    let kind = parse_kind(input.kind)?;
    let bytes = Arc::<[u8]>::from(input_bytes);
    let produced = FileImportProducer
        .produce(ProduceRequest::FileImport {
            kind,
            bytes,
            provenance: FileImportProvenance::new(input.source_label),
        })
        .map_err(|e| AssetImportErr {
            message: format!("error[asset.import]: {e}"),
            exit_code: 2,
        })?;

    finish_asset_run(doc_src, input, produced)
}

pub fn zpx_bake_run(
    doc_src: &str,
    manifest_src: &str,
    input: AssetImportInput<'_>,
) -> Result<AssetImportOutcome, AssetImportErr> {
    validate_asset_src(input.src)?;
    let zpx_doc = zenith_zpx::parse_manifest(manifest_src).map_err(|e| AssetImportErr {
        message: format!("error[asset.zpx_bake]: {e}"),
        exit_code: 2,
    })?;
    let produced = ZpxBakeProducer
        .produce(ProduceRequest::ZpxBake { doc: zpx_doc })
        .map_err(|e| AssetImportErr {
            message: format!("error[asset.zpx_bake]: {e}"),
            exit_code: 2,
        })?;

    finish_asset_run(doc_src, input, produced)
}

fn finish_asset_run(
    doc_src: &str,
    input: AssetImportInput<'_>,
    produced: ProducedAsset,
) -> Result<AssetImportOutcome, AssetImportErr> {
    let doc = KdlAdapter
        .parse(doc_src.as_bytes())
        .map_err(|e| AssetImportErr {
            message: format!("error[parse.error]: {}", e.message),
            exit_code: 2,
        })?;
    let tx = Transaction {
        ops: vec![add_asset_op(
            input.id,
            produced.kind.kind_str(),
            input.src,
            &produced.sha256,
            &produced.provenance,
        )],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).map_err(|e| AssetImportErr {
        message: format!("error[tx.engine]: {}", e.message),
        exit_code: 2,
    })?;

    let exit_code = status_exit_code(&result.status);
    let human = render_human(&result, &input, &produced);
    let json_str = render_json(&result, &input, &produced);

    Ok(AssetImportOutcome {
        result,
        produced,
        human,
        json_str,
        exit_code,
    })
}

pub fn validate_asset_src(src: &str) -> Result<(), AssetImportErr> {
    if src.is_empty() {
        return Err(invalid_src(src));
    }
    let path = Path::new(src);
    let bytes = src.as_bytes();
    let is_absolute_unix = src.starts_with('/');
    let is_absolute_windows = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/');
    let is_url = src.contains("://");
    let has_traversal = src == ".."
        || src.starts_with("../")
        || src.starts_with("..\\")
        || src.contains("/../")
        || src.contains("/..\\")
        || src.contains("\\..\\")
        || src.contains("\\../")
        || src.ends_with("/..")
        || src.ends_with("\\..");

    if path.is_absolute() || is_absolute_unix || is_absolute_windows || is_url || has_traversal {
        return Err(invalid_src(src));
    }

    Ok(())
}

fn invalid_src(src: &str) -> AssetImportErr {
    AssetImportErr {
        message: format!(
            "error[asset.invalid_src]: src '{src}' is not a safe relative path; absolute paths, parent-traversal segments ('..'), and URLs are not allowed"
        ),
        exit_code: 2,
    }
}

fn parse_kind(kind: &str) -> Result<AssetKind, AssetImportErr> {
    match kind {
        "image" => Ok(AssetKind::Image),
        "svg" => Ok(AssetKind::Svg),
        "font" => Ok(AssetKind::Font),
        other => Err(AssetImportErr {
            message: format!(
                "error[asset.invalid_kind]: unknown kind '{other}'; recognized kinds are: image, svg, font"
            ),
            exit_code: 2,
        }),
    }
}

fn add_asset_op(id: &str, kind: &str, src: &str, sha256: &str, provenance: &Provenance) -> Op {
    Op::AddAsset {
        id: id.to_owned(),
        kind: kind.to_owned(),
        src: src.to_owned(),
        sha256: Some(sha256.to_owned()),
        producer_kind: Some(provenance.kind_str().to_owned()),
        producer_source: Some(provenance.source_str().to_owned()),
        ai_prompt: None,
        ai_model: None,
        ai_provider: None,
        ai_seed: None,
        ai_generation_date: None,
        ai_license: None,
        ai_source_rights: None,
        ai_safety_status: None,
        ai_reuse_policy: None,
    }
}

fn render_human(
    result: &TxResult,
    input: &AssetImportInput<'_>,
    produced: &ProducedAsset,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("status: {}\n", status_label(&result.status)));
    out.push_str(&format!("asset: {}\n", input.id));
    out.push_str(&format!("kind: {}\n", produced.kind.kind_str()));
    out.push_str(&format!("src: {}\n", input.src));
    out.push_str(&format!("sha256: {}\n", produced.sha256));
    out.push_str(&format!(
        "changed: {}\n",
        result.source_before != result.source_after
    ));

    if result.diagnostics.is_empty() {
        out.push_str("diagnostics: (none)");
    } else {
        out.push_str("diagnostics:");
        for d in &result.diagnostics {
            let sev = crate::json_types::severity_str(&d.severity);
            let subject = d
                .subject_id
                .as_deref()
                .map(|s| format!(" ({s})"))
                .unwrap_or_default();
            out.push_str(&format!("\n  {sev}[{}]{subject}: {}", d.code, d.message));
        }
    }

    out
}

fn render_json(
    result: &TxResult,
    input: &AssetImportInput<'_>,
    produced: &ProducedAsset,
) -> String {
    let out = AssetImportJson {
        schema: "zenith-asset-import-v1",
        status: status_json(&result.status),
        asset: AssetImportAssetJson {
            id: input.id,
            kind: produced.kind.kind_str(),
            src: input.src,
            sha256: &produced.sha256,
            source: input.source_label,
        },
        changed: result.source_before != result.source_after,
        affected: &result.affected_node_ids,
        diagnostics: result
            .diagnostics
            .iter()
            .map(DiagnosticJson::from)
            .collect(),
    };
    serialize_pretty(&out)
}

fn status_exit_code(status: &TxStatus) -> u8 {
    match status {
        TxStatus::Accepted | TxStatus::AcceptedWithWarnings => 0,
        TxStatus::Rejected => 1,
    }
}

fn status_label(status: &TxStatus) -> &'static str {
    match status {
        TxStatus::Accepted => "accepted",
        TxStatus::AcceptedWithWarnings => "accepted (with warnings)",
        TxStatus::Rejected => "rejected",
    }
}

fn status_json(status: &TxStatus) -> &'static str {
    match status {
        TxStatus::Accepted => "accepted",
        TxStatus::AcceptedWithWarnings => "accepted_with_warnings",
        TxStatus::Rejected => "rejected",
    }
}

#[derive(Debug, Serialize)]
struct AssetImportJson<'a> {
    schema: &'static str,
    status: &'static str,
    asset: AssetImportAssetJson<'a>,
    changed: bool,
    affected: &'a [String],
    diagnostics: Vec<DiagnosticJson>,
}

#[derive(Debug, Serialize)]
struct AssetImportAssetJson<'a> {
    id: &'a str,
    kind: &'a str,
    src: &'a str,
    sha256: &'a str,
    source: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = r#"zenith version=1 {
  project id="proj.asset" name="Asset Test"
  tokens format="zenith-token-v1" { }
  styles { }
  assets { }
  document id="doc.asset" title="Asset Test" {
    page id="page.main" w=(px)100 h=(px)100 { }
  }
}"#;

    #[test]
    fn asset_import_dry_run_builds_add_asset_transaction() {
        let outcome = import_run(
            DOC,
            b"asset bytes",
            AssetImportInput {
                id: "asset.logo",
                src: "assets/logo.svg",
                kind: "svg",
                source_label: "logo.svg",
            },
        )
        .expect("import should run");

        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.result.source_after.contains(r#"id="asset.logo""#));
        assert!(outcome.result.source_after.contains(r#"kind="svg""#));
        assert!(
            outcome
                .result
                .source_after
                .contains(r#"src="assets/logo.svg""#)
        );
        assert!(outcome.result.source_after.contains("sha256="));
        assert!(outcome.json_str.contains("zenith-asset-import-v1"));
    }

    #[test]
    fn asset_import_rejects_parent_traversal_src() {
        let err = import_run(
            DOC,
            b"asset bytes",
            AssetImportInput {
                id: "asset.logo",
                src: "../logo.svg",
                kind: "svg",
                source_label: "logo.svg",
            },
        )
        .expect_err("unsafe src should fail before tx");

        assert_eq!(err.exit_code, 2);
        assert!(err.message.contains("asset.invalid_src"));
    }

    #[test]
    fn asset_zpx_bake_builds_image_add_asset_transaction() {
        let manifest = r##"zpx version=1 {
    canvas width=4 height=4 color-space="srgb" alpha="premultiplied"
    layers {
        layer id="paint" blend="normal" opacity=1.0 visible=#true clipping=#false {
            source kind="program" {
                stroke color="#ff0000ff" opacity=1.0 blend="normal" seed=1 {
                    brush kind="round" radius=3.0 hardness=1.0 spacing=1.0
                    sample x=2.0 y=2.0 pressure=1.0
                }
            }
        }
    }
}"##;

        let outcome = zpx_bake_run(
            DOC,
            manifest,
            AssetImportInput {
                id: "asset.paint",
                src: "assets/paint.png",
                kind: "image",
                source_label: "paint.zpx",
            },
        )
        .expect("zpx bake should run");

        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.produced.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(outcome.result.source_after.contains(r#"id="asset.paint""#));
        assert!(outcome.result.source_after.contains(r#"kind="image""#));
        assert!(
            outcome
                .result
                .source_after
                .contains(r#"src="assets/paint.png""#)
        );
        assert!(
            outcome
                .result
                .source_after
                .contains(&outcome.produced.sha256)
        );
    }
}
