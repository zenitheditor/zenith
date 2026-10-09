//! The `edit` part of `node.inspect`: which `node.set` fields apply to a
//! node and their current values.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;
use zenith_core::schema::node_attributes;
use zenith_core::{Document, Node};
use zenith_tx::node_token_properties;

use super::attrs::Attribute;
use super::style::StyleSource;
use crate::commands::set::{SpanOut, plain_text, spans_of};
use crate::gesture::current_axes;
use crate::gesture::facts::{Axis, AxisValue, box_facts};
use crate::gesture::kind::{Kind, rotate_blocked};
use crate::gesture::target::Target;

/// What `node.set` can write on one node, with the current values.
#[derive(Debug, Serialize)]
pub(crate) struct Editable {
    /// The `node.set` fields that apply, in display order.
    fields: Vec<&'static str>,
    /// Authored px of the box axes, as `node.set` takes them.
    #[serde(skip_serializing_if = "Option::is_none")]
    x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    y: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    w: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    h: Option<f64>,
    /// What each box axis holds: `length`, `token`, `absent`, `keyword`,
    /// or `unresolved`; plus `anchor` when an anchor places an absent
    /// position, and `flow` when a layout frame places it.
    axes: BTreeMap<&'static str, &'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rotate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    opacity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fill: Option<Bound>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stroke: Option<Bound>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stroke_width: Option<Bound>,
    #[serde(skip_serializing_if = "Option::is_none")]
    radius: Option<Bound>,
    #[serde(skip_serializing_if = "Option::is_none")]
    font_family: Option<Bound>,
    #[serde(skip_serializing_if = "Option::is_none")]
    font_size: Option<Bound>,
    #[serde(skip_serializing_if = "Option::is_none")]
    font_weight: Option<Bound>,
    /// The text alignment: the node's own, else its style's, else `start`.
    #[serde(skip_serializing_if = "Option::is_none")]
    align: Option<String>,
    /// The content of a `text` node with one plain span.
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    /// The spans of a `text` or `shape` node without one plain span.
    #[serde(skip_serializing_if = "Option::is_none")]
    spans: Option<Vec<SpanOut>>,
    /// The node's own `visible` and `locked`.
    #[serde(skip_serializing_if = "Option::is_none")]
    visible: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    locked: Option<bool>,
}

/// A token-backed property: the token it is bound to (with the resolved
/// value), another written value, or, when the node writes none, the value
/// of the style it draws with.
#[derive(Debug, Serialize)]
struct Bound {
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,
    /// The bound token's resolved value.
    #[serde(skip_serializing_if = "Option::is_none")]
    resolved: Option<Value>,
    /// The style's `{token, resolved}` when the node writes no value.
    #[serde(skip_serializing_if = "Option::is_none")]
    style: Option<Value>,
}

/// The editable fields of the node `t` in `doc`, the parse of the current
/// valid text. `attributes` are the node's own attributes; `style` is the
/// style it draws with.
pub(crate) fn editable(
    doc: &Document,
    t: &Target<'_>,
    attributes: &[Attribute],
    style: Option<&StyleSource>,
) -> Editable {
    let node = t.located.node;
    let kind = node.kind_str();
    let known = node_attributes(kind);
    let mut out = Editable {
        fields: Vec::new(),
        x: None,
        y: None,
        w: None,
        h: None,
        axes: BTreeMap::new(),
        rotate: None,
        opacity: None,
        fill: None,
        stroke: None,
        stroke_width: None,
        radius: None,
        font_family: None,
        font_size: None,
        font_weight: None,
        align: None,
        text: None,
        spans: None,
        visible: None,
        locked: None,
    };
    if Kind::of(node) == Kind::Box
        && let Some(facts) = box_facts(doc, node, &t.id)
    {
        for (axis, value) in current_axes(&facts, t) {
            let Some(value) = value else { continue };
            let held = match facts.value(axis) {
                AxisValue::Absent if !axis.is_size() && facts.flow.is_some() => "flow",
                AxisValue::Absent if facts.anchor_supplies(axis) => "anchor",
                AxisValue::Length => "length",
                AxisValue::Token(_) => "token",
                AxisValue::Absent => "absent",
                AxisValue::Keyword(_) => "keyword",
                AxisValue::Unresolved(_) => "unresolved",
            };
            out.axes.insert(axis.name(), held);
            out.fields.push(axis.name());
            match axis {
                Axis::X => out.x = Some(value),
                Axis::Y => out.y = Some(value),
                Axis::W => out.w = Some(value),
                Axis::H => out.h = Some(value),
            }
        }
    }
    if rotate_blocked(node).is_none() {
        out.fields.push("rotate");
        out.rotate = Some(node.rotate().map_or(0.0, |d| d.value));
    }
    if known.contains(&"opacity") {
        out.fields.push("opacity");
        out.opacity = Some(node.opacity().unwrap_or(1.0));
    }
    for (name, field, slot) in [
        ("fill", "fill", &mut out.fill),
        ("stroke", "stroke", &mut out.stroke),
        ("stroke-width", "stroke_width", &mut out.stroke_width),
    ] {
        let target = if name == "stroke-width" {
            "stroke"
        } else {
            name
        };
        if paints(node, target) && known.contains(&name) {
            out.fields.push(field);
            *slot = Some(bound(attributes, name, style));
        }
    }
    let tokens = node_token_properties(node);
    for (name, field, slot) in [
        ("radius", "radius", &mut out.radius),
        ("font-family", "font_family", &mut out.font_family),
        ("font-size", "font_size", &mut out.font_size),
        ("font-weight", "font_weight", &mut out.font_weight),
    ] {
        if tokens.contains(&name) && known.contains(&name) {
            out.fields.push(field);
            *slot = Some(bound(attributes, name, style));
        }
    }
    if let Node::Text(text) = node {
        out.fields.push("align");
        let from_style = style
            .and_then(|s| s.properties.get("align"))
            .and_then(|v| v.get("value"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        out.align = Some(
            text.align
                .clone()
                .or(from_style)
                .unwrap_or_else(|| "start".to_owned()),
        );
    }
    if let Some(text) = plain_text(node) {
        out.fields.push("text");
        out.text = Some(text.to_owned());
    } else if let Some(spans) = spans_of(node) {
        out.fields.push("spans");
        out.spans = Some(spans);
    }
    out.fields.push("visible");
    out.visible = Some(node.is_visible());
    out.fields.push("locked");
    out.locked = Some(node.is_locked());
    out
}

/// `true` when `set_fill` / `set_stroke` write `name` on `node`.
fn paints(node: &Node, name: &str) -> bool {
    // (fill, stroke), as the transaction engine's property slots have them.
    let (fill, stroke) = match node {
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Polygon(_)
        | Node::Polyline(_)
        | Node::Path(_)
        | Node::Shape(_)
        | Node::Pattern(_)
        | Node::Chart(_) => (true, true),
        Node::Text(_)
        | Node::Code(_)
        | Node::Field(_)
        | Node::Toc(_)
        | Node::Footnote(_)
        | Node::Table(_) => (true, false),
        Node::Line(_) | Node::Connector(_) | Node::Mesh(_) => (false, true),
        Node::Frame(_)
        | Node::Group(_)
        | Node::Image(_)
        | Node::Instance(_)
        | Node::Light(_)
        | Node::Unknown(_) => (false, false),
    };
    match name {
        "fill" => fill,
        "stroke" => stroke,
        _ => false,
    }
}

/// The written `name` attribute as a [`Bound`], or the style's value when
/// the node writes none.
fn bound(attributes: &[Attribute], name: &str, style: Option<&StyleSource>) -> Bound {
    let Some(a) = attributes.iter().find(|a| a.name == name) else {
        return Bound {
            token: None,
            value: None,
            resolved: None,
            style: style.and_then(|s| s.properties.get(name)).cloned(),
        };
    };
    match &a.token {
        Some(binding) => Bound {
            token: Some(binding.id.clone()),
            value: None,
            resolved: binding.value.clone(),
            style: None,
        },
        None => Bound {
            token: None,
            value: Some(match &a.value {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            }),
            resolved: None,
            style: None,
        },
    }
}
