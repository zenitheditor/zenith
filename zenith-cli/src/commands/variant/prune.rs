//! Drop the tokens that variant generation leaves without a reference.
//!
//! A generated variant can orphan a token in two ways:
//!
//! - An override replaces the only reference, such as a title `font-size`.
//! - Only a sibling variant references the token, such as a story title size.
//!
//! `zenith validate` then reports `token.unused` on the generated `.zen`. The
//! engine removes each such token. A token the source document already
//! leaves unused stays, so its `token.unused` report stays too.

use std::collections::BTreeSet;

use zenith_core::{Document, validate};

/// The ids of the tokens `doc` declares and never references.
///
/// The source document counts the token refs of its `variants` block as
/// references.
pub(super) fn unused_token_ids(doc: &Document) -> BTreeSet<String> {
    validate(doc)
        .diagnostics
        .into_iter()
        .filter(|d| d.code == "token.unused")
        .filter_map(|d| d.subject_id)
        .collect()
}

/// Remove from `doc` each unreferenced token that is not in `source_unused`.
pub(super) fn prune_orphaned_tokens(doc: &mut Document, source_unused: &BTreeSet<String>) {
    let orphaned: BTreeSet<String> = unused_token_ids(doc)
        .into_iter()
        .filter(|id| !source_unused.contains(id))
        .collect();
    if orphaned.is_empty() {
        return;
    }
    doc.tokens.tokens.retain(|t| !orphaned.contains(&t.id));
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{KdlAdapter, KdlSource, Node, PropertyValue};

    const SRC: &str = r##"zenith version=1 {
  project id="p" name="P"
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#111111"
    token id="color.spare" type="color" value="#222222"
    token id="size.base" type="dimension" value=(px)16
    token id="size.a" type="dimension" value=(px)20
    token id="size.b" type="dimension" value=(px)40
  }
  styles {}
  variants {
    variant id="a" source="pg" w=(px)200 h=(px)100 {
      override node="t" font-size=(token)"size.a"
    }
    variant id="b" source="pg" w=(px)100 h=(px)200 {
      override node="t" font-size=(token)"size.b"
    }
  }
  document id="d" title="D" {
    page id="pg" w=(px)100 h=(px)100 {
      text id="t" x=(px)0 y=(px)0 w=(px)100 h=(px)40 fill=(token)"color.ink" font-size=(token)"size.base" { span "a" }
    }
  }
}
"##;

    fn token_ids(doc: &Document) -> Vec<&str> {
        doc.tokens.tokens.iter().map(|t| t.id.as_str()).collect()
    }

    #[test]
    fn source_counts_variant_token_refs() {
        let doc = KdlAdapter.parse(SRC.as_bytes()).expect("fixture parses");
        let unused: Vec<String> = unused_token_ids(&doc).into_iter().collect();
        assert_eq!(unused, ["color.spare"]);
    }

    #[test]
    fn removes_replaced_and_sibling_tokens_only() {
        let doc = KdlAdapter.parse(SRC.as_bytes()).expect("fixture parses");
        let source_unused = unused_token_ids(&doc);
        // Variant `a` as generated: its override replaced `size.base`.
        let mut generated = doc.clone();
        generated.variants.clear();
        if let Some(Node::Text(t)) = generated.body.pages[0].children.first_mut() {
            t.font_size = Some(PropertyValue::TokenRef("size.a".to_owned()));
        }
        prune_orphaned_tokens(&mut generated, &source_unused);
        // `color.spare` was unused in the source, so it stays.
        assert_eq!(
            token_ids(&generated),
            ["color.ink", "color.spare", "size.a"]
        );
    }

    #[test]
    fn unchanged_document_keeps_every_token() {
        let doc = KdlAdapter.parse(SRC.as_bytes()).expect("fixture parses");
        let source_unused = unused_token_ids(&doc);
        let mut same = doc.clone();
        prune_orphaned_tokens(&mut same, &source_unused);
        assert_eq!(same, doc);
    }
}
