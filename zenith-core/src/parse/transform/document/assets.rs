//! The document-level `assets { … }` block and asset declarations.

use kdl::KdlNode;

use crate::ast::asset::{AssetBlock, AssetDecl, AssetKind};
use crate::error::ParseError;
use crate::parse::transform::helpers::{
    collect_unknown_props, node_span, optional_i64_prop, optional_string_prop, required_string_prop,
};

/// Canonical set of property names recognised on an `asset` declaration node.
///
/// Used by `zenith-core::schema` to surface the authorable attribute list for
/// the `zenith schema asset` subcommand.
pub(crate) const ASSET_KNOWN_PROPS: &[&str] = &[
    "id",
    "kind",
    "src",
    "sha256",
    "producer-kind",
    "producer-source",
    "ai-prompt",
    "ai-model",
    "ai-provider",
    "ai-seed",
    "ai-generation-date",
    "ai-license",
    "ai-source-rights",
    "ai-safety-status",
    "ai-reuse-policy",
];

pub(super) fn transform_assets(node: &KdlNode) -> Result<AssetBlock, ParseError> {
    let source_span = node_span(node);
    let mut asset_list: Vec<AssetDecl> = Vec::new();

    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() == "asset" {
                asset_list.push(transform_asset_decl(child)?);
            }
            // Non-`asset` child nodes inside assets block are silently ignored
            // (forward-compat).
        }
    }

    Ok(AssetBlock {
        assets: asset_list,
        source_span,
    })
}

fn transform_asset_decl(node: &KdlNode) -> Result<AssetDecl, ParseError> {
    let id = required_string_prop(node, "id")?.to_owned();
    let kind_str = required_string_prop(node, "kind")?;
    let kind = AssetKind::from_kind_str(kind_str);
    let src = required_string_prop(node, "src")?.to_owned();
    let sha256 = optional_string_prop(node, "sha256").map(str::to_owned);
    let producer_kind = optional_string_prop(node, "producer-kind").map(str::to_owned);
    let producer_source = optional_string_prop(node, "producer-source").map(str::to_owned);
    let ai_prompt = optional_string_prop(node, "ai-prompt").map(str::to_owned);
    let ai_model = optional_string_prop(node, "ai-model").map(str::to_owned);
    let ai_provider = optional_string_prop(node, "ai-provider").map(str::to_owned);
    let ai_seed = optional_i64_prop(node, "ai-seed");
    let ai_generation_date = optional_string_prop(node, "ai-generation-date").map(str::to_owned);
    let ai_license = optional_string_prop(node, "ai-license").map(str::to_owned);
    let ai_source_rights = optional_string_prop(node, "ai-source-rights").map(str::to_owned);
    let ai_safety_status = optional_string_prop(node, "ai-safety-status").map(str::to_owned);
    let ai_reuse_policy = optional_string_prop(node, "ai-reuse-policy").map(str::to_owned);
    let unknown_props = collect_unknown_props(node, ASSET_KNOWN_PROPS);
    let source_span = node_span(node);

    Ok(AssetDecl {
        id,
        kind,
        src,
        sha256,
        producer_kind,
        producer_source,
        ai_prompt,
        ai_model,
        ai_provider,
        ai_seed,
        ai_generation_date,
        ai_license,
        ai_source_rights,
        ai_safety_status,
        ai_reuse_policy,
        source_span,
        unknown_props,
    })
}
