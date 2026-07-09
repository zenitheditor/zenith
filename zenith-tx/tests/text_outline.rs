mod common;

use common::parse;
use zenith_core::default_provider;
use zenith_scene::collect_text_outline_paths;
use zenith_tx::{
    TextOutlineRequest, TxStatus, apply_text_outline_paths, check_text_outline_source,
    reject_text_outline,
};

fn materialize(doc: &zenith_core::Document, node: &str, id_prefix: &str) -> zenith_tx::TxResult {
    // Validate before compile — wrong-kind / missing id must not pay compile cost
    // and must not stack conversion diagnostics.
    if let Err(diags) = check_text_outline_source(doc, node) {
        return reject_text_outline(doc, diags).expect("reject should return a tx result");
    }
    let fonts = default_provider();
    let (paths, diags) = collect_text_outline_paths(doc, &fonts, node, id_prefix);
    apply_text_outline_paths(
        doc,
        &TextOutlineRequest {
            node: node.to_owned(),
        },
        paths,
        diags,
    )
    .expect("materialization should return a tx result")
}

fn assert_before(haystack: &str, first: &str, second: &str) {
    let first_index = haystack.find(first).expect("first substring should exist");
    let second_index = haystack
        .find(second)
        .expect("second substring should exist");
    assert!(
        first_index < second_index,
        "{first:?} should appear before {second:?}"
    );
}

const TEXT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#112233"
    token id="size.text" type="dimension" value=(px)32
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      text id="label" x=(px)10 y=(px)40 w=(px)200 h=(px)60 fill=(token)"color.ink" font-size=(token)"size.text" {
        span "Hi"
      }
      rect id="after" x=(px)0 y=(px)0 w=(px)10 h=(px)10
    }
  }
}"##;

const RECT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="box" x=(px)0 y=(px)0 w=(px)10 h=(px)10
    }
  }
}"##;

const SPACE_TEXT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      text id="blank" x=(px)10 y=(px)40 font-size=(px)32 {
        span " "
      }
    }
  }
}"##;

#[test]
fn materializes_text_glyph_runs_as_path_siblings() {
    let doc = parse(TEXT_DOC);
    let result = materialize(&doc, "label", "label.outline");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    assert_eq!(result.affected_node_ids, vec!["label.outline-0"]);
    assert!(result.source_after.contains("path id=\"label.outline-0\""));
    assert!(result.source_after.contains("subpath closed=#true"));
    assert!(result.source_after.contains("fill=(token)\"color.ink\""));
    assert_before(
        &result.source_after,
        "text id=\"label\"",
        "path id=\"label.outline-0\"",
    );
    assert_before(
        &result.source_after,
        "path id=\"label.outline-0\"",
        "rect id=\"after\"",
    );
}

#[test]
fn materialization_rejects_non_text_source_nodes() {
    let doc = parse(RECT_DOC);
    let result = materialize(&doc, "box", "box.outline.");

    assert_eq!(result.status, TxStatus::Rejected);
    assert_eq!(result.source_after, result.source_before);
    assert!(result.affected_node_ids.is_empty());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "tx.unsupported_property")
    );
    // Must not stack empty-outline or conversion failures after a kind reject.
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.no_text_outlines" || d.code == "scene.text_outline_failed")
    );
}

#[test]
fn materialization_rejects_when_text_has_no_outlined_glyphs() {
    let doc = parse(SPACE_TEXT_DOC);
    let result = materialize(&doc, "blank", "blank.outline.");

    assert_eq!(result.status, TxStatus::Rejected);
    assert_eq!(result.source_after, result.source_before);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "tx.no_text_outlines")
    );
}
