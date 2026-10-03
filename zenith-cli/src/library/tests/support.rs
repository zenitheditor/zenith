//! Shared test helpers for the `library` test tree.

use zenith_core::{Document, KdlAdapter, KdlSource, Node, validate};

/// The embedded `@zenith/filters` pack source, shared by the `registry` and
/// `token` test modules.
pub(super) const FILTERS_SRC: &str = include_str!("../../../assets/libraries/zenith-filters.zen");

/// A minimal target document with a single empty page `pg`.
pub(super) const TARGET_SRC: &str = r#"zenith version=1 {
  project id="proj.x" name="Target"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="d" title="x" {
    page id="pg" w=(px)800 h=(px)600 {}
  }
}
"#;

pub(super) fn parse_target() -> Document {
    KdlAdapter
        .parse(TARGET_SRC.as_bytes())
        .expect("target parses")
}

pub(super) fn hard_errors(doc: &Document) -> Vec<String> {
    validate(doc)
        .diagnostics
        .into_iter()
        .filter(|d| d.is_error())
        .map(|d| format!("{}: {}", d.code, d.message))
        .collect()
}

pub(super) fn first_page_instance_ids(doc: &Document) -> Vec<String> {
    doc.body.pages[0]
        .children
        .iter()
        .filter_map(|n| match n {
            Node::Instance(i) => Some(i.id.clone()),
            _ => None,
        })
        .collect()
}
