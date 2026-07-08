mod common;

use common::parse;
use zenith_core::default_provider;
use zenith_tx::{TextOutlineRequest, TxStatus, materialize_text_outlines};

fn assert_before(haystack: &str, first: &str, second: &str) {
    let first_index = haystack.find(first).expect("first substring should exist");
    let second_index = haystack.find(second).expect("second substring should exist");
    assert!(first_index < second_index, "{first:?} should appear before {second:?}");
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
    let fonts = default_provider();
    let result = materialize_text_outlines(
        &doc,
        &fonts,
        &TextOutlineRequest {
            node: "label".to_owned(),
            id_prefix: "label.outline".to_owned(),
        },
    )
    .expect("materialization should return a tx result");

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
    assert_before(&result.source_after, "text id=\"label\"", "path id=\"label.outline-0\"");
    assert_before(
        &result.source_after,
        "path id=\"label.outline-0\"",
        "rect id=\"after\"",
    );
}

#[test]
fn materialization_rejects_non_text_source_nodes() {
    let doc = parse(RECT_DOC);
    let fonts = default_provider();
    let result = materialize_text_outlines(
        &doc,
        &fonts,
        &TextOutlineRequest {
            node: "box".to_owned(),
            id_prefix: "box.outline.".to_owned(),
        },
    )
    .expect("materialization should return a tx result");

    assert_eq!(result.status, TxStatus::Rejected);
    assert_eq!(result.source_after, result.source_before);
    assert!(result.affected_node_ids.is_empty());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "tx.unsupported_property")
    );
}

#[test]
fn materialization_rejects_when_text_has_no_outlined_glyphs() {
    let doc = parse(SPACE_TEXT_DOC);
    let fonts = default_provider();
    let result = materialize_text_outlines(
        &doc,
        &fonts,
        &TextOutlineRequest {
            node: "blank".to_owned(),
            id_prefix: "blank.outline.".to_owned(),
        },
    )
    .expect("materialization should return a tx result");

    assert_eq!(result.status, TxStatus::Rejected);
    assert_eq!(result.source_after, result.source_before);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "tx.no_text_outlines")
    );
}
