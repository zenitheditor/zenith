//! The document-level `variants { … }` and `recipes { … }` blocks.

use kdl::KdlNode;

use crate::ast::recipe::{RecipeDef, RecipeParam};
use crate::ast::variant::{VariantDef, VariantOverride};
use crate::error::{ParseError, ParseErrorCode};
use crate::parse::transform::helpers::{
    collect_unknown_props, entry_to_dimension, entry_to_property_value, node_span,
    optional_bool_prop, optional_dimension_prop, optional_i64_prop, optional_string_prop,
    required_string_prop,
};

// ---------------------------------------------------------------------------
// Variants
// ---------------------------------------------------------------------------

const VARIANT_KNOWN_PROPS: &[&str] = &["id", "source", "w", "h"];
const VARIANT_OVERRIDE_KNOWN_PROPS: &[&str] =
    &["node", "visible", "x", "y", "w", "h", "fill", "text"];

/// Transform the document-level `variants { … }` block into a list of
/// [`VariantDef`]. Each `variant id="…" source="…" w=(px)N h=(px)N { … }` is
/// a block node; non-`variant` children inside the block are silently ignored
/// (forward-compat). Mirrors [`transform_provenance`](super::structure::transform_provenance).
pub(super) fn transform_variants(node: &KdlNode) -> Result<Vec<VariantDef>, ParseError> {
    let mut defs: Vec<VariantDef> = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() == "variant" {
                defs.push(transform_variant_def(child)?);
            }
        }
    }
    Ok(defs)
}

fn transform_variant_def(node: &KdlNode) -> Result<VariantDef, ParseError> {
    let id = required_string_prop(node, "id")?.to_owned();
    let source = required_string_prop(node, "source")?.to_owned();

    let w = node
        .entry("w")
        .ok_or_else(|| {
            ParseError::spanless(
                ParseErrorCode::InvalidPropertyValue,
                format!("variant `{id}` is missing required property `w`"),
            )
        })
        .and_then(|e| entry_to_dimension(e, "w"))?;

    let h = node
        .entry("h")
        .ok_or_else(|| {
            ParseError::spanless(
                ParseErrorCode::InvalidPropertyValue,
                format!("variant `{id}` is missing required property `h`"),
            )
        })
        .and_then(|e| entry_to_dimension(e, "h"))?;

    let unknown_props = collect_unknown_props(node, VARIANT_KNOWN_PROPS);
    let source_span = node_span(node);

    let mut overrides: Vec<VariantOverride> = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() == "override" {
                overrides.push(transform_variant_override(child)?);
            }
        }
    }

    Ok(VariantDef {
        id,
        source,
        w,
        h,
        overrides,
        source_span,
        unknown_props,
    })
}

fn transform_variant_override(node: &KdlNode) -> Result<VariantOverride, ParseError> {
    let target_node = required_string_prop(node, "node")?.to_owned();
    let visible = optional_bool_prop(node, "visible");
    let x = optional_dimension_prop(node, "x");
    let y = optional_dimension_prop(node, "y");
    let w = optional_dimension_prop(node, "w");
    let h = optional_dimension_prop(node, "h");
    let fill = node
        .entry("fill")
        .and_then(|e| entry_to_property_value(e).ok());
    let text = optional_string_prop(node, "text").map(str::to_owned);
    let unknown_props = collect_unknown_props(node, VARIANT_OVERRIDE_KNOWN_PROPS);
    let source_span = node_span(node);

    Ok(VariantOverride {
        node: target_node,
        visible,
        x,
        y,
        w,
        h,
        fill,
        text,
        source_span,
        unknown_props,
    })
}

// ---------------------------------------------------------------------------
// Recipes
// ---------------------------------------------------------------------------

const RECIPE_KNOWN_PROPS: &[&str] = &["id", "kind", "seed", "generator", "bounds", "detached"];
const RECIPE_PARAM_KNOWN_PROPS: &[&str] = &["name", "value"];

/// Transform the document-level `recipes { … }` block into a list of
/// [`RecipeDef`]. Each `recipe id="…" kind="…" …` is a block node; non-`recipe`
/// children inside the block are silently ignored (forward-compat). Mirrors
/// [`transform_variants`].
pub(super) fn transform_recipes(node: &KdlNode) -> Result<Vec<RecipeDef>, ParseError> {
    let mut defs: Vec<RecipeDef> = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() == "recipe" {
                defs.push(transform_recipe_def(child)?);
            }
        }
    }
    Ok(defs)
}

fn transform_recipe_def(node: &KdlNode) -> Result<RecipeDef, ParseError> {
    let id = required_string_prop(node, "id")?.to_owned();
    let kind = required_string_prop(node, "kind")?.to_owned();

    // Optional integer seed: negative seeds are valid, so read as i64 not u32.
    let seed = optional_i64_prop(node, "seed");

    let generator = optional_string_prop(node, "generator").map(str::to_owned);
    let bounds = optional_string_prop(node, "bounds").map(str::to_owned);
    let detached = optional_bool_prop(node, "detached");

    let unknown_props = collect_unknown_props(node, RECIPE_KNOWN_PROPS);
    let source_span = node_span(node);

    let mut params: Vec<RecipeParam> = Vec::new();
    let mut palette: Vec<String> = Vec::new();
    let mut expanded: Vec<String> = Vec::new();

    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "param" => {
                    params.push(transform_recipe_param(child)?);
                }
                "palette" => {
                    palette.push(required_string_prop(child, "token")?.to_owned());
                }
                "expanded" => {
                    expanded.push(required_string_prop(child, "node")?.to_owned());
                }
                _ => {}
            }
        }
    }

    Ok(RecipeDef {
        id,
        kind,
        seed,
        generator,
        bounds,
        detached,
        params,
        palette,
        expanded,
        source_span,
        unknown_props,
    })
}

fn transform_recipe_param(node: &KdlNode) -> Result<RecipeParam, ParseError> {
    let name = required_string_prop(node, "name")?.to_owned();
    let value = node
        .entry("value")
        .ok_or_else(|| {
            ParseError::spanless(
                ParseErrorCode::InvalidPropertyValue,
                format!("recipe `param` `{name}` is missing required property `value`"),
            )
        })
        .and_then(entry_to_property_value)?;
    let unknown_props = collect_unknown_props(node, RECIPE_PARAM_KNOWN_PROPS);
    let source_span = node_span(node);

    Ok(RecipeParam {
        name,
        value,
        source_span,
        unknown_props,
    })
}
