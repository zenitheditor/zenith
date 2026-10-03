//! Auto-layout attribute emitters shared by the box-node writers: keyword-aware
//! `w`/`h`, the item attributes, and the `frame` container attributes.

use crate::ast::node::{LayoutContainer, LayoutItem, SizeKeyword};
use crate::ast::value::{Dimension, PropertyValue};

use crate::format::writer::{
    write_opt_bool, write_opt_dimension, write_opt_property_value, write_opt_str_escaped,
};

/// Emit `w`/`h`: the keyword (`w="hug"`) when set, else the dimension value.
pub(super) fn write_box_size(
    out: &mut String,
    key: &str,
    value: &Option<PropertyValue>,
    keyword: Option<SizeKeyword>,
) {
    match keyword {
        Some(k) => write_keyword(out, key, k),
        None => write_opt_property_value(out, key, value),
    }
}

/// Emit an `instance` `w`/`h`: the keyword when set, else the dimension.
pub(super) fn write_box_dimension(
    out: &mut String,
    key: &str,
    value: &Option<Dimension>,
    keyword: Option<SizeKeyword>,
) {
    match keyword {
        Some(k) => write_keyword(out, key, k),
        None => write_opt_dimension(out, key, value),
    }
}

fn write_keyword(out: &mut String, key: &str, k: SizeKeyword) {
    out.push(' ');
    out.push_str(key);
    out.push_str("=\"");
    out.push_str(k.as_str());
    out.push('"');
}

/// Emit the item attributes in canonical order: `min-w`, `max-w`, `min-h`,
/// `max-h`, `position`. Emits nothing for an empty [`LayoutItem`].
pub(super) fn write_layout_item(out: &mut String, item: &LayoutItem) {
    write_opt_property_value(out, "min-w", &item.min_w);
    write_opt_property_value(out, "max-w", &item.max_w);
    write_opt_property_value(out, "min-h", &item.min_h);
    write_opt_property_value(out, "max-h", &item.max_h);
    let position = item.position.as_ref().map(|p| p.as_str().to_owned());
    write_opt_str_escaped(out, "position", &position);
}

/// Emit the frame container attributes in canonical order: `gap`, `wrap-gap`,
/// `padding`, `padding-x`, `padding-y`, `padding-top`, `padding-right`,
/// `padding-bottom`, `padding-left`, `justify`, `align`, `wrap`.
pub(super) fn write_layout_container(out: &mut String, c: &LayoutContainer) {
    write_opt_property_value(out, "gap", &c.gap);
    write_opt_property_value(out, "wrap-gap", &c.wrap_gap);
    write_opt_property_value(out, "padding", &c.padding);
    write_opt_property_value(out, "padding-x", &c.padding_x);
    write_opt_property_value(out, "padding-y", &c.padding_y);
    write_opt_property_value(out, "padding-top", &c.padding_top);
    write_opt_property_value(out, "padding-right", &c.padding_right);
    write_opt_property_value(out, "padding-bottom", &c.padding_bottom);
    write_opt_property_value(out, "padding-left", &c.padding_left);
    let justify = c.justify.as_ref().map(|j| j.as_str().to_owned());
    write_opt_str_escaped(out, "justify", &justify);
    let align = c.align.as_ref().map(|a| a.as_str().to_owned());
    write_opt_str_escaped(out, "align", &align);
    write_opt_bool(out, "wrap", &c.wrap);
}
