//! Transforms for the axis-aligned leaf shapes: rect, image, ellipse, line.

use kdl::KdlNode;

use crate::ast::node::{EllipseNode, ImageNode, LineNode, RectNode};
use crate::error::ParseError;

use crate::parse::transform::helpers::{
    collect_unknown_props, node_span, optional_bool_prop, optional_dimension_prop,
    optional_f64_prop, optional_object_position_prop, optional_property_value,
    optional_property_value_aliased, optional_string_prop, optional_string_prop_aliased,
    required_string_prop,
};

pub(crate) const RECT_KNOWN_PROPS: &[&str] = &[
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
    "anchor-edge",
    "anchor_edge",
    "anchor-gap",
    "anchor_gap",
    "anchor-parent",
    "anchor_parent",
];

pub(in crate::parse::transform) fn transform_rect(node: &KdlNode) -> Result<RectNode, ParseError> {
    let id = required_string_prop(node, "id")?.to_owned();

    // Handle both hyphenated and underscored variants for forward-compat.
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

    // Per-corner radius overrides: accept both hyphenated and underscored spellings.
    let radius_tl = optional_property_value_aliased(node, "radius-tl", "radius_tl");
    let radius_tr = optional_property_value_aliased(node, "radius-tr", "radius_tr");
    let radius_br = optional_property_value_aliased(node, "radius-br", "radius_br");
    let radius_bl = optional_property_value_aliased(node, "radius-bl", "radius_bl");

    // Per-side border colors.
    let border_top = optional_property_value_aliased(node, "border-top", "border_top");
    let border_bottom = optional_property_value_aliased(node, "border-bottom", "border_bottom");
    let border_left = optional_property_value_aliased(node, "border-left", "border_left");
    let border_right = optional_property_value_aliased(node, "border-right", "border_right");
    // Shared border width.
    let border_width = optional_property_value_aliased(node, "border-width", "border_width");
    // Double-border (outer stroke).
    let stroke_outer = optional_property_value_aliased(node, "stroke-outer", "stroke_outer");
    let stroke_outer_width =
        optional_property_value_aliased(node, "stroke-outer-width", "stroke_outer_width");

    let unknown_props = collect_unknown_props(node, RECT_KNOWN_PROPS);

    Ok(RectNode {
        id,
        name: optional_string_prop(node, "name").map(str::to_owned),
        role: optional_string_prop(node, "role").map(str::to_owned),
        x: optional_property_value(node, "x"),
        y: optional_property_value(node, "y"),
        w: optional_property_value(node, "w"),
        h: optional_property_value(node, "h"),
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
        anchor_edge: optional_string_prop(node, "anchor-edge")
            .or_else(|| optional_string_prop(node, "anchor_edge"))
            .map(str::to_owned),
        anchor_gap: optional_dimension_prop(node, "anchor-gap")
            .or_else(|| optional_dimension_prop(node, "anchor_gap")),
        anchor_parent: optional_bool_prop(node, "anchor-parent")
            .or_else(|| optional_bool_prop(node, "anchor_parent")),
        source_span: node_span(node),
        unknown_props,
    })
}

pub(crate) const IMAGE_KNOWN_PROPS: &[&str] = &[
    "id",
    "name",
    "role",
    "asset",
    "x",
    "y",
    "w",
    "h",
    "src-x",
    "src_x",
    "src-y",
    "src_y",
    "src-w",
    "src_w",
    "src-h",
    "src_h",
    "fit",
    "svg-stroke",
    "svg_stroke",
    "svg-fill",
    "svg_fill",
    "svg-stroke-width",
    "svg_stroke_width",
    "clip",
    "clip-radius",
    "clip_radius",
    "object-position-x",
    "object_position_x",
    "object-position-y",
    "object_position_y",
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
    "style",
    "anchor",
    "anchor-zone",
    "anchor_zone",
    "anchor-sibling",
    "anchor_sibling",
    "anchor-edge",
    "anchor_edge",
    "anchor-gap",
    "anchor_gap",
    "anchor-parent",
    "anchor_parent",
];

pub(in crate::parse::transform) fn transform_image(
    node: &KdlNode,
) -> Result<ImageNode, ParseError> {
    let id = required_string_prop(node, "id")?.to_owned();
    let asset = required_string_prop(node, "asset")?.to_owned();

    // object-position accepts hyphenated or underscored spellings.
    let object_position_x = optional_object_position_prop(node, "object-position-x")
        .or_else(|| optional_object_position_prop(node, "object_position_x"));
    let object_position_y = optional_object_position_prop(node, "object-position-y")
        .or_else(|| optional_object_position_prop(node, "object_position_y"));

    // src-* accept hyphenated or underscored spellings.
    let src_x =
        optional_dimension_prop(node, "src-x").or_else(|| optional_dimension_prop(node, "src_x"));
    let src_y =
        optional_dimension_prop(node, "src-y").or_else(|| optional_dimension_prop(node, "src_y"));
    let src_w =
        optional_dimension_prop(node, "src-w").or_else(|| optional_dimension_prop(node, "src_w"));
    let src_h =
        optional_dimension_prop(node, "src-h").or_else(|| optional_dimension_prop(node, "src_h"));

    let unknown_props = collect_unknown_props(node, IMAGE_KNOWN_PROPS);

    Ok(ImageNode {
        id,
        name: optional_string_prop(node, "name").map(str::to_owned),
        role: optional_string_prop(node, "role").map(str::to_owned),
        asset,
        x: optional_property_value(node, "x"),
        y: optional_property_value(node, "y"),
        w: optional_property_value(node, "w"),
        h: optional_property_value(node, "h"),
        src_x,
        src_y,
        src_w,
        src_h,
        fit: optional_string_prop(node, "fit").map(str::to_owned),
        svg_stroke: optional_property_value_aliased(node, "svg-stroke", "svg_stroke"),
        svg_fill: optional_property_value_aliased(node, "svg-fill", "svg_fill"),
        svg_stroke_width: optional_property_value_aliased(
            node,
            "svg-stroke-width",
            "svg_stroke_width",
        ),
        clip: optional_string_prop(node, "clip").map(str::to_owned),
        clip_radius: optional_property_value_aliased(node, "clip-radius", "clip_radius"),
        object_position_x,
        object_position_y,
        shadow: optional_property_value(node, "shadow"),
        filter: optional_property_value(node, "filter"),
        mask: optional_property_value(node, "mask"),
        blend_mode: optional_string_prop_aliased(node, "blend-mode", "blend_mode")
            .map(str::to_owned),
        blur: optional_dimension_prop(node, "blur"),
        opacity: optional_f64_prop(node, "opacity"),
        visible: optional_bool_prop(node, "visible"),
        locked: optional_bool_prop(node, "locked"),
        rotate: optional_dimension_prop(node, "rotate"),
        style: optional_string_prop(node, "style").map(str::to_owned),
        anchor: optional_string_prop(node, "anchor").map(str::to_owned),
        anchor_zone: optional_string_prop(node, "anchor-zone")
            .or_else(|| optional_string_prop(node, "anchor_zone"))
            .map(str::to_owned),
        anchor_sibling: optional_string_prop(node, "anchor-sibling")
            .or_else(|| optional_string_prop(node, "anchor_sibling"))
            .map(str::to_owned),
        anchor_edge: optional_string_prop(node, "anchor-edge")
            .or_else(|| optional_string_prop(node, "anchor_edge"))
            .map(str::to_owned),
        anchor_gap: optional_dimension_prop(node, "anchor-gap")
            .or_else(|| optional_dimension_prop(node, "anchor_gap")),
        anchor_parent: optional_bool_prop(node, "anchor-parent")
            .or_else(|| optional_bool_prop(node, "anchor_parent")),
        source_span: node_span(node),
        unknown_props,
    })
}

pub(crate) const ELLIPSE_KNOWN_PROPS: &[&str] = &[
    "id",
    "name",
    "role",
    "x",
    "y",
    "w",
    "h",
    "rx",
    "ry",
    "style",
    "fill",
    "stroke",
    "stroke-width",
    "stroke_width",
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
    "anchor",
    "anchor-zone",
    "anchor_zone",
    "anchor-sibling",
    "anchor_sibling",
    "anchor-edge",
    "anchor_edge",
    "anchor-gap",
    "anchor_gap",
    "anchor-parent",
    "anchor_parent",
];

pub(in crate::parse::transform) fn transform_ellipse(
    node: &KdlNode,
) -> Result<EllipseNode, ParseError> {
    let id = required_string_prop(node, "id")?.to_owned();

    // Handle both hyphenated and underscored variants for forward-compat.
    let stroke_width = optional_property_value_aliased(node, "stroke-width", "stroke_width");
    let stroke_dash = optional_property_value_aliased(node, "stroke-dash", "stroke_dash");
    let stroke_gap = optional_property_value_aliased(node, "stroke-gap", "stroke_gap");
    let stroke_linecap =
        optional_string_prop_aliased(node, "stroke-linecap", "stroke_linecap").map(str::to_owned);
    let blend_mode =
        optional_string_prop_aliased(node, "blend-mode", "blend_mode").map(str::to_owned);

    // Independent axis radii (override inscribed-ellipse default).
    let rx = optional_property_value(node, "rx");
    let ry = optional_property_value(node, "ry");

    let unknown_props = collect_unknown_props(node, ELLIPSE_KNOWN_PROPS);

    Ok(EllipseNode {
        id,
        name: optional_string_prop(node, "name").map(str::to_owned),
        role: optional_string_prop(node, "role").map(str::to_owned),
        x: optional_property_value(node, "x"),
        y: optional_property_value(node, "y"),
        w: optional_property_value(node, "w"),
        h: optional_property_value(node, "h"),
        rx,
        ry,
        style: optional_string_prop(node, "style").map(str::to_owned),
        fill: optional_property_value(node, "fill"),
        stroke: optional_property_value(node, "stroke"),
        stroke_width,
        stroke_dash,
        stroke_gap,
        stroke_linecap,
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
        anchor_edge: optional_string_prop(node, "anchor-edge")
            .or_else(|| optional_string_prop(node, "anchor_edge"))
            .map(str::to_owned),
        anchor_gap: optional_dimension_prop(node, "anchor-gap")
            .or_else(|| optional_dimension_prop(node, "anchor_gap")),
        anchor_parent: optional_bool_prop(node, "anchor-parent")
            .or_else(|| optional_bool_prop(node, "anchor_parent")),
        source_span: node_span(node),
        unknown_props,
    })
}

pub(crate) const LINE_KNOWN_PROPS: &[&str] = &[
    "id",
    "name",
    "role",
    "x1",
    "y1",
    "x2",
    "y2",
    "style",
    "stroke",
    "stroke-width",
    "stroke_width",
    "stroke-dash",
    "stroke_dash",
    "stroke-gap",
    "stroke_gap",
    "stroke-linecap",
    "stroke_linecap",
    "opacity",
    "visible",
    "locked",
    // NOTE: "stroke-alignment" is intentionally absent — it does not apply to
    // line nodes. An author who writes it will receive a node.unknown_property
    // warning, which is the correct diagnostic for inapplicable properties.
];

pub(in crate::parse::transform) fn transform_line(node: &KdlNode) -> Result<LineNode, ParseError> {
    let id = required_string_prop(node, "id")?.to_owned();

    // Handle both hyphenated and underscored variants for forward-compat.
    let stroke_width = optional_property_value_aliased(node, "stroke-width", "stroke_width");
    let stroke_dash = optional_property_value_aliased(node, "stroke-dash", "stroke_dash");
    let stroke_gap = optional_property_value_aliased(node, "stroke-gap", "stroke_gap");
    let stroke_linecap =
        optional_string_prop_aliased(node, "stroke-linecap", "stroke_linecap").map(str::to_owned);

    let unknown_props = collect_unknown_props(node, LINE_KNOWN_PROPS);

    Ok(LineNode {
        id,
        name: optional_string_prop(node, "name").map(str::to_owned),
        role: optional_string_prop(node, "role").map(str::to_owned),
        x1: optional_dimension_prop(node, "x1"),
        y1: optional_dimension_prop(node, "y1"),
        x2: optional_dimension_prop(node, "x2"),
        y2: optional_dimension_prop(node, "y2"),
        style: optional_string_prop(node, "style").map(str::to_owned),
        stroke: optional_property_value(node, "stroke"),
        stroke_width,
        stroke_dash,
        stroke_gap,
        stroke_linecap,
        opacity: optional_f64_prop(node, "opacity"),
        visible: optional_bool_prop(node, "visible"),
        locked: optional_bool_prop(node, "locked"),
        source_span: node_span(node),
        unknown_props,
    })
}
