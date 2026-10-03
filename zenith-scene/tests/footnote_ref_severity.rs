//! An unresolved `footnote-ref` is a broken document reference, so compile
//! reports `footnote.unresolved_ref` at the catalogued Warning severity.

mod common;
use common::*;
use zenith_core::{Severity, default_provider};
use zenith_scene::compile;

#[test]
fn unresolved_footnote_ref_is_a_warning() {
    let src = r##"zenith version=1 {
  project id="proj.fnsev" name="FNSEV"
  tokens format="zenith-token-v1" {
  }
  styles {}
  document id="doc.fnsev" title="FNSEV" {
page id="page.fnsev" w=(px)600 h=(px)900 margin-inner=(px)60 margin-outer=(px)60 margin-top=(px)80 margin-bottom=(px)80 {
  text id="body" x=(px)60 y=(px)80 w=(px)480 h=(px)200 {
    span "Dangling reference" footnote-ref="fn.missing"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let found: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "footnote.unresolved_ref")
        .collect();
    assert_eq!(found.len(), 1, "got {:?}", result.diagnostics);
    assert_eq!(found[0].severity, Severity::Warning);
}
