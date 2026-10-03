//! Per-kind lowering: write each default style value a node kind consumes
//! as an explicit attribute.
//!
//! Each function lists the style keys the scene compiler reads from the node
//! style of that kind, so a lowered attribute reproduces exactly what the
//! style would have painted.

use crate::ast::node::{
    ChartNode, CodeNode, EllipseNode, FieldNode, FootnoteNode, FrameNode, GroupNode, ImageNode,
    LineNode, PathNode, PatternNode, PolygonNode, PolylineNode, RectNode, TableNode, TextNode,
    TocNode,
};
use crate::ast::value::PropertyValue;
use crate::schema::enums::H_ALIGNS;

use super::scope::{Cascade, MergeExtras, Synth};

/// Style keys a `field`, `footnote`, or `toc` reads through its style with no
/// attribute of its own. A default for any of them lowers into the node style.
const STYLE_ONLY_TEXT_KEYS: &[&str] = &["font-weight", "letter-spacing", "shadow", "v-align"];

/// Fill `slot` with the default value of `key` when the node sets neither.
pub(super) fn put(slot: &mut Option<PropertyValue>, cascade: &Cascade<'_>, key: &str) {
    if slot.is_none()
        && let Some(value) = cascade.fallback(key)
    {
        *slot = Some(value.clone());
    }
}

/// Fill an enum `slot` (`align`, `v-align`) with the default literal of `key`.
pub(super) fn put_enum(slot: &mut Option<String>, cascade: &Cascade<'_>, key: &str) {
    if slot.is_none()
        && let Some(value) = cascade.fallback_enum(key)
    {
        *slot = Some(value);
    }
}

/// Fill an `h-align` slot from the default `align`, kept only when it is an
/// `h-align` value (`justify` is not; the scene ignores it).
pub(super) fn put_h_align(slot: &mut Option<String>, cascade: &Cascade<'_>) {
    if slot.is_none()
        && let Some(value) = cascade.fallback_enum("align")
        && H_ALIGNS.contains(&value.as_str())
    {
        *slot = Some(value);
    }
}

/// Fill a text `fill` slot: the ambient `pair`, else the default fill. Skipped
/// when the node or its own style sets a fill.
pub(super) fn put_text_fill(
    slot: &mut Option<PropertyValue>,
    cascade: &Cascade<'_>,
    pair: Option<&PropertyValue>,
) {
    if slot.is_some() || cascade.node_has("fill") {
        return;
    }
    match pair {
        Some(pair) => *slot = Some(pair.clone()),
        None => put(slot, cascade, "fill"),
    }
}

/// Fill, stroke, and stroke width.
fn paint(
    fill: Option<&mut Option<PropertyValue>>,
    stroke: &mut Option<PropertyValue>,
    stroke_width: &mut Option<PropertyValue>,
    cascade: &Cascade<'_>,
) {
    if let Some(fill) = fill {
        put(fill, cascade, "fill");
    }
    put(stroke, cascade, "stroke");
    put(stroke_width, cascade, "stroke-width");
}

pub(super) fn rect(n: &mut RectNode, c: &Cascade<'_>) {
    paint(Some(&mut n.fill), &mut n.stroke, &mut n.stroke_width, c);
    put(&mut n.radius, c, "radius");
    put(&mut n.shadow, c, "shadow");
}

pub(super) fn ellipse(n: &mut EllipseNode, c: &Cascade<'_>) {
    paint(Some(&mut n.fill), &mut n.stroke, &mut n.stroke_width, c);
    put(&mut n.shadow, c, "shadow");
}

pub(super) fn line(n: &mut LineNode, c: &Cascade<'_>) {
    paint(None, &mut n.stroke, &mut n.stroke_width, c);
}

pub(super) fn polygon(n: &mut PolygonNode, c: &Cascade<'_>) {
    paint(Some(&mut n.fill), &mut n.stroke, &mut n.stroke_width, c);
}

pub(super) fn polyline(n: &mut PolylineNode, c: &Cascade<'_>) {
    paint(Some(&mut n.fill), &mut n.stroke, &mut n.stroke_width, c);
}

pub(super) fn path(n: &mut PathNode, c: &Cascade<'_>) {
    paint(Some(&mut n.fill), &mut n.stroke, &mut n.stroke_width, c);
}

pub(super) fn text(n: &mut TextNode, c: &Cascade<'_>, pair: Option<&PropertyValue>) {
    put_text_fill(&mut n.fill, c, pair);
    put(&mut n.font_family, c, "font-family");
    put(&mut n.font_size, c, "font-size");
    put(&mut n.font_weight, c, "font-weight");
    put(&mut n.letter_spacing, c, "letter-spacing");
    put(&mut n.shadow, c, "shadow");
    put_enum(&mut n.align, c, "align");
    put_enum(&mut n.v_align, c, "v-align");
}

/// A `code` block carries its own syntax colours, so it takes no content
/// pair: only the default style's fill.
pub(super) fn code(n: &mut CodeNode, c: &Cascade<'_>) {
    put(&mut n.fill, c, "fill");
    put(&mut n.font_family, c, "font-family");
    put(&mut n.font_size, c, "font-size");
    put(&mut n.font_weight, c, "font-weight");
    put(&mut n.letter_spacing, c, "letter-spacing");
}

pub(super) fn frame(n: &mut FrameNode, c: &Cascade<'_>) {
    paint(Some(&mut n.fill), &mut n.stroke, &mut n.stroke_width, c);
    put(&mut n.radius, c, "radius");
    put(&mut n.shadow, c, "shadow");
    put(&mut n.container.padding, c, "padding");
    put(&mut n.container.gap, c, "gap");
}

pub(super) fn group(n: &mut GroupNode, c: &Cascade<'_>) {
    put(&mut n.shadow, c, "shadow");
}

pub(super) fn image(n: &mut ImageNode, c: &Cascade<'_>) {
    put(&mut n.shadow, c, "shadow");
}

pub(super) fn pattern(n: &mut PatternNode, c: &Cascade<'_>) {
    put(&mut n.shadow, c, "shadow");
}

pub(super) fn chart(n: &mut ChartNode, c: &Cascade<'_>) {
    put(&mut n.shadow, c, "shadow");
}

pub(super) fn table(n: &mut TableNode, c: &Cascade<'_>) {
    put_h_align(&mut n.h_align, c);
    put_enum(&mut n.v_align, c, "v-align");
}

/// The text-bearing attributes a `field`, `footnote`, and `toc` share.
struct TextSlots<'n> {
    style: &'n mut Option<String>,
    fill: &'n mut Option<PropertyValue>,
    font_family: &'n mut Option<PropertyValue>,
    font_size: &'n mut Option<PropertyValue>,
}

fn text_slots(
    role: &str,
    slots: TextSlots<'_>,
    c: &Cascade<'_>,
    pair: Option<&PropertyValue>,
    synth: &mut Synth,
) {
    put_text_fill(slots.fill, c, pair);
    put(slots.font_family, c, "font-family");
    put(slots.font_size, c, "font-size");
    if STYLE_ONLY_TEXT_KEYS
        .iter()
        .any(|key| c.fallback(key).is_some())
    {
        *slots.style = synth.style_for(role, c, MergeExtras::default());
    }
}

pub(super) fn field(
    n: &mut FieldNode,
    c: &Cascade<'_>,
    pair: Option<&PropertyValue>,
    synth: &mut Synth,
) {
    let slots = TextSlots {
        style: &mut n.style,
        fill: &mut n.fill,
        font_family: &mut n.font_family,
        font_size: &mut n.font_size,
    };
    text_slots("field", slots, c, pair, synth);
}

pub(super) fn footnote(
    n: &mut FootnoteNode,
    c: &Cascade<'_>,
    pair: Option<&PropertyValue>,
    synth: &mut Synth,
) {
    let slots = TextSlots {
        style: &mut n.style,
        fill: &mut n.fill,
        font_family: &mut n.font_family,
        font_size: &mut n.font_size,
    };
    text_slots("footnote", slots, c, pair, synth);
}

pub(super) fn toc(
    n: &mut TocNode,
    c: &Cascade<'_>,
    pair: Option<&PropertyValue>,
    synth: &mut Synth,
) {
    let slots = TextSlots {
        style: &mut n.style,
        fill: &mut n.fill,
        font_family: &mut n.font_family,
        font_size: &mut n.font_size,
    };
    text_slots("toc", slots, c, pair, synth);
}
