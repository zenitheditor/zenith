//! Transform for the `pattern` node: the procedural primitive that carries one
//! TEMPLATE child (`motif`). The common visual/geometry props are read exactly
//! like `rect`; the pattern-specific props (`kind`, `seed`, `count`,
//! `spacing`, `jitter`) describe the deferred expansion.

use kdl::KdlNode;

use crate::ast::node::PatternNode;
use crate::error::{ParseError, ParseErrorCode};

use super::helpers::{
    collect_unknown_props, node_span, optional_bool_prop, optional_dimension_prop,
    optional_f64_prop, optional_i64_prop, optional_property_value, optional_property_value_aliased,
    optional_string_prop, optional_string_prop_aliased, required_string_prop,
};
use super::node::transform_node;

const PATTERN_KNOWN_PROPS: &[&str] = &[
    "id",
    "name",
    "role",
    "x",
    "y",
    "w",
    "h",
    "radius",
    "radius-tl",
    "radius_tl",
    "radius-tr",
    "radius_tr",
    "radius-br",
    "radius_br",
    "radius-bl",
    "radius_bl",
    "style",
    "fill",
    "stroke",
    "stroke-width",
    "stroke_width",
    "stroke-alignment",
    "stroke_alignment",
    "stroke-dash",
    "stroke_dash",
    "stroke-gap",
    "stroke_gap",
    "stroke-linecap",
    "stroke_linecap",
    "shadow",
    "filter",
    "mask",
    "blend-mode",
    "blend_mode",
    "blur",
    "opacity",
    "visible",
    "locked",
    "rotate",
    "border-top",
    "border_top",
    "border-bottom",
    "border_bottom",
    "border-left",
    "border_left",
    "border-right",
    "border_right",
    "border-width",
    "border_width",
    "stroke-outer",
    "stroke_outer",
    "stroke-outer-width",
    "stroke_outer_width",
    "anchor",
    "anchor-zone",
    "anchor_zone",
    "anchor-sibling",
    "anchor_sibling",
    "anchor-parent",
    "anchor_parent",
    "kind",
    "seed",
    "count",
    "spacing",
    "jitter",
];

pub(super) fn transform_pattern(node: &KdlNode) -> Result<PatternNode, ParseError> {
    let id = required_string_prop(node, "id")?.to_owned();
    let kind = required_string_prop(node, "kind")?.to_owned();

    // Common visual props (mirror transform_rect): accept hyphen + underscore.
    let stroke_width = optional_property_value_aliased(node, "stroke-width", "stroke_width");
    let stroke_alignment =
        optional_string_prop_aliased(node, "stroke-alignment", "stroke_alignment")
            .map(str::to_owned);
    let stroke_dash = optional_property_value_aliased(node, "stroke-dash", "stroke_dash");
    let stroke_gap = optional_property_value_aliased(node, "stroke-gap", "stroke_gap");
    let stroke_linecap =
        optional_string_prop_aliased(node, "stroke-linecap", "stroke_linecap").map(str::to_owned);
    let blend_mode =
        optional_string_prop_aliased(node, "blend-mode", "blend_mode").map(str::to_owned);

    let radius_tl = optional_property_value_aliased(node, "radius-tl", "radius_tl");
    let radius_tr = optional_property_value_aliased(node, "radius-tr", "radius_tr");
    let radius_br = optional_property_value_aliased(node, "radius-br", "radius_br");
    let radius_bl = optional_property_value_aliased(node, "radius-bl", "radius_bl");

    let border_top = optional_property_value_aliased(node, "border-top", "border_top");
    let border_bottom = optional_property_value_aliased(node, "border-bottom", "border_bottom");
    let border_left = optional_property_value_aliased(node, "border-left", "border_left");
    let border_right = optional_property_value_aliased(node, "border-right", "border_right");
    let border_width = optional_property_value_aliased(node, "border-width", "border_width");
    let stroke_outer = optional_property_value_aliased(node, "stroke-outer", "stroke_outer");
    let stroke_outer_width =
        optional_property_value_aliased(node, "stroke-outer-width", "stroke_outer_width");

    // The motif: the FIRST child node of the pattern's children block. A pattern
    // with no child motif is a hard parse error.
    let motif = node
        .children()
        .and_then(|c| c.nodes().first())
        .ok_or_else(|| {
            ParseError::spanless(
                ParseErrorCode::UnexpectedNode,
                format!("pattern `{id}` is missing its required child motif node"),
            )
        })?;
    let motif = Box::new(transform_node(motif)?);

    let unknown_props = collect_unknown_props(node, PATTERN_KNOWN_PROPS);

    Ok(PatternNode {
        id,
        name: optional_string_prop(node, "name").map(str::to_owned),
        role: optional_string_prop(node, "role").map(str::to_owned),
        x: optional_dimension_prop(node, "x"),
        y: optional_dimension_prop(node, "y"),
        w: optional_dimension_prop(node, "w"),
        h: optional_dimension_prop(node, "h"),
        radius: optional_property_value(node, "radius"),
        radius_tl,
        radius_tr,
        radius_br,
        radius_bl,
        style: optional_string_prop(node, "style").map(str::to_owned),
        fill: optional_property_value(node, "fill"),
        stroke: optional_property_value(node, "stroke"),
        stroke_width,
        stroke_alignment,
        stroke_dash,
        stroke_gap,
        stroke_linecap,
        border_top,
        border_bottom,
        border_left,
        border_right,
        border_width,
        stroke_outer,
        stroke_outer_width,
        shadow: optional_property_value(node, "shadow"),
        filter: optional_property_value(node, "filter"),
        mask: optional_property_value(node, "mask"),
        blend_mode,
        blur: optional_dimension_prop(node, "blur"),
        opacity: optional_f64_prop(node, "opacity"),
        visible: optional_bool_prop(node, "visible"),
        locked: optional_bool_prop(node, "locked"),
        rotate: optional_dimension_prop(node, "rotate"),
        anchor: optional_string_prop(node, "anchor").map(str::to_owned),
        anchor_zone: optional_string_prop(node, "anchor-zone")
            .or_else(|| optional_string_prop(node, "anchor_zone"))
            .map(str::to_owned),
        anchor_sibling: optional_string_prop(node, "anchor-sibling")
            .or_else(|| optional_string_prop(node, "anchor_sibling"))
            .map(str::to_owned),
        anchor_parent: optional_bool_prop(node, "anchor-parent")
            .or_else(|| optional_bool_prop(node, "anchor_parent")),
        kind,
        seed: optional_i64_prop(node, "seed"),
        count: optional_i64_prop(node, "count"),
        spacing: optional_dimension_prop(node, "spacing"),
        jitter: optional_f64_prop(node, "jitter"),
        motif,
        source_span: node_span(node),
        unknown_props,
    })
}
