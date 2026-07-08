//! Transforms for the point-based leaf nodes: polygon, polyline, path — plus
//! the shared `point`, `anchor`, and `subpath` children.

use kdl::KdlNode;

use crate::ast::node::{
    AnchorKind, PathAnchor, PathNode, PathSubpath, Point, PolygonNode, PolylineNode,
};
use crate::error::ParseError;

use crate::parse::transform::helpers::{
    collect_unknown_props, node_span, optional_bool_prop, optional_dimension_prop,
    optional_f64_prop, optional_property_value, optional_property_value_aliased,
    optional_string_prop, optional_string_prop_aliased, required_string_prop,
};

pub(crate) const POLYGON_KNOWN_PROPS: &[&str] = &[
    "id",
    "name",
    "role",
    "fill",
    "stroke",
    "stroke-width",
    "stroke_width",
    "stroke-alignment",
    "stroke_alignment",
    "fill-rule",
    "fill_rule",
    "opacity",
    "visible",
    "locked",
    "rotate",
    "style",
];

// NOTE: polyline intentionally omits stroke-alignment — an author
// writing it gets a node.unknown_property warning, which is correct.
pub(crate) const POLYLINE_KNOWN_PROPS: &[&str] = &[
    "id",
    "name",
    "role",
    "fill",
    "stroke",
    "stroke-width",
    "stroke_width",
    "fill-rule",
    "fill_rule",
    "opacity",
    "visible",
    "locked",
    "rotate",
    "style",
];

pub(crate) const PATH_KNOWN_PROPS: &[&str] = &[
    "id",
    "name",
    "role",
    "closed",
    "fill",
    "stroke",
    "stroke-width",
    "stroke_width",
    "stroke-alignment",
    "stroke_alignment",
    "stroke-linejoin",
    "stroke_linejoin",
    "stroke-linecap",
    "stroke_linecap",
    "stroke-miter-limit",
    "stroke_miter_limit",
    "fill-rule",
    "fill_rule",
    "opacity",
    "visible",
    "locked",
    "rotate",
    "style",
];

/// Transform a `point` child node into a [`Point`].
///
/// `x` and `y` are optional at parse time; validate checks their presence.
fn transform_point(node: &KdlNode) -> Point {
    Point {
        x: optional_dimension_prop(node, "x"),
        y: optional_dimension_prop(node, "y"),
    }
}

/// Transform an `anchor` child node into a [`PathAnchor`].
///
/// All fields are optional at parse time; validate checks required anchor
/// coordinates and handle-pair completeness.
fn transform_path_anchor(node: &KdlNode) -> PathAnchor {
    PathAnchor {
        x: optional_dimension_prop(node, "x"),
        y: optional_dimension_prop(node, "y"),
        kind: optional_string_prop(node, "kind").map(AnchorKind::from_kind_str),
        in_x: optional_dimension_prop(node, "in-x")
            .or_else(|| optional_dimension_prop(node, "in_x")),
        in_y: optional_dimension_prop(node, "in-y")
            .or_else(|| optional_dimension_prop(node, "in_y")),
        out_x: optional_dimension_prop(node, "out-x")
            .or_else(|| optional_dimension_prop(node, "out_x")),
        out_y: optional_dimension_prop(node, "out-y")
            .or_else(|| optional_dimension_prop(node, "out_y")),
    }
}

fn transform_path_subpath(node: &KdlNode) -> PathSubpath {
    let mut anchors: Vec<PathAnchor> = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() == "anchor" {
                anchors.push(transform_path_anchor(child));
            }
        }
    }

    PathSubpath {
        closed: optional_bool_prop(node, "closed"),
        anchors,
    }
}

pub(in crate::parse::transform) fn transform_polygon(
    node: &KdlNode,
) -> Result<PolygonNode, ParseError> {
    let id = required_string_prop(node, "id")?.to_owned();

    let stroke_width = optional_property_value_aliased(node, "stroke-width", "stroke_width");
    let stroke_alignment =
        optional_string_prop_aliased(node, "stroke-alignment", "stroke_alignment")
            .map(str::to_owned);
    let fill_rule = optional_string_prop_aliased(node, "fill-rule", "fill_rule").map(str::to_owned);

    // Collect `point` child nodes — this is where the vertex list lives.
    let mut points: Vec<Point> = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() == "point" {
                points.push(transform_point(child));
            }
        }
    }

    let unknown_props = collect_unknown_props(node, POLYGON_KNOWN_PROPS);

    Ok(PolygonNode {
        id,
        name: optional_string_prop(node, "name").map(str::to_owned),
        role: optional_string_prop(node, "role").map(str::to_owned),
        fill: optional_property_value(node, "fill"),
        stroke: optional_property_value(node, "stroke"),
        stroke_width,
        stroke_alignment,
        fill_rule,
        opacity: optional_f64_prop(node, "opacity"),
        visible: optional_bool_prop(node, "visible"),
        locked: optional_bool_prop(node, "locked"),
        rotate: optional_dimension_prop(node, "rotate"),
        style: optional_string_prop(node, "style").map(str::to_owned),
        points,
        source_span: node_span(node),
        unknown_props,
    })
}

pub(in crate::parse::transform) fn transform_polyline(
    node: &KdlNode,
) -> Result<PolylineNode, ParseError> {
    let id = required_string_prop(node, "id")?.to_owned();

    let stroke_width = optional_property_value_aliased(node, "stroke-width", "stroke_width");
    let fill_rule = optional_string_prop_aliased(node, "fill-rule", "fill_rule").map(str::to_owned);

    // Collect `point` child nodes.
    let mut points: Vec<Point> = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() == "point" {
                points.push(transform_point(child));
            }
        }
    }

    let unknown_props = collect_unknown_props(node, POLYLINE_KNOWN_PROPS);

    Ok(PolylineNode {
        id,
        name: optional_string_prop(node, "name").map(str::to_owned),
        role: optional_string_prop(node, "role").map(str::to_owned),
        fill: optional_property_value(node, "fill"),
        stroke: optional_property_value(node, "stroke"),
        stroke_width,
        fill_rule,
        opacity: optional_f64_prop(node, "opacity"),
        visible: optional_bool_prop(node, "visible"),
        locked: optional_bool_prop(node, "locked"),
        rotate: optional_dimension_prop(node, "rotate"),
        style: optional_string_prop(node, "style").map(str::to_owned),
        points,
        source_span: node_span(node),
        unknown_props,
    })
}

pub(in crate::parse::transform) fn transform_path(node: &KdlNode) -> Result<PathNode, ParseError> {
    let id = required_string_prop(node, "id")?.to_owned();

    let stroke_width = optional_property_value_aliased(node, "stroke-width", "stroke_width");
    let stroke_alignment =
        optional_string_prop_aliased(node, "stroke-alignment", "stroke_alignment")
            .map(str::to_owned);
    let stroke_linejoin =
        optional_string_prop_aliased(node, "stroke-linejoin", "stroke_linejoin").map(str::to_owned);
    let stroke_linecap =
        optional_string_prop_aliased(node, "stroke-linecap", "stroke_linecap").map(str::to_owned);
    let stroke_miter_limit = optional_f64_prop(node, "stroke-miter-limit")
        .or_else(|| optional_f64_prop(node, "stroke_miter_limit"));
    let fill_rule = optional_string_prop_aliased(node, "fill-rule", "fill_rule").map(str::to_owned);

    let mut anchors: Vec<PathAnchor> = Vec::new();
    let mut subpaths: Vec<PathSubpath> = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "anchor" => anchors.push(transform_path_anchor(child)),
                "subpath" => subpaths.push(transform_path_subpath(child)),
                _ => {}
            }
        }
    }

    let unknown_props = collect_unknown_props(node, PATH_KNOWN_PROPS);

    Ok(PathNode {
        id,
        name: optional_string_prop(node, "name").map(str::to_owned),
        role: optional_string_prop(node, "role").map(str::to_owned),
        closed: optional_bool_prop(node, "closed"),
        fill: optional_property_value(node, "fill"),
        stroke: optional_property_value(node, "stroke"),
        stroke_width,
        stroke_alignment,
        stroke_linejoin,
        stroke_linecap,
        stroke_miter_limit,
        fill_rule,
        opacity: optional_f64_prop(node, "opacity"),
        visible: optional_bool_prop(node, "visible"),
        locked: optional_bool_prop(node, "locked"),
        rotate: optional_dimension_prop(node, "rotate"),
        style: optional_string_prop(node, "style").map(str::to_owned),
        anchors,
        subpaths,
        source_span: node_span(node),
        unknown_props,
    })
}
