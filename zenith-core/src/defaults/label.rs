//! Lowering of `shape` and `connector`: body attributes plus the owned label.
//!
//! The label renders through its `text-style`, so a label default cascades
//! into that style: node `text-style` > page default `text-style` > document
//! default `text-style`, per property. The label fill is the ambient pair
//! (the shape's own fill pairs its label) when the node `text-style` sets no
//! fill, ahead of the default `text-style` fill.

use crate::ast::defaults::DefaultsKind;
use crate::ast::node::{ConnectorNode, ShapeNode};
use crate::ast::value::PropertyValue;
use crate::schema::enums::H_ALIGNS;

use super::apply::{put, put_enum, put_h_align};
use super::lower::Cx;
use super::scope::{Cascade, MergeExtras, Scope, literal};

pub(super) fn shape(
    n: &mut ShapeNode,
    cx: &mut Cx<'_>,
    scope: Scope<'_>,
    pair: Option<&PropertyValue>,
) {
    let c = scope.cascade(DefaultsKind::Shape, n.style.as_deref());
    put(&mut n.fill, &c, "fill");
    put(&mut n.stroke, &c, "stroke");
    put(&mut n.stroke_width, &c, "stroke-width");
    put(&mut n.radius, &c, "radius");
    put(&mut n.shadow, &c, "shadow");
    put_enum(&mut n.v_align, &c, "v-align");
    if n.spans.is_empty() {
        return;
    }

    let fill = n.fill.as_ref().or_else(|| c.node_value("fill"));
    let label_pair = cx.child_pair(scope, fill, pair);
    let labels = scope.label_cascade(DefaultsKind::Shape, n.text_style.as_deref());
    // The label align order is: `h-align`, node `text-style` align, node
    // style align (an `h-align` value), default label align, default style
    // align. A node style align therefore keeps the default label align out.
    let node_style_align = c
        .node_value("align")
        .and_then(literal)
        .filter(|v| H_ALIGNS.contains(&v.as_str()));
    let skip_default = node_style_align.as_ref().map(|_| "align");
    let label_has_align =
        labels.node_has("align") || (skip_default.is_none() && labels.fallback("align").is_some());
    lower_label(
        &mut n.text_style,
        &labels,
        label_pair.as_ref(),
        skip_default,
        cx,
    );
    if node_style_align.is_none() && !label_has_align {
        put_h_align(&mut n.h_align, &c);
    }
}

pub(super) fn connector(
    n: &mut ConnectorNode,
    cx: &mut Cx<'_>,
    scope: Scope<'_>,
    pair: Option<&PropertyValue>,
) {
    let c = scope.cascade(DefaultsKind::Connector, n.style.as_deref());
    put(&mut n.stroke, &c, "stroke");
    put(&mut n.stroke_width, &c, "stroke-width");
    if n.spans.is_empty() {
        return;
    }
    let labels = scope.label_cascade(DefaultsKind::Connector, n.text_style.as_deref());
    lower_label(&mut n.text_style, &labels, pair, None, cx);
}

/// Point `text_style` at the style that expresses the label cascade, when
/// a default label style or the content pair applies.
fn lower_label(
    text_style: &mut Option<String>,
    labels: &Cascade<'_>,
    pair: Option<&PropertyValue>,
    skip_default: Option<&str>,
    cx: &mut Cx<'_>,
) {
    let pair = pair.filter(|_| !labels.node_has("fill"));
    if !labels.has_defaults() && pair.is_none() {
        return;
    }
    let extras = MergeExtras {
        fill: pair,
        skip_default: skip_default.filter(|_| labels.has_defaults()),
    };
    *text_style = cx.synth.style_for("label", labels, extras);
}
