//! Capture of unknown child KDL nodes inside structural blocks.
//!
//! The transforms for non-renderable blocks (`brand`, `assets`, `variants`, …)
//! and for the structured sub-children of renderable nodes (`row`, `subpath`,
//! `override`, …) read a fixed set of child node names and drop everything
//! else. This module is a pure classification pass over the raw KDL that
//! records each dropped child in the shared [`UnsupportedChild`] side table,
//! tagged with the block's allowed child names. Validation turns each record
//! into a `block.unknown_child` Error with a did-you-mean.
//!
//! Direct children of a renderable node are NOT classified here: the sibling
//! `unsupported` module owns them (`node.unsupported_child`).
//!
//! Each structural block is identified by a `/`-joined path of child names.
//! The document root is `(zenith)`. A renderable node kind is the root of its
//! own substructure (`table`, `table/row`, …).

use kdl::{KdlDocument, KdlNode};

use crate::ast::{ChildSite, TokenType, UnsupportedChild};
use crate::error::{ParseError, ParseErrorCode};
use crate::suggest::unknown_child_message;

use super::helpers::{node_span, optional_string_prop};

/// Root path of the `zenith` document node.
const DOC_ROOT: &str = "(zenith)";

/// Top-level children of the `zenith` node.
const ZENITH_CHILDREN: &[&str] = &[
    "project",
    "assets",
    "libraries",
    "imports",
    "actions",
    "tokens",
    "styles",
    "components",
    "masters",
    "sections",
    "provenance",
    "variants",
    "recipes",
    "diagnostics",
    "brand",
    "document",
];

/// Filter-op child names of a `filter` token (see `FilterKind::from_op_name`).
const FILTER_OPS: &[&str] = &[
    "grayscale",
    "invert",
    "sepia",
    "saturate",
    "brightness",
    "contrast",
    "hue-rotate",
    "duotone",
    "noise",
];

/// Shape child names of a `mask` token (see `MaskShape::from_shape_name`).
const MASK_SHAPES: &[&str] = &["rect", "rounded", "ellipse"];

/// Top-level node names a standalone config file accepts.
const CONFIG_ROOTS: &[&str] = &["diagnostics", "brand"];

/// How a structural block treats its child nodes.
#[derive(Clone, Copy)]
enum Spec {
    /// Exactly these child names are accepted; any other child is flagged.
    Only(&'static [&'static str]),
    /// Children are renderable nodes plus structural extras that have their
    /// own specs. Nothing is flagged here; renderable nodes are classified by
    /// the node transforms.
    Nodes,
}

/// A block that accepts no child nodes.
const LEAF: Spec = Spec::Only(&[]);

/// Return the child spec for the block at `path`, or `None` when the path is
/// not a structural block this pass classifies.
fn spec_for(node: &KdlNode, path: &str) -> Option<Spec> {
    let spec = match path {
        DOC_ROOT => Spec::Only(ZENITH_CHILDREN),
        "(zenith)/project" => Spec::Only(&["author"]),
        "(zenith)/assets" => Spec::Only(&["asset"]),
        "(zenith)/libraries" => Spec::Only(&["library"]),
        "(zenith)/imports" => Spec::Only(&["import"]),
        "(zenith)/imports/import" => Spec::Only(&["token-map"]),
        "(zenith)/actions" => Spec::Only(&["action"]),
        "(zenith)/actions/action" => Spec::Only(&["tx"]),
        "(zenith)/styles" => Spec::Only(&["style"]),
        "(zenith)/tokens" => Spec::Only(&["token"]),
        "(zenith)/tokens/token" => token_spec(node),
        "(zenith)/components" => Spec::Only(&["component"]),
        "(zenith)/components/component" => Spec::Nodes,
        "(zenith)/components/component/ports" => Spec::Only(&["port"]),
        "(zenith)/masters" => Spec::Only(&["master"]),
        "(zenith)/sections" => Spec::Only(&["section"]),
        "(zenith)/provenance" => Spec::Only(&["origin"]),
        "(zenith)/variants" => Spec::Only(&["variant"]),
        "(zenith)/variants/variant" => Spec::Only(&["override"]),
        "(zenith)/recipes" => Spec::Only(&["recipe"]),
        "(zenith)/recipes/recipe" => Spec::Only(&["param", "palette", "expanded"]),
        "(zenith)/diagnostics" => Spec::Only(&["allow", "deny", "warn"]),
        "(zenith)/brand" => Spec::Only(&["colors", "fonts", "weights"]),
        "(zenith)/document" => Spec::Only(&["block", "page"]),
        "(zenith)/document/page" => Spec::Nodes,
        "(zenith)/document/page/construction" => Spec::Only(&["guide"]),
        "(zenith)/document/page/ports" => Spec::Only(&["port"]),
        // Leaf declarations inside document-level blocks.
        "(zenith)/project/author"
        | "(zenith)/assets/asset"
        | "(zenith)/libraries/library"
        | "(zenith)/imports/import/token-map"
        | "(zenith)/actions/action/tx"
        | "(zenith)/components/component/ports/port"
        | "(zenith)/sections/section"
        | "(zenith)/provenance/origin"
        | "(zenith)/variants/variant/override"
        | "(zenith)/recipes/recipe/param"
        | "(zenith)/recipes/recipe/palette"
        | "(zenith)/recipes/recipe/expanded"
        | "(zenith)/diagnostics/allow"
        | "(zenith)/diagnostics/deny"
        | "(zenith)/diagnostics/warn"
        | "(zenith)/brand/colors"
        | "(zenith)/brand/fonts"
        | "(zenith)/brand/weights"
        | "(zenith)/document/block"
        | "(zenith)/document/page/safe-zone"
        | "(zenith)/document/page/fold"
        | "(zenith)/document/page/block"
        | "(zenith)/document/page/ports/port"
        | "(zenith)/document/page/construction/guide" => LEAF,
        // Structured sub-children of renderable nodes.
        "path/subpath" => Spec::Only(&["anchor"]),
        "table/row" => Spec::Only(&["cell"]),
        "instance/override" => Spec::Only(&["span"]),
        "text/block"
        | "text/kern-pair"
        | "text/span"
        | "code/content"
        | "code/kern-pair"
        | "shape/span"
        | "connector/span"
        | "footnote/span"
        | "polygon/point"
        | "polyline/point"
        | "path/anchor"
        | "path/subpath/anchor"
        | "table/column"
        | "chart/series"
        | "chart/categories"
        | "chart/label-colors"
        | "chart/slice-colors"
        | "instance/override/span" => LEAF,
        // Children of a token's own children (`stop`, `layer`, filter ops, …).
        _ if path.starts_with("(zenith)/tokens/token/") => LEAF,
        _ => return None,
    };
    Some(spec)
}

/// Child spec of a `token` block: the accepted child names depend on its type.
fn token_spec(node: &KdlNode) -> Spec {
    let token_type = TokenType::from_type_name(optional_string_prop(node, "type").unwrap_or(""));
    match token_type {
        TokenType::Gradient => Spec::Only(&["stop"]),
        TokenType::Shadow => Spec::Only(&["layer"]),
        TokenType::Filter => Spec::Only(FILTER_OPS),
        TokenType::Mask => Spec::Only(MASK_SHAPES),
        TokenType::Color
        | TokenType::Dimension
        | TokenType::Number
        | TokenType::FontFamily
        | TokenType::FontWeight
        | TokenType::Unknown(_) => LEAF,
    }
}

/// Walk the children of `node` (at `path`), flagging unknown children and
/// descending into every accepted child that is itself a structural block.
fn walk(node: &KdlNode, path: &str, sink: &mut Vec<UnsupportedChild>) {
    let Some(children) = node.children() else {
        return;
    };
    let spec = spec_for(node, path);
    let is_mask = path == "(zenith)/tokens/token"
        && matches!(
            TokenType::from_type_name(optional_string_prop(node, "type").unwrap_or("")),
            TokenType::Mask
        );
    let mut shapes_seen = 0usize;
    for child in children.nodes() {
        let name = child.name().value();
        if let Some(Spec::Only(allowed)) = spec
            && !allowed.contains(&name)
        {
            sink.push(UnsupportedChild {
                parent_id: optional_string_prop(node, "id").map(str::to_owned),
                parent_kind: parent_label(path).to_owned(),
                child_kind: name.to_owned(),
                source_span: node_span(child),
                site: ChildSite::Block,
                allowed,
            });
            continue;
        }
        if is_mask {
            // Only the first shape child is read; each later one is dropped.
            shapes_seen += 1;
            if shapes_seen > 1 {
                sink.push(UnsupportedChild {
                    parent_id: optional_string_prop(node, "id").map(str::to_owned),
                    parent_kind: parent_label(path).to_owned(),
                    child_kind: name.to_owned(),
                    source_span: node_span(child),
                    site: ChildSite::ExtraMaskShape,
                    allowed: MASK_SHAPES,
                });
            }
            continue;
        }
        let child_path = format!("{path}/{name}");
        if spec_for(child, &child_path).is_some() {
            walk(child, &child_path, sink);
        }
    }
}

/// Last path segment, with the document-root parentheses removed.
fn parent_label(path: &str) -> &str {
    let last = path.rsplit('/').next().unwrap_or(path);
    last.trim_start_matches('(').trim_end_matches(')')
}

/// Record every unknown child inside the `zenith` document's structural blocks.
pub(super) fn collect_unknown_document_children(
    zenith_node: &KdlNode,
    sink: &mut Vec<UnsupportedChild>,
) {
    walk(zenith_node, DOC_ROOT, sink);
}

/// Record every unknown child inside the structural sub-children of the
/// renderable `node` (for example the `cell` list of a table `row`).
///
/// The direct children of `node` are not classified here.
pub(super) fn collect_unknown_substructure(node: &KdlNode, sink: &mut Vec<UnsupportedChild>) {
    walk(node, node.name().value(), sink);
}

/// Reject unknown content in a standalone config document.
///
/// A config file accepts the top-level `diagnostics` and `brand` blocks only.
/// Both config parsers call this, so any unknown top-level node or unknown
/// child inside either block is a hard [`ParseError`] with a did-you-mean.
pub(crate) fn check_config_document(doc: &KdlDocument) -> Result<(), ParseError> {
    for node in doc.nodes() {
        let name = node.name().value();
        if !CONFIG_ROOTS.contains(&name) {
            let message = unknown_child_message("config file", name, CONFIG_ROOTS);
            return Err(match node_span(node) {
                Some(span) => ParseError::with_span(ParseErrorCode::UnexpectedNode, span, message),
                None => ParseError::spanless(ParseErrorCode::UnexpectedNode, message),
            });
        }
        let mut found: Vec<UnsupportedChild> = Vec::new();
        walk(node, &format!("{DOC_ROOT}/{name}"), &mut found);
        if let Some(entry) = found.into_iter().next() {
            let message =
                unknown_child_message(&entry.parent_kind, &entry.child_kind, entry.allowed);
            return Err(match entry.source_span {
                Some(span) => ParseError::with_span(ParseErrorCode::UnexpectedNode, span, message),
                None => ParseError::spanless(ParseErrorCode::UnexpectedNode, message),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{FilterKind, MaskShape};

    fn flagged(src: &str) -> Vec<(String, String)> {
        let doc: KdlDocument = src.parse().expect("kdl");
        let zenith = doc.nodes().first().expect("zenith node");
        let mut sink = Vec::new();
        collect_unknown_document_children(zenith, &mut sink);
        sink.into_iter()
            .map(|c| (c.parent_kind, c.child_kind))
            .collect()
    }

    #[test]
    fn filter_ops_match_ast_names() {
        for name in FILTER_OPS {
            assert!(FilterKind::from_op_name(name).is_some(), "{name}");
        }
        for name in MASK_SHAPES {
            assert!(MaskShape::from_shape_name(name).is_some(), "{name}");
        }
    }

    #[test]
    fn flags_unknown_top_level_and_block_children() {
        let got = flagged(
            r##"zenith version=1 {
                brand { colors "#fff"; palette "x" }
                sections { section id="s" name="n" start-page="p" { extra } ; other }
                bogus
            }"##,
        );
        assert!(got.contains(&("brand".into(), "palette".into())), "{got:?}");
        assert!(
            got.contains(&("sections".into(), "other".into())),
            "{got:?}"
        );
        assert!(got.contains(&("section".into(), "extra".into())), "{got:?}");
        assert!(got.contains(&("zenith".into(), "bogus".into())), "{got:?}");
    }

    #[test]
    fn accepts_well_formed_blocks() {
        let got = flagged(
            r##"zenith version=1 {
                brand { colors "#fff"; fonts "A"; weights 400 }
                tokens format="x" {
                    token id="g" type="gradient" { stop color=(token)"c" offset=0.0 }
                    token id="c" type="color" value="#fff"
                }
                document id="d" { page id="p" { construction { guide id="g1" } } }
            }"##,
        );
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn token_children_depend_on_type() {
        let got = flagged(
            r##"zenith version=1 {
                tokens format="x" {
                    token id="c" type="color" value="#fff" { stop }
                    token id="f" type="filter" { sepia; stop }
                }
            }"##,
        );
        assert_eq!(
            got,
            vec![
                ("token".to_owned(), "stop".to_owned()),
                ("token".to_owned(), "stop".to_owned())
            ]
        );
    }

    #[test]
    fn mask_extra_shapes_are_flagged() {
        let doc: KdlDocument = r##"zenith version=1 {
                tokens format="x" {
                    token id="m" type="mask" { rect; ellipse; rounded }
                }
            }"##
        .parse()
        .expect("kdl");
        let zenith = doc.nodes().first().expect("zenith node");
        let mut sink = Vec::new();
        collect_unknown_document_children(zenith, &mut sink);
        let extra: Vec<&str> = sink
            .iter()
            .filter(|c| c.site == ChildSite::ExtraMaskShape)
            .map(|c| c.child_kind.as_str())
            .collect();
        assert_eq!(extra, vec!["ellipse", "rounded"]);
    }

    #[test]
    fn substructure_flags_row_children() {
        let doc: KdlDocument = r#"table id="t" { row { cell; bogus } }"#.parse().expect("kdl");
        let table = doc.nodes().first().expect("table");
        let mut sink = Vec::new();
        collect_unknown_substructure(table, &mut sink);
        assert_eq!(sink.len(), 1);
        assert_eq!(sink[0].parent_kind, "row");
        assert_eq!(sink[0].child_kind, "bogus");
    }
}
