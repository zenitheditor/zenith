//! `token.unused` counts tokens that only the `defaults` lowering references
//! (content pairing, default styles).

use zenith_core::{Diagnostic, KdlAdapter, KdlSource, validate};

fn doc_src(defaults: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="p" name="P"
  tokens format="zenith-token-v1" {{
    token id="color.primary" type="color" value="#605dff"
    token id="color.primary.content" type="color" value="#ffffff"
    token id="color.orphan" type="color" value="#123456"
  }}
  styles {{
  }}
  document id="doc" {{
    page id="p1" w=(px)400 h=(px)300 background=(token)"color.primary" {{
      {defaults}
      text id="t" x=(px)0 y=(px)0 w=(px)100 h=(px)20 {{ span "Hi" }}
    }}
  }}
}}
"##
    )
}

fn unused(source: &str) -> Vec<String> {
    let doc = KdlAdapter.parse(source.as_bytes()).expect("parse");
    validate(&doc)
        .diagnostics
        .into_iter()
        .filter(|d: &Diagnostic| d.code == "token.unused")
        .filter_map(|d| d.subject_id)
        .collect()
}

#[test]
fn content_token_used_only_by_pairing_is_not_unused() {
    let ids = unused(&doc_src("defaults {}"));
    assert!(
        !ids.contains(&"color.primary.content".to_owned()),
        "{ids:?}"
    );
    assert!(ids.contains(&"color.orphan".to_owned()), "{ids:?}");
}

#[test]
fn content_token_is_unused_without_defaults() {
    let ids = unused(&doc_src(""));
    assert!(ids.contains(&"color.primary.content".to_owned()), "{ids:?}");
}
