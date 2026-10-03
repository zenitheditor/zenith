//! `set_layout`: set or clear a node's auto-layout attributes.
//!
//! Every value is checked before the node changes, so a rejected op leaves
//! the node untouched. Container fields need a `frame`; item fields need a
//! node that carries layout item attributes (box kinds and `instance`).

use zenith_core::schema::enums::{LAYOUT_ALIGNS, LAYOUT_JUSTIFIES, LAYOUT_KINDS, LAYOUT_POSITIONS};
use zenith_core::{
    Diagnostic, Document, LayoutAlign, LayoutJustify, LayoutKind, LayoutPosition, Node,
    PropertyValue, Unit,
};

use super::super::structure::parse_dimension_str;
use super::super::{find_node_any_mut, px, record_affected};
use crate::op::{LayoutDim, LayoutEdit};

/// A checked field change: `None` leaves it, `Some(None)` clears it,
/// `Some(Some(v))` sets it.
type Change<T> = Option<Option<T>>;

/// Every checked change of one `set_layout` op.
struct Checked {
    layout: Change<LayoutKind>,
    gap: Change<PropertyValue>,
    wrap_gap: Change<PropertyValue>,
    padding: Change<PropertyValue>,
    padding_x: Change<PropertyValue>,
    padding_y: Change<PropertyValue>,
    padding_top: Change<PropertyValue>,
    padding_right: Change<PropertyValue>,
    padding_bottom: Change<PropertyValue>,
    padding_left: Change<PropertyValue>,
    justify: Change<LayoutJustify>,
    align: Change<LayoutAlign>,
    wrap: Change<bool>,
    clip: Change<bool>,
    position: Change<LayoutPosition>,
    min_w: Change<PropertyValue>,
    max_w: Change<PropertyValue>,
    min_h: Change<PropertyValue>,
    max_h: Change<PropertyValue>,
}

/// Collects `tx.invalid_value` errors while the fields are checked.
struct Check<'a> {
    node: &'a str,
    diagnostics: &'a mut Vec<Diagnostic>,
    failed: bool,
}

impl Check<'_> {
    fn invalid(&mut self, field: &str, detail: String) {
        self.failed = true;
        self.diagnostics.push(Diagnostic::error(
            "tx.invalid_value",
            format!("set_layout: {field} on node {:?} {detail}", self.node),
            None,
            Some(self.node.to_owned()),
        ));
    }

    /// Check an enum field against its allowed values.
    fn word<T>(
        &mut self,
        field: &str,
        value: &Option<Option<String>>,
        allowed: &[&str],
        parse: fn(&str) -> T,
    ) -> Change<T> {
        match value {
            None => None,
            Some(None) => Some(None),
            Some(Some(v)) if allowed.contains(&v.as_str()) => Some(Some(parse(v))),
            Some(Some(v)) => {
                self.invalid(
                    field,
                    format!(
                        "is {v:?}; use one of {}, or null to clear",
                        allowed.join(", ")
                    ),
                );
                None
            }
        }
    }

    /// Check a dimension field: a px number, a `(px)` / `(pt)` string, or a
    /// dimension token id.
    fn dim(&mut self, field: &str, value: &Option<Option<LayoutDim>>) -> Change<PropertyValue> {
        let input = match value {
            None => return None,
            Some(None) => return Some(None),
            Some(Some(v)) => v,
        };
        match dim_value(input) {
            Some(pv) => Some(Some(pv)),
            None => {
                let shown = match input {
                    LayoutDim::Px(v) => v.to_string(),
                    LayoutDim::Text(s) => format!("{s:?}"),
                };
                self.invalid(
                    field,
                    format!(
                        "is {shown}; pass a px number, a \"(px)N\" or \"(pt)N\" string, \
                         a dimension token id, or null to clear"
                    ),
                );
                None
            }
        }
    }
}

/// The stored value of a layout dimension input, or `None` when it is not a
/// finite px number, a px/pt dimension string, or a token id.
fn dim_value(input: &LayoutDim) -> Option<PropertyValue> {
    match input {
        LayoutDim::Px(v) if v.is_finite() => Some(PropertyValue::Dimension(px(*v))),
        LayoutDim::Px(_) => None,
        LayoutDim::Text(s) => {
            if let Some(id) = s.strip_prefix("(token)") {
                return token_id(id);
            }
            if s.starts_with('(') {
                let d = parse_dimension_str(s)?;
                return match d.unit {
                    Unit::Px | Unit::Pt => Some(PropertyValue::Dimension(d)),
                    Unit::Pct | Unit::Deg | Unit::Unknown(_) => None,
                };
            }
            token_id(s)
        }
    }
}

/// A token ref for a non-empty id without whitespace.
fn token_id(id: &str) -> Option<PropertyValue> {
    let id = id.trim_matches('"');
    if id.is_empty() || id.chars().any(char::is_whitespace) {
        return None;
    }
    Some(PropertyValue::TokenRef(id.to_owned()))
}

/// Check every field of `edit`; `None` after pushing at least one error.
fn check(edit: &LayoutEdit, diagnostics: &mut Vec<Diagnostic>) -> Option<Checked> {
    let mut c = Check {
        node: &edit.node,
        diagnostics,
        failed: false,
    };
    let checked = Checked {
        layout: c.word("layout", &edit.layout, LAYOUT_KINDS, LayoutKind::from_attr),
        gap: c.dim("gap", &edit.gap),
        wrap_gap: c.dim("wrap_gap", &edit.wrap_gap),
        padding: c.dim("padding", &edit.padding),
        padding_x: c.dim("padding_x", &edit.padding_x),
        padding_y: c.dim("padding_y", &edit.padding_y),
        padding_top: c.dim("padding_top", &edit.padding_top),
        padding_right: c.dim("padding_right", &edit.padding_right),
        padding_bottom: c.dim("padding_bottom", &edit.padding_bottom),
        padding_left: c.dim("padding_left", &edit.padding_left),
        justify: c.word(
            "justify",
            &edit.justify,
            LAYOUT_JUSTIFIES,
            LayoutJustify::from_attr,
        ),
        align: c.word("align", &edit.align, LAYOUT_ALIGNS, LayoutAlign::from_attr),
        wrap: edit.wrap,
        clip: edit.clip,
        position: c.word(
            "position",
            &edit.position,
            LAYOUT_POSITIONS,
            LayoutPosition::from_attr,
        ),
        min_w: c.dim("min_w", &edit.min_w),
        max_w: c.dim("max_w", &edit.max_w),
        min_h: c.dim("min_h", &edit.min_h),
        max_h: c.dim("max_h", &edit.max_h),
    };
    (!c.failed).then_some(checked)
}

/// Names of the container fields `edit` sets or clears.
fn container_fields(edit: &LayoutEdit) -> Vec<&'static str> {
    [
        ("layout", edit.layout.is_some()),
        ("gap", edit.gap.is_some()),
        ("wrap_gap", edit.wrap_gap.is_some()),
        ("padding", edit.padding.is_some()),
        ("padding_x", edit.padding_x.is_some()),
        ("padding_y", edit.padding_y.is_some()),
        ("padding_top", edit.padding_top.is_some()),
        ("padding_right", edit.padding_right.is_some()),
        ("padding_bottom", edit.padding_bottom.is_some()),
        ("padding_left", edit.padding_left.is_some()),
        ("justify", edit.justify.is_some()),
        ("align", edit.align.is_some()),
        ("wrap", edit.wrap.is_some()),
        ("clip", edit.clip.is_some()),
    ]
    .into_iter()
    .filter_map(|(name, set)| set.then_some(name))
    .collect()
}

/// Names of the item fields `edit` sets or clears.
fn item_fields(edit: &LayoutEdit) -> Vec<&'static str> {
    [
        ("position", edit.position.is_some()),
        ("min_w", edit.min_w.is_some()),
        ("max_w", edit.max_w.is_some()),
        ("min_h", edit.min_h.is_some()),
        ("max_h", edit.max_h.is_some()),
    ]
    .into_iter()
    .filter_map(|(name, set)| set.then_some(name))
    .collect()
}

/// Apply a change to one slot.
fn put<T>(slot: &mut Option<T>, change: Change<T>) {
    if let Some(v) = change {
        *slot = v;
    }
}

/// Apply `set_layout` to `doc`.
pub(in crate::engine) fn apply_set_layout(
    edit: &LayoutEdit,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    let node_id = edit.node.as_str();
    let container = container_fields(edit);
    let item = item_fields(edit);
    if container.is_empty() && item.is_empty() {
        diagnostics.push(Diagnostic::advisory(
            "tx.noop",
            format!("set_layout on {node_id:?} specified no fields; document is unchanged"),
            None,
            Some(node_id.to_owned()),
        ));
        return;
    }
    let Some(node) = find_node_any_mut(doc, node_id) else {
        diagnostics.push(Diagnostic::error(
            "tx.unknown_node",
            format!("set_layout: node {node_id:?} not found in document"),
            None,
            Some(node_id.to_owned()),
        ));
        return;
    };
    let kind = node.kind_str();
    let mut rejected = false;
    if !container.is_empty() && !matches!(node, Node::Frame(_)) {
        rejected = true;
        diagnostics.push(Diagnostic::error(
            "tx.wrong_node_type",
            format!(
                "set_layout: {} apply to frame nodes only, and node {node_id:?} is a {kind}; \
                 target a frame or remove these fields",
                container.join(", ")
            ),
            None,
            Some(node_id.to_owned()),
        ));
    }
    if !item.is_empty() && node.layout_item().is_none() {
        rejected = true;
        diagnostics.push(Diagnostic::error(
            "tx.unsupported_property",
            format!(
                "set_layout: {} apply to box nodes and instances only, and node {node_id:?} \
                 is a {kind}; target a box node or remove these fields",
                item.join(", ")
            ),
            None,
            Some(node_id.to_owned()),
        ));
    }
    let Some(c) = check(edit, diagnostics) else {
        return;
    };
    if rejected {
        return;
    }
    if let Node::Frame(f) = &mut *node {
        put(&mut f.layout, c.layout);
        let k = &mut f.container;
        put(&mut k.gap, c.gap);
        put(&mut k.wrap_gap, c.wrap_gap);
        put(&mut k.padding, c.padding);
        put(&mut k.padding_x, c.padding_x);
        put(&mut k.padding_y, c.padding_y);
        put(&mut k.padding_top, c.padding_top);
        put(&mut k.padding_right, c.padding_right);
        put(&mut k.padding_bottom, c.padding_bottom);
        put(&mut k.padding_left, c.padding_left);
        put(&mut k.justify, c.justify);
        put(&mut k.align, c.align);
        put(&mut k.wrap, c.wrap);
        put(&mut f.clip, c.clip);
    }
    if let Some(li) = node.layout_item_mut() {
        put(&mut li.position, c.position);
        put(&mut li.min_w, c.min_w);
        put(&mut li.max_w, c.max_w);
        put(&mut li.min_h, c.min_h);
        put(&mut li.max_h, c.max_h);
    }
    record_affected(node_id, affected);
}
