//! The document-level `variants { … }` and `recipes { … }` blocks.

use kdl::{KdlNode, KdlValue};

use crate::ast::node::{LayoutKind, SizeKeyword};
use crate::ast::recipe::{RecipeDef, RecipeParam};
use crate::ast::value::Dimension;
use crate::ast::variant::{VariantDef, VariantOverride};
use crate::error::{ParseError, ParseErrorCode};
use crate::parse::transform::helpers::{
    collect_unknown_props, entry_to_dimension, entry_to_property_value, node_span,
    optional_bool_prop, optional_i64_prop, optional_property_value, optional_string_prop,
    required_string_prop, spanned,
};
use crate::parse::transform::layout::{size_keyword, transform_layout_container};

// ---------------------------------------------------------------------------
// Variants
// ---------------------------------------------------------------------------

const VARIANT_KNOWN_PROPS: &[&str] = &["id", "source", "w", "h"];
pub(crate) const VARIANT_OVERRIDE_KNOWN_PROPS: &[&str] = &[
    "node",
    "visible",
    "x",
    "y",
    "w",
    "h",
    "fill",
    "text",
    "font-size",
    "layout",
    "gap",
    "wrap-gap",
    "padding",
    "padding-x",
    "padding-y",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "justify",
    "align",
    "wrap",
];

/// Transform the document-level `variants { … }` block into a list of
/// [`VariantDef`]. Each `variant id="…" source="…" w=(px)N h=(px)N { … }` is
/// a block node; non-`variant` children inside the block are reported as
/// `block.unknown_child` Errors. Mirrors [`transform_provenance`](super::structure::transform_provenance).
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
            ParseError::at(
                node_span(node),
                ParseErrorCode::InvalidPropertyValue,
                format!("variant `{id}` is missing required property `w`"),
            )
        })
        .and_then(|e| entry_to_dimension(e, "w"))?;

    let h = node
        .entry("h")
        .ok_or_else(|| {
            ParseError::at(
                node_span(node),
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
    let x = spanned(override_dimension(node, "x"), node)?;
    let y = spanned(override_dimension(node, "y"), node)?;
    let (w, w_keyword) = spanned(override_size(node, "w"), node)?;
    let (h, h_keyword) = spanned(override_size(node, "h"), node)?;
    let fill = node
        .entry("fill")
        .and_then(|e| entry_to_property_value(e).ok());
    let text = optional_string_prop(node, "text").map(str::to_owned);
    let font_size = optional_property_value(node, "font-size");
    let layout = optional_string_prop(node, "layout").map(LayoutKind::from_attr);
    let container = transform_layout_container(node);
    let unknown_props = collect_unknown_props(node, VARIANT_OVERRIDE_KNOWN_PROPS);
    let source_span = node_span(node);

    Ok(VariantOverride {
        node: target_node,
        visible,
        x,
        y,
        w,
        h,
        w_keyword,
        h_keyword,
        fill,
        text,
        font_size,
        layout,
        container,
        source_span,
        unknown_props,
    })
}

/// Read an override `x`/`y`. A present value must be a typed dimension.
fn override_dimension(node: &KdlNode, key: &str) -> Result<Option<Dimension>, ParseError> {
    node.entry(key)
        .map(|e| entry_to_dimension(e, &format!("override {key}")))
        .transpose()
}

/// Read an override `w`/`h`: a typed dimension, or the keyword `hug` / `fill`.
fn override_size(
    node: &KdlNode,
    key: &str,
) -> Result<(Option<Dimension>, Option<SizeKeyword>), ParseError> {
    if let Some(keyword) = size_keyword(node, key) {
        return Ok((None, Some(keyword)));
    }
    let Some(entry) = node.entry(key) else {
        return Ok((None, None));
    };
    match entry.value() {
        KdlValue::String(s) => Err(ParseError::spanless(
            ParseErrorCode::InvalidPropertyValue,
            format!(
                "override {key}=\"{s}\" is not a size; use a dimension such as (px)800, \
                 or \"hug\" or \"fill\""
            ),
        )),
        KdlValue::Integer(_) | KdlValue::Float(_) | KdlValue::Bool(_) | KdlValue::Null => {
            entry_to_dimension(entry, &format!("override {key}")).map(|d| (Some(d), None))
        }
    }
}

// ---------------------------------------------------------------------------
// Recipes
// ---------------------------------------------------------------------------

const RECIPE_KNOWN_PROPS: &[&str] = &["id", "kind", "seed", "generator", "bounds", "detached"];
const RECIPE_PARAM_KNOWN_PROPS: &[&str] = &["name", "value"];

/// Transform the document-level `recipes { … }` block into a list of
/// [`RecipeDef`]. Each `recipe id="…" kind="…" …` is a block node; non-`recipe`
/// children inside the block are reported as `block.unknown_child` Errors. Mirrors
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
            ParseError::at(
                node_span(node),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn override_node(attrs: &str) -> KdlNode {
        let doc: kdl::KdlDocument = format!("override node=\"n\" {attrs}").parse().expect("kdl");
        doc.nodes().first().cloned().expect("one node")
    }

    #[test]
    fn override_size_reads_dimension_keyword_or_nothing() {
        let n = override_node(r#"w=(px)12 h="fill""#);
        let (w, wk) = override_size(&n, "w").expect("w parses");
        assert_eq!((w.map(|d| d.value), wk), (Some(12.0), None));
        let (h, hk) = override_size(&n, "h").expect("h parses");
        assert_eq!((h, hk), (None, Some(SizeKeyword::Fill)));
        let none = override_node("");
        assert_eq!(override_size(&none, "w").expect("absent"), (None, None));
    }

    #[test]
    fn override_size_rejects_other_strings_and_bare_numbers() {
        let n = override_node(r#"w="wide" h=40"#);
        let w = override_size(&n, "w").expect_err("string fails");
        assert!(w.message.contains("override w=\"wide\" is not a size"));
        let h = override_size(&n, "h").expect_err("bare number fails");
        assert!(h.message.contains("override h"));
    }

    #[test]
    fn layout_keys_are_known_props() {
        let ov = transform_variant_override(&override_node(
            r#"layout="grid" gap=(px)4 wrap=#true font-size=(token)"size.x""#,
        ))
        .expect("override parses");
        assert!(ov.unknown_props.is_empty(), "{:?}", ov.unknown_props);
        assert_eq!(ov.layout, Some(LayoutKind::Grid));
        assert_eq!(ov.container.wrap, Some(true));
        assert!(ov.font_size.is_some());
    }
}
