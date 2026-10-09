//! `bounded_page` and `rect_at`, shared by the `validate_containers`,
//! `validate_geometry`, and `validate_layout` binaries. Needs the sibling `px`
//! and `pxv_doc_with` modules declared at the binary root.

use super::px::px;
use super::pxv_doc_with::pxv;
use std::collections::BTreeMap;
use zenith_core::{ConstructionBlock, Node, Page, RectNode};

/// Helper: build a page with a given width/height (px) and children.
pub fn bounded_page(id: &str, w: f64, h: f64, children: Vec<Node>) -> Page {
    Page {
        id: id.to_owned(),
        name: None,
        source: None,
        fit: None,
        width: px(w),
        height: px(h),
        background: None,
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
        construction: ConstructionBlock::default(),
        ports: Vec::new(),
        block_styles: Vec::new(),
        defaults: Default::default(),
        children,
        source_span: None,
    }
}

/// Helper: rect at (x, y, w, h) in px, no fill.
pub fn rect_at(id: &str, x: f64, y: f64, w: f64, h: f64) -> Node {
    Node::Rect(Box::new(RectNode {
        shadow: None,
        filter: None,
        mask: None,
        id: id.to_owned(),
        name: None,
        role: None,
        x: Some(pxv(x)),
        y: Some(pxv(y)),
        w: Some(pxv(w)),
        h: Some(pxv(h)),
        layout_item: Default::default(),
        radius: None,
        radius_tl: None,
        radius_tr: None,
        radius_br: None,
        radius_bl: None,
        style: None,
        fill: None,
        stroke: None,
        stroke_width: None,
        stroke_alignment: None,
        stroke_dash: None,
        stroke_gap: None,
        stroke_linecap: None,
        border_top: None,
        border_bottom: None,
        border_left: None,
        border_right: None,
        border_width: None,
        stroke_outer: None,
        stroke_outer_width: None,
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
    }))
}
