//! The optional `library.kdl` manifest that sits beside a directory of SVGs.
//!
//! An SVG library needs no manifest: the icons ARE the `*.svg` files, and their
//! ids are the file stems. A manifest only adds what a filename cannot carry —
//! the pack's identity (id, version, license, upstream revision) and per-icon
//! SEARCH METADATA:
//!
//! ```kdl
//! library id="@zenith/icons-lucide" version="1.23.0" {
//!   license "ISC AND MIT"
//!   license-file "NOTICE"
//!   source "https://github.com/lucide-icons/lucide"
//!   revision "c67c9bdbfb43b0ecd69b52d37aeb4ab2d5386271"
//!   icon "house" aliases="home" tags="living residence" categories="buildings"
//! }
//! ```
//!
//! `aliases` are alternate NAMES, ranked with near-id authority — `home` finds
//! `house` because the icon could have been called that. `tags` are merely
//! RELATED words, ranked below names. `categories` are a
//! closed vocabulary used to FILTER (`library search arrow --category
//! navigation`); they are deliberately never matched as free text, because a
//! category like `shapes` applies to hundreds of icons and would swamp the
//! ranking.
//!
//! An `icon` entry is purely additive: an icon with no entry still exists, it is
//! simply searchable by its name alone. An entry naming an icon that has no
//! `.svg` file is ignored.

use std::collections::BTreeMap;

use kdl::{KdlDocument, KdlNode};

/// Search metadata attached to one icon by the manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IconMeta {
    /// Alternate NAMES for the icon (`home` for `house`). Ranked with near-id
    /// authority: an alias is what the icon could have been called.
    pub aliases: Vec<String>,
    /// Related words (`living`, `residence`). Ranked below names.
    pub tags: Vec<String>,
    /// Closed-vocabulary categories used to filter, never to rank.
    pub categories: Vec<String>,
}

/// A parsed `library.kdl`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    /// The pack id, e.g. `@zenith/icons-lucide`.
    pub id: Option<String>,
    /// The pack version.
    pub version: Option<String>,
    /// SPDX-style license expression, e.g. `ISC AND MIT`.
    pub license: Option<String>,
    /// Directory-relative file holding the full license text.
    pub license_file: Option<String>,
    /// Upstream project URL the icons were vendored from.
    pub source_url: Option<String>,
    /// Exact upstream revision (commit sha) the icons were vendored at.
    pub revision: Option<String>,
    /// Per-icon search metadata, keyed by icon id (the `.svg` file stem).
    pub icons: BTreeMap<String, IconMeta>,
}

/// Read a node's first positional argument as a string, e.g. `license "ISC"`.
fn positional_str(node: &KdlNode) -> Option<String> {
    node.get(0).and_then(|v| v.as_string()).map(str::to_owned)
}

/// Read a named string property, e.g. the `tags` of `icon "x" tags="a b"`.
fn prop_str(node: &KdlNode, key: &str) -> Option<String> {
    node.get(key).and_then(|v| v.as_string()).map(str::to_owned)
}

/// Split a whitespace-separated property value into normalized, de-duplicated
/// lowercase terms, preserving first-seen order.
fn terms(raw: Option<String>) -> Vec<String> {
    let mut seen = Vec::new();
    let Some(raw) = raw else {
        return seen;
    };
    for term in raw.split_whitespace() {
        let term = term.to_lowercase();
        if !term.is_empty() && !seen.contains(&term) {
            seen.push(term);
        }
    }
    seen
}

/// Parse a `library.kdl` manifest.
///
/// The document must contain exactly one top-level `library` node; its children
/// supply the pack metadata and the `icon` entries. Unrecognized child nodes are
/// ignored, so a manifest can carry annotations this version does not read.
///
/// # Errors
///
/// Returns a message when the text is not valid KDL, or when it has no
/// top-level `library` node.
pub fn parse_manifest(text: &str) -> Result<Manifest, String> {
    let doc = KdlDocument::parse(text).map_err(|e| format!("invalid KDL: {e}"))?;
    let library = doc
        .nodes()
        .iter()
        .find(|n| n.name().value() == "library")
        .ok_or_else(|| "manifest has no top-level `library` node".to_owned())?;

    let mut manifest = Manifest {
        id: prop_str(library, "id"),
        version: prop_str(library, "version"),
        ..Manifest::default()
    };

    let Some(children) = library.children() else {
        return Ok(manifest);
    };

    for node in children.nodes() {
        match node.name().value() {
            "license" => manifest.license = positional_str(node),
            "license-file" => manifest.license_file = positional_str(node),
            "source" => manifest.source_url = positional_str(node),
            "revision" => manifest.revision = positional_str(node),
            "icon" => {
                let Some(name) = positional_str(node) else {
                    continue;
                };
                manifest.icons.insert(
                    name,
                    IconMeta {
                        aliases: terms(prop_str(node, "aliases")),
                        tags: terms(prop_str(node, "tags")),
                        categories: terms(prop_str(node, "categories")),
                    },
                );
            }
            // Unknown annotations are tolerated, not an error.
            _ => {}
        }
    }

    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
library id="@acme/icons" version="2.1.0" {
  license "MIT"
  license-file "NOTICE"
  source "https://example.invalid/icons"
  revision "abc123"
  icon "house" aliases="home" tags="  Living living" categories="buildings navigation"
  icon "plain"
}
"#;

    #[test]
    fn parses_identity_and_provenance() {
        let m = parse_manifest(SAMPLE).expect("parses");
        assert_eq!(m.id.as_deref(), Some("@acme/icons"));
        assert_eq!(m.version.as_deref(), Some("2.1.0"));
        assert_eq!(m.license.as_deref(), Some("MIT"));
        assert_eq!(m.license_file.as_deref(), Some("NOTICE"));
        assert_eq!(
            m.source_url.as_deref(),
            Some("https://example.invalid/icons")
        );
        assert_eq!(m.revision.as_deref(), Some("abc123"));
    }

    #[test]
    fn terms_are_lowercased_deduped_and_ordered() {
        let m = parse_manifest(SAMPLE).expect("parses");
        let house = &m.icons["house"];
        assert_eq!(house.aliases, ["home"]);
        assert_eq!(house.tags, ["living"]);
        assert_eq!(house.categories, ["buildings", "navigation"]);
    }

    #[test]
    fn icon_entry_without_metadata_is_still_recorded() {
        let m = parse_manifest(SAMPLE).expect("parses");
        assert_eq!(m.icons["plain"], IconMeta::default());
    }

    #[test]
    fn unknown_child_nodes_are_tolerated() {
        let m = parse_manifest("library id=\"x\" { future-thing \"y\" }").expect("parses");
        assert_eq!(m.id.as_deref(), Some("x"));
    }

    #[test]
    fn missing_library_node_is_an_error() {
        assert!(parse_manifest("something id=\"x\"").is_err());
    }

    #[test]
    fn invalid_kdl_is_an_error() {
        assert!(parse_manifest("library id=\"unterminated").is_err());
    }

    #[test]
    fn manifest_is_optional_shaped() {
        // A bare `library` node with no children yields identity only.
        let m = parse_manifest("library id=\"@a/b\"").expect("parses");
        assert_eq!(m.id.as_deref(), Some("@a/b"));
        assert!(m.icons.is_empty());
        assert!(m.license.is_none());
    }
}
