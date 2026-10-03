//! Auto-layout attribute transforms shared by every box-node kind: keyword
//! `w`/`h` sizes, the item attributes (`min-*`, `max-*`, `position`), and the
//! `frame` container attributes.

use kdl::{KdlNode, KdlValue};

use crate::ast::node::{
    LayoutAlign, LayoutContainer, LayoutItem, LayoutJustify, LayoutPosition, SizeKeyword,
};
use crate::ast::value::PropertyValue;

use super::helpers::{
    entry_annotation, optional_bool_prop, optional_property_value, optional_string_prop,
};

/// The keyword form of `key` (`w="hug"`), when the entry is an unannotated
/// string that names a [`SizeKeyword`].
fn size_keyword(node: &KdlNode, key: &str) -> Option<SizeKeyword> {
    let entry = node.entry(key)?;
    if entry_annotation(entry).is_some() {
        return None;
    }
    match entry.value() {
        KdlValue::String(s) => SizeKeyword::from_attr(s),
        KdlValue::Integer(_) | KdlValue::Float(_) | KdlValue::Bool(_) | KdlValue::Null => None,
    }
}

/// Read a box `w`/`h`: `None` when the value is a size keyword (the keyword is
/// stored on [`LayoutItem`]), else the usual [`PropertyValue`].
pub(super) fn optional_box_size(node: &KdlNode, key: &str) -> Option<PropertyValue> {
    if size_keyword(node, key).is_some() {
        return None;
    }
    optional_property_value(node, key)
}

/// Read the per-child item attributes of a box node.
pub(super) fn transform_layout_item(node: &KdlNode) -> LayoutItem {
    LayoutItem {
        w_keyword: size_keyword(node, "w"),
        h_keyword: size_keyword(node, "h"),
        min_w: optional_property_value(node, "min-w"),
        max_w: optional_property_value(node, "max-w"),
        min_h: optional_property_value(node, "min-h"),
        max_h: optional_property_value(node, "max-h"),
        position: optional_string_prop(node, "position").map(LayoutPosition::from_attr),
    }
}

/// Read the container attributes of a `frame`.
pub(super) fn transform_layout_container(node: &KdlNode) -> LayoutContainer {
    LayoutContainer {
        gap: optional_property_value(node, "gap"),
        wrap_gap: optional_property_value(node, "wrap-gap"),
        padding: optional_property_value(node, "padding"),
        padding_x: optional_property_value(node, "padding-x"),
        padding_y: optional_property_value(node, "padding-y"),
        padding_top: optional_property_value(node, "padding-top"),
        padding_right: optional_property_value(node, "padding-right"),
        padding_bottom: optional_property_value(node, "padding-bottom"),
        padding_left: optional_property_value(node, "padding-left"),
        justify: optional_string_prop(node, "justify").map(LayoutJustify::from_attr),
        align: optional_string_prop(node, "align").map(LayoutAlign::from_attr),
        wrap: optional_bool_prop(node, "wrap"),
    }
}
