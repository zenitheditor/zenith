//! Validation tests for the `pattern` node: the pattern's own id is registered
//! (so a duplicate vs another node fires `id.duplicate`), a pattern alone
//! validates cleanly, and the motif is a TEMPLATE whose id is NOT collected —
//! so a motif id colliding with a real node does NOT error.

mod common;

use common::*;

/// A pattern whose id duplicates another node's id fires `id.duplicate` — this
/// proves the pattern's own id participates in id-uniqueness.
#[test]
fn pattern_duplicate_id_fires_id_duplicate() {
    let src = r##"zenith version=1 {
  project id="proj.dup" name="Dup"
  tokens format="zenith-token-v1" {
    token id="color.dot" type="color" value="#334155"
  }
  styles {
  }
  document id="doc.dup" title="Dup" {
    page id="page.dup" w=(px)800 h=(px)600 {
      rect id="node.dup" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.dot"
      pattern id="node.dup" kind="grid" x=(px)0 y=(px)0 w=(px)800 h=(px)600 fill=(token)"color.dot" {
        ellipse id="e.dot" w=(px)8 h=(px)8 fill=(token)"color.dot"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);
    assert!(
        has_code(&report, "id.duplicate"),
        "pattern id duplicate must fire id.duplicate; got: {:?}",
        codes(&report)
    );
}

/// A pattern node alone validates without crashing AND its motif's id is NOT
/// collected: a motif whose id collides with a real sibling node must NOT
/// produce `id.duplicate` (the motif is a template, invisible to id-collection).
#[test]
fn pattern_motif_id_not_collected() {
    let src = r##"zenith version=1 {
  project id="proj.motif" name="Motif"
  tokens format="zenith-token-v1" {
    token id="color.dot" type="color" value="#334155"
  }
  styles {
  }
  document id="doc.motif" title="Motif" {
    page id="page.motif" w=(px)800 h=(px)600 {
      rect id="shared.id" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.dot"
      pattern id="p.dots" kind="grid" x=(px)0 y=(px)0 w=(px)800 h=(px)600 fill=(token)"color.dot" {
        ellipse id="shared.id" w=(px)8 h=(px)8 fill=(token)"color.dot"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);
    // The motif shares an id with a real node, but the motif is a TEMPLATE and is
    // never id-collected — so NO id.duplicate is produced.
    assert!(
        !has_code(&report, "id.duplicate"),
        "motif id must NOT be collected (no id.duplicate); got: {:?}",
        codes(&report)
    );
    assert!(
        !report.has_errors(),
        "a clean pattern + motif must not error; got: {:?}",
        codes(&report)
    );
}
