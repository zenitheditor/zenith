//! Authored text facts for the legibility checks: where a `text` node's
//! `x` / `y` / `font-size` come from, and every authored font size with the
//! top-level group it sits in.

use std::collections::BTreeMap;

use zenith_core::{Node, PropertyValue, Span};

use super::super::{resolve_property_dimension_px, style_prop};
use super::paint::{PaintEnv, literal_px};

/// Authored facts of one `text` node.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct TextFacts {
    /// The authored `x` when it is a literal dimension, in px.
    pub(super) x_px: Option<f64>,
    /// The authored `y` when it is a literal dimension, in px.
    pub(super) y_px: Option<f64>,
    /// `font-size` is a literal dimension on the node itself.
    pub(super) size_literal: bool,
    /// The token `font-size` resolves through, on the node or its style.
    pub(super) size_token: Option<String>,
}

/// One authored font size and the node that uses it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct SizeUse {
    /// Id of the top-level `frame` / `group` holding the node; empty for a
    /// node placed on the page itself.
    pub(super) scope: String,
    pub(super) node: String,
    pub(super) span: Option<Span>,
    /// The size, in px.
    pub(super) px: f64,
    /// The token id the size comes from, `literal` for a literal dimension.
    pub(super) source: String,
}

/// The text facts and authored sizes of one page.
#[derive(Debug, Default)]
pub(super) struct PageText {
    pub(super) facts: BTreeMap<String, TextFacts>,
    pub(super) sizes: Vec<SizeUse>,
}

/// Collect the facts of every `text` node under `children`. Sizes skip guide
/// and hidden nodes, decoration and background roles, and chart, table, and
/// code internals.
pub(super) fn collect(children: &[Node], paint: PaintEnv<'_>) -> PageText {
    let mut out = PageText::default();
    let walk = Walk {
        paint,
        scope: "",
        sized: true,
    };
    for node in children {
        let scope = match node {
            Node::Frame(_) | Node::Group(_) => node.id().unwrap_or(""),
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Text(_)
            | Node::Code(_)
            | Node::Image(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Instance(_)
            | Node::Field(_)
            | Node::Toc(_)
            | Node::Footnote(_)
            | Node::Table(_)
            | Node::Connector(_)
            | Node::Shape(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Unknown(_) => "",
        };
        visit(node, Walk { scope, ..walk }, &mut out);
    }
    out
}

/// What the walk carries into a node.
#[derive(Clone, Copy)]
struct Walk<'a> {
    paint: PaintEnv<'a>,
    scope: &'a str,
    /// Texts here count toward the type scale.
    sized: bool,
}

fn visit(node: &Node, walk: Walk<'_>, out: &mut PageText) {
    if node.role() == Some("guide") || !node.is_visible() {
        return;
    }
    let walk = if node.is_decorative() {
        Walk {
            sized: false,
            ..walk
        }
    } else {
        walk
    };
    match node {
        Node::Text(t) => text(node, t.font_size.as_ref(), &t.style, walk, out),
        Node::Frame(f) => {
            for child in &f.children {
                visit(child, walk, out);
            }
        }
        Node::Group(g) => {
            for child in &g.children {
                visit(child, walk, out);
            }
        }
        Node::Unknown(u) => {
            for child in &u.children {
                visit(child, walk, out);
            }
        }
        Node::Table(t) => {
            let inner = Walk {
                sized: false,
                ..walk
            };
            for row in &t.rows {
                for cell in &row.cells {
                    for child in &cell.children {
                        visit(child, inner, out);
                    }
                }
            }
        }
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Line(_)
        | Node::Code(_)
        | Node::Image(_)
        | Node::Polygon(_)
        | Node::Polyline(_)
        | Node::Path(_)
        | Node::Instance(_)
        | Node::Field(_)
        | Node::Toc(_)
        | Node::Footnote(_)
        | Node::Connector(_)
        | Node::Shape(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Mesh(_) => {}
    }
}

fn text(
    node: &Node,
    font_size: Option<&PropertyValue>,
    style: &Option<String>,
    walk: Walk<'_>,
    out: &mut PageText,
) {
    let Some(id) = node.id() else {
        return;
    };
    let view = node.box_view();
    let size = font_size.or_else(|| style_prop(style, walk.paint.style_map, "font-size"));
    let size_token = match size {
        Some(PropertyValue::TokenRef(t)) => Some(t.clone()),
        Some(
            PropertyValue::Literal(_) | PropertyValue::Dimension(_) | PropertyValue::DataRef(_),
        )
        | None => None,
    };
    let size_literal = literal_px(font_size).is_some();
    out.facts.entry(id.to_owned()).or_insert(TextFacts {
        x_px: literal_px(view.as_ref().and_then(|v| v.x)),
        y_px: literal_px(view.as_ref().and_then(|v| v.y)),
        size_literal,
        size_token: size_token.clone(),
    });
    if !walk.sized {
        return;
    }
    let px = resolve_property_dimension_px(size, walk.paint.resolved, 0.0);
    if px <= 0.0 {
        return;
    }
    out.sizes.push(SizeUse {
        scope: walk.scope.to_owned(),
        node: id.to_owned(),
        span: node.source_span(),
        px,
        source: size_token.unwrap_or_else(|| "literal".to_owned()),
    });
}

/// A px value for a message: whole numbers without a decimal point.
pub(super) fn fmt_px(value: f64) -> String {
    if (value - value.round()).abs() < 0.05 {
        format!("{:.0}", value.round())
    } else {
        format!("{value:.1}")
    }
}
