//! `minimal_rect`, shared by the `validate_*` test binaries. Needs the sibling
//! `px` and `pxv_doc_with` modules declared at the binary root.

use super::pxv_doc_with::pxv;
use std::collections::BTreeMap;
use zenith_core::{Node, PropertyValue, RectNode};

pub fn minimal_rect(id: &str, fill: Option<PropertyValue>) -> Node {
    Node::Rect(Box::new(RectNode {
        shadow: None,
        filter: None,
        mask: None,
        id: id.to_owned(),
        name: None,
        role: None,
        x: Some(pxv(0.0)),
        y: Some(pxv(0.0)),
        w: Some(pxv(100.0)),
        h: Some(pxv(100.0)),
        layout_item: Default::default(),
        radius: None,
        radius_tl: None,
        radius_tr: None,
        radius_br: None,
        radius_bl: None,
        style: None,
        fill,
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
