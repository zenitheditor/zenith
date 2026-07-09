//! Node-property introspection for the contrast check: geometry-independent
//! helpers that read opacity/visibility/rotation/effects off a [`Node`], build a
//! rect's coverage shape, and resolve color/font-size/font-weight properties.

use std::collections::BTreeMap;

use crate::ast::node::{Node, RectNode, TextNode};
use crate::ast::style::Style;
use crate::ast::value::{Dimension, PropertyValue, dim_to_px};
use crate::color::parse_rgb;
use crate::tokens::{ResolvedToken, ResolvedValue};

use super::geometry::{CoverageShape, RectPx, Rotation, resolve_axis_px};

macro_rules! node_option_field {
    ($node:expr, $field:ident) => {
        match $node {
            Node::Rect(n) => n.$field,
            Node::Ellipse(n) => n.$field,
            Node::Image(n) => n.$field,
            Node::Shape(n) => n.$field,
            Node::Frame(n) => n.$field,
            Node::Group(n) => n.$field,
            Node::Text(n) => n.$field,
            Node::Line(n) => n.$field,
            Node::Code(n) => n.$field,
            Node::Polygon(n) => n.$field,
            Node::Polyline(n) => n.$field,
            Node::Path(n) => n.$field,
            Node::Instance(n) => n.$field,
            Node::Field(n) => n.$field,
            Node::Footnote(_) => None,
            Node::Toc(n) => n.$field,
            Node::Connector(n) => n.$field,
            Node::Pattern(n) => n.$field,
            Node::Chart(n) => n.$field,
            Node::Light(n) => n.$field,
            Node::Mesh(n) => n.$field,
            Node::Table(n) => n.$field,
            Node::Unknown(_) => None,
        }
    };
}

pub(super) fn node_opacity(node: &Node) -> Option<f64> {
    node_option_field!(node, opacity)
}

pub(super) fn node_visible(node: &Node) -> bool {
    node_option_field!(node, visible).unwrap_or(true)
}

/// The rotation angle (degrees) a leaf/container declares, or `None` when there
/// is no non-zero rotation. Read directly as the scene compiler does.
pub(super) fn node_rotate_deg(node: &Node) -> Option<f64> {
    let dim = match node {
        Node::Rect(n) => n.rotate.as_ref(),
        Node::Ellipse(n) => n.rotate.as_ref(),
        Node::Shape(n) => n.rotate.as_ref(),
        Node::Image(n) => n.rotate.as_ref(),
        Node::Polygon(n) => n.rotate.as_ref(),
        Node::Polyline(n) => n.rotate.as_ref(),
        Node::Path(n) => n.rotate.as_ref(),
        Node::Frame(n) => n.rotate.as_ref(),
        Node::Group(n) => n.rotate.as_ref(),
        Node::Text(n) => n.rotate.as_ref(),
        Node::Table(n) => n.rotate.as_ref(),
        Node::Line(_)
        | Node::Code(_)
        | Node::Instance(_)
        | Node::Field(_)
        | Node::Footnote(_)
        | Node::Toc(_)
        | Node::Connector(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Mesh(_)
        | Node::Unknown(_) => None,
    }?;
    Some(dim.value).filter(|v| *v != 0.0)
}

/// The exact rotation of a rotated leaf backdrop about its own box center.
pub(super) fn leaf_rotation(node: &Node, bounds: RectPx) -> Option<Rotation> {
    node_rotate_deg(node).map(|angle_deg| Rotation {
        angle_deg,
        cx: bounds.x + bounds.w / 2.0,
        cy: bounds.y + bounds.h / 2.0,
    })
}

/// Whether a node carries a paint-altering effect the validator cannot sample
/// through: a mask, a filter, a non-zero blur, or a non-normal blend mode.
pub(super) fn candidate_has_effect(node: &Node) -> bool {
    match node {
        Node::Rect(n) => {
            has_effect_fields(&n.mask, &n.filter, n.blur.as_ref(), n.blend_mode.as_deref())
        }
        Node::Ellipse(n) => {
            has_effect_fields(&n.mask, &n.filter, n.blur.as_ref(), n.blend_mode.as_deref())
        }
        Node::Image(n) => {
            has_effect_fields(&n.mask, &n.filter, n.blur.as_ref(), n.blend_mode.as_deref())
        }
        Node::Frame(n) => {
            has_effect_fields(&n.mask, &n.filter, n.blur.as_ref(), n.blend_mode.as_deref())
        }
        Node::Group(n) => {
            has_effect_fields(&n.mask, &n.filter, n.blur.as_ref(), n.blend_mode.as_deref())
        }
        Node::Text(n) => {
            has_effect_fields(&n.mask, &n.filter, n.blur.as_ref(), n.blend_mode.as_deref())
        }
        Node::Shape(_)
        | Node::Polygon(_)
        | Node::Polyline(_)
        | Node::Path(_)
        | Node::Line(_)
        | Node::Code(_)
        | Node::Instance(_)
        | Node::Field(_)
        | Node::Footnote(_)
        | Node::Toc(_)
        | Node::Connector(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Mesh(_)
        | Node::Table(_)
        | Node::Unknown(_) => false,
    }
}

fn has_effect_fields(
    mask: &Option<PropertyValue>,
    filter: &Option<PropertyValue>,
    blur: Option<&Dimension>,
    blend_mode: Option<&str>,
) -> bool {
    mask.is_some()
        || filter.is_some()
        || blur.is_some_and(|d| d.value != 0.0)
        || is_nonnormal_blend(blend_mode)
}

fn is_nonnormal_blend(blend_mode: Option<&str>) -> bool {
    blend_mode.is_some_and(|m| !m.is_empty() && m != "normal")
}

/// A `group`/`frame` ancestor is "unmodeled" when it rotates (a group/frame
/// rotation pivots on the children's union bbox, which is not replicated here) or
/// carries a paint-altering effect. Descendant backdrops become indeterminate.
pub(super) fn container_is_unmodeled(node: &Node) -> bool {
    node_rotate_deg(node).is_some() || candidate_has_effect(node)
}

/// Build the coverage shape for a `rect` backdrop: a plain rectangle, or a
/// rounded rectangle when any (per-corner) radius is set. Radii are resolved to
/// pixels and clamped to the box half-extents so corner arcs never overlap.
pub(super) fn rect_coverage_shape(
    rect: &RectNode,
    page_size: (f64, f64),
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
) -> CoverageShape {
    let (page_w, page_h) = page_size;
    let (Some(w), Some(h)) = (
        resolve_axis_px(rect.w.as_ref(), page_w, resolved_tokens),
        resolve_axis_px(rect.h.as_ref(), page_h, resolved_tokens),
    ) else {
        return CoverageShape::Rect;
    };
    if w <= 0.0 || h <= 0.0 {
        return CoverageShape::Rect;
    }
    let limit = (w.min(h)) / 2.0;
    let uniform = rect.radius.as_ref();
    let corner = |override_value: Option<&PropertyValue>| -> f64 {
        resolve_axis_px(override_value.or(uniform), limit, resolved_tokens)
            .unwrap_or(0.0)
            .clamp(0.0, limit)
    };
    let tl = corner(rect.radius_tl.as_ref());
    let tr = corner(rect.radius_tr.as_ref());
    let br = corner(rect.radius_br.as_ref());
    let bl = corner(rect.radius_bl.as_ref());
    if tl <= 0.0 && tr <= 0.0 && br <= 0.0 && bl <= 0.0 {
        CoverageShape::Rect
    } else {
        CoverageShape::RoundedRect { tl, tr, br, bl }
    }
}

pub(super) fn clip_bounds(clip: Option<RectPx>, bounds: RectPx) -> Option<RectPx> {
    match clip {
        Some(clip) => clip.intersect(bounds),
        None => Some(bounds),
    }
}

pub(super) fn resolve_color_property(
    value: Option<&PropertyValue>,
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
) -> Option<(u8, u8, u8)> {
    let Some(PropertyValue::TokenRef(id)) = value else {
        return None;
    };
    resolved_tokens.get(id.as_str()).and_then(|rt| {
        if let ResolvedValue::Color(hex) = &rt.value {
            parse_rgb(hex)
        } else {
            None
        }
    })
}

pub(super) fn style_property<'a>(
    style: Option<&str>,
    key: &str,
    style_map: &'a BTreeMap<&str, &Style>,
) -> Option<&'a PropertyValue> {
    style_map.get(style?).and_then(|s| s.properties.get(key))
}

pub(super) fn resolve_font_size(
    text: &TextNode,
    style_map: &BTreeMap<&str, &Style>,
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
) -> f64 {
    text.font_size
        .as_ref()
        .or_else(|| style_property(text.style.as_deref(), "font-size", style_map))
        .and_then(|pv| {
            if let PropertyValue::TokenRef(id) = pv {
                resolved_tokens.get(id.as_str()).and_then(|rt| {
                    if let ResolvedValue::Dimension(dim) = &rt.value {
                        dim_to_px(dim.value, &dim.unit)
                    } else {
                        None
                    }
                })
            } else {
                None
            }
        })
        .unwrap_or(16.0)
}

pub(super) fn resolve_font_weight(
    text: &TextNode,
    style_map: &BTreeMap<&str, &Style>,
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
) -> u32 {
    text.font_weight
        .as_ref()
        .or_else(|| style_property(text.style.as_deref(), "font-weight", style_map))
        .and_then(|pv| {
            if let PropertyValue::TokenRef(id) = pv {
                resolved_tokens.get(id.as_str()).and_then(|rt| {
                    if let ResolvedValue::FontWeight(w) = &rt.value {
                        Some(*w)
                    } else {
                        None
                    }
                })
            } else {
                None
            }
        })
        .unwrap_or(400)
}
