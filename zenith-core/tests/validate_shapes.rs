//! Integration tests: shapes validation.
//!
//! Test bodies moved verbatim from the former in-`src` `validate/check/tests/`
//! concern files; only import paths changed (`crate::`/`super::common` ->
//! `zenith_core::`/`common`).

use std::collections::BTreeMap;

#[path = "common/codes.rs"]
mod codes;
#[path = "common/color_token.rs"]
mod color_token;
#[path = "common/dim_token.rs"]
mod dim_token;
#[path = "common/has_code.rs"]
mod has_code;
#[path = "common/minimal_page.rs"]
mod minimal_page;
#[path = "common/minimal_rect.rs"]
mod minimal_rect;
#[path = "common/minimal_text.rs"]
mod minimal_text;
#[path = "common/px.rs"]
mod px;
#[path = "common/pxv_doc_with.rs"]
mod pxv_doc_with;
#[path = "common/token_ref.rs"]
mod token_ref;

use codes::codes;
use color_token::color_token;
use dim_token::dim_token;
use has_code::has_code;
use minimal_page::minimal_page;
use minimal_rect::minimal_rect;
use minimal_text::minimal_text;
use px::px;
use pxv_doc_with::{doc_with, pxv};
use token_ref::token_ref;
use zenith_core::{
    ConnectorNode, KdlAdapter, KdlSource, Node, Point, PolygonNode, PolylineNode, PropertyValue,
    Severity, ShapeNode, TextSpan, ValidationReport, validate,
};

fn tri_points() -> Vec<Point> {
    vec![
        Point {
            x: Some(px(160.0)),
            y: Some(px(40.0)),
        },
        Point {
            x: Some(px(260.0)),
            y: Some(px(170.0)),
        },
        Point {
            x: Some(px(60.0)),
            y: Some(px(170.0)),
        },
    ]
}

fn minimal_polygon(id: &str, fill: Option<PropertyValue>) -> Node {
    Node::Polygon(PolygonNode {
        id: id.to_owned(),
        name: None,
        role: None,
        fill,
        stroke: None,
        stroke_width: None,
        stroke_alignment: None,
        fill_rule: None,
        opacity: None,
        visible: None,
        locked: None,
        rotate: None,
        style: None,
        points: tri_points(),
        source_span: None,
        unknown_props: BTreeMap::new(),
    })
}

/// Parameters for [`make_connector`], bundled to keep the helper's arity small
/// (and to satisfy the workspace no-`#[allow]` rule on over-long arg lists).
#[derive(Clone, Copy)]
struct ConnectorSpec<'a> {
    id: &'a str,
    from: Option<&'a str>,
    to: Option<&'a str>,
    route: Option<&'a str>,
    marker_end: Option<&'a str>,
    from_anchor: Option<&'a str>,
}

/// A bare connector with caller-supplied `from`/`to` (and optional enum attrs)
/// for driving the validate-time diagnostic paths.
fn make_connector(spec: ConnectorSpec) -> Node {
    Node::Connector(Box::new(ConnectorNode {
        id: spec.id.to_owned(),
        name: None,
        role: None,
        from: spec.from.map(str::to_owned),
        to: spec.to.map(str::to_owned),
        from_anchor: spec.from_anchor.map(str::to_owned),
        to_anchor: None,
        route: spec.route.map(str::to_owned),
        marker_start: None,
        marker_end: spec.marker_end.map(str::to_owned),
        stroke: None,
        stroke_width: None,
        opacity: None,
        visible: None,
        locked: None,
        rotate: None,
        style: None,
        text_style: None,
        label_offset_x: None,
        label_offset_y: None,
        spans: Vec::new(),
        source_span: None,
        unknown_props: BTreeMap::new(),
    }))
}

fn validate_source(src: &str) -> ValidationReport {
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    validate(&doc)
}

/// A geometry-complete `shape` with one label span. Enum-valued attributes
/// (`kind`, `h_align`) are caller-supplied so tests can drive the enum-warning
/// paths in `compile`/`validate`.
fn minimal_shape(id: &str, kind: Option<&str>, h_align: Option<&str>) -> Node {
    Node::Shape(Box::new(ShapeNode {
        id: id.to_owned(),
        name: None,
        role: None,
        x: Some(pxv(0.0)),
        y: Some(pxv(0.0)),
        w: Some(pxv(200.0)),
        h: Some(pxv(120.0)),
        layout_item: Default::default(),
        kind: kind.map(str::to_owned),
        fill: None,
        stroke: None,
        stroke_width: None,
        radius: None,
        stroke_alignment: None,
        padding: None,
        h_align: h_align.map(str::to_owned),
        v_align: None,
        text_style: None,
        spans: vec![TextSpan {
            text: "Label".to_owned(),
            fill: None,
            font_weight: None,
            font_features: None,
            font_alternates: None,
            letter_spacing: None,
            italic: None,
            underline: None,
            strikethrough: None,
            vertical_align: None,
            footnote_ref: None,
            data_ref: None,
            data_format: None,
            highlight: None,
            code: None,
            link: None,
        }],
        style: None,
        shadow: None,
        opacity: None,
        visible: None,
        locked: None,
        rotate: None,
        anchor: None,
        anchor_zone: None,
        anchor_sibling: None,
        anchor_edge: None,
        anchor_gap: None,
        anchor_parent: None,
        source_span: None,
        unknown_props: BTreeMap::new(),
    }))
}

#[path = "validate_shapes/connector.rs"]
mod connector;
#[path = "validate_shapes/polygon.rs"]
mod polygon;
#[path = "validate_shapes/polygon_misc.rs"]
mod polygon_misc;
