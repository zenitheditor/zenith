//! `imports { … }` block: import declarations and their token maps.

use kdl::KdlNode;

use crate::ast::document::{ImportDecl, TokenMapDecl};
use crate::error::ParseError;
use crate::parse::transform::helpers::{
    collect_unknown_props, node_span, optional_string_prop, required_string_prop,
};

const IMPORT_KNOWN_PROPS: &[&str] = &["id", "kind", "src", "sha256"];
const TOKEN_MAP_KNOWN_PROPS: &[&str] = &["from", "to"];

pub(super) fn transform_imports(node: &KdlNode) -> Result<Vec<ImportDecl>, ParseError> {
    let mut imports = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() == "import" {
                imports.push(transform_import(child)?);
            }
        }
    }
    Ok(imports)
}

fn transform_import(node: &KdlNode) -> Result<ImportDecl, ParseError> {
    let mut token_maps = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() == "token-map" {
                token_maps.push(transform_token_map(child)?);
            }
        }
    }

    Ok(ImportDecl {
        id: required_string_prop(node, "id")?.to_owned(),
        kind: required_string_prop(node, "kind")?.to_owned(),
        src: required_string_prop(node, "src")?.to_owned(),
        sha256: optional_string_prop(node, "sha256").map(str::to_owned),
        token_maps,
        source_span: node_span(node),
        unknown_props: collect_unknown_props(node, IMPORT_KNOWN_PROPS),
    })
}

fn transform_token_map(node: &KdlNode) -> Result<TokenMapDecl, ParseError> {
    Ok(TokenMapDecl {
        from: required_string_prop(node, "from")?.to_owned(),
        to: required_string_prop(node, "to")?.to_owned(),
        source_span: node_span(node),
        unknown_props: collect_unknown_props(node, TOKEN_MAP_KNOWN_PROPS),
    })
}
