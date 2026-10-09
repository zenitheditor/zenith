//! Contrast builders shared by the `validate_contrast` and
//! `validate_contrast_effects` binaries. Needs the sibling `px`, `pxv_doc_with`,
//! `minimal_rect`, and `minimal_text` modules declared at the binary root.

use super::minimal_rect::minimal_rect;
use super::minimal_text::minimal_text;
use super::px::px;
use super::pxv_doc_with::pxv;
use std::collections::BTreeMap;
use zenith_core::{EllipseNode, GroupNode, Node, Page, PropertyValue};

/// Helper: build a page with a background color token reference.
pub fn page_with_bg(id: &str, bg_token_id: &str, children: Vec<Node>) -> Page {
    Page {
        id: id.to_owned(),
        name: None,
        source: None,
        fit: None,
        width: px(1280.0),
        height: px(720.0),
        background: Some(PropertyValue::TokenRef(bg_token_id.to_owned())),
        bleed: None,
        margin_inner: None,
        margin_outer: None,
        margin_top: None,
        margin_bottom: None,
        baseline_grid: None,
        line_jumps: None,
        parity: None,
        master: None,
        safe_zones: Vec::new(),
        folds: Vec::new(),
        construction: zenith_core::ConstructionBlock::default(),
        ports: Vec::new(),
        block_styles: Vec::new(),
        defaults: Default::default(),
        children,
        source_span: None,
    }
}

/// Build a filled ellipse backdrop large enough for a centered text box.
pub fn ellipse_backdrop(id: &str, fill_token: &str) -> Node {
    Node::Ellipse(EllipseNode {
        shadow: None,
        filter: None,
        mask: None,
        id: id.to_owned(),
        name: None,
        role: None,
        x: Some(pxv(520.0)),
        y: Some(pxv(300.0)),
        w: Some(pxv(240.0)),
        h: Some(pxv(120.0)),
        layout_item: Default::default(),
        rx: None,
        ry: None,
        style: None,
        fill: Some(PropertyValue::TokenRef(fill_token.to_owned())),
        stroke: None,
        stroke_width: None,
        stroke_dash: None,
        stroke_gap: None,
        stroke_linecap: None,
        opacity: None,
        visible: None,
        locked: None,
        rotate: None,
        blend_mode: None,
        blur: None,
        anchor: None,
        anchor_zone: None,
        anchor_sibling: None,
        anchor_edge: None,
        anchor_gap: None,
        anchor_parent: None,
        source_span: None,
        unknown_props: BTreeMap::new(),
    })
}

pub fn rect_backdrop_at(id: &str, fill_token: &str, x: f64, y: f64, w: f64, h: f64) -> Node {
    rect_backdrop_at_with_opacity(id, fill_token, x, y, w, h, None)
}

pub fn rect_backdrop_at_with_opacity(
    id: &str,
    fill_token: &str,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    opacity: Option<f64>,
) -> Node {
    let Node::Rect(mut rect) =
        minimal_rect(id, Some(PropertyValue::TokenRef(fill_token.to_owned())))
    else {
        unreachable!("minimal_rect returns Node::Rect");
    };
    rect.x = Some(pxv(x));
    rect.y = Some(pxv(y));
    rect.w = Some(pxv(w));
    rect.h = Some(pxv(h));
    rect.opacity = opacity;
    Node::Rect(rect)
}

pub fn group_at(id: &str, x: f64, y: f64, children: Vec<Node>) -> Node {
    group_at_with_opacity(id, x, y, None, children)
}

pub fn group_at_with_opacity(
    id: &str,
    x: f64,
    y: f64,
    opacity: Option<f64>,
    children: Vec<Node>,
) -> Node {
    Node::Group(GroupNode {
        id: id.to_owned(),
        name: None,
        role: None,
        x: Some(pxv(x)),
        y: Some(pxv(y)),
        w: None,
        h: None,
        layout_item: Default::default(),
        opacity,
        visible: None,
        locked: None,
        rotate: None,
        blend_mode: None,
        shadow: None,
        filter: None,
        mask: None,
        blur: None,
        style: None,
        semantic_role: None,
        intensity: None,
        layer_priority: None,
        symmetry_count: None,
        symmetry_cx: None,
        symmetry_cy: None,
        symmetry_start_angle: None,
        symmetry_mode: None,
        anchor: None,
        anchor_zone: None,
        anchor_sibling: None,
        anchor_edge: None,
        anchor_gap: None,
        anchor_parent: None,
        children,
        protected_regions: Vec::new(),
        editable_param_ids: Vec::new(),
        source_span: None,
        unknown_props: BTreeMap::new(),
    })
}

pub fn text_at(id: &str, fill_token: &str, x: f64, y: f64, w: f64, h: f64) -> Node {
    let Node::Text(mut text) =
        minimal_text(id, Some(PropertyValue::TokenRef(fill_token.to_owned())))
    else {
        unreachable!("minimal_text returns Node::Text");
    };
    text.x = Some(pxv(x));
    text.y = Some(pxv(y));
    text.w = Some(pxv(w));
    text.h = Some(pxv(h));
    Node::Text(text)
}
