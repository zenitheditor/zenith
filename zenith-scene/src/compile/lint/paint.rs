//! Paint facts of one node for the page ledger: the opaque region it covers,
//! whether it draws through an effect, and the authored placement facts.

use std::collections::BTreeMap;

use zenith_core::{
    AssetDecl, Diagnostic, Dimension, LayoutPosition, Node, PropertyValue, ResolvedToken, Style,
    dim_to_px,
};

use crate::layout::LayoutBox;

use super::super::paint::{resolve_property_color, resolve_property_gradient};
use super::super::{resolve_property_dimension_px, style_prop};
use super::geom::Coverage;
use super::ledger::Occluder;

/// The document facts paint resolution reads.
#[derive(Clone, Copy)]
pub(in crate::compile) struct PaintEnv<'a> {
    pub(in crate::compile) resolved: &'a BTreeMap<String, ResolvedToken>,
    pub(in crate::compile) style_map: &'a BTreeMap<&'a str, &'a Style>,
    pub(in crate::compile) assets: &'a [AssetDecl],
}

/// How the authored document places one node.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Authored {
    /// The authored `y` when it is a literal dimension, in px.
    pub(super) y_px: Option<f64>,
    /// Any `anchor*` attribute is set.
    pub(super) anchored: bool,
    /// A `row` / `column` / `grid` frame places the node.
    pub(super) in_flow: bool,
}

/// The placement facts of every id-bearing node on an authored page.
pub(super) fn authored_facts(children: &[Node]) -> BTreeMap<String, Authored> {
    let mut out = BTreeMap::new();
    collect_authored(children, false, &mut out);
    out
}

fn collect_authored(nodes: &[Node], placed_by_parent: bool, out: &mut BTreeMap<String, Authored>) {
    for node in nodes {
        let view = node.box_view();
        let absolute =
            view.is_some_and(|v| matches!(v.layout_item.position, Some(LayoutPosition::Absolute)));
        if let Some(id) = node.id() {
            out.entry(id.to_owned()).or_insert(Authored {
                y_px: view.and_then(|v| match v.y {
                    Some(PropertyValue::Dimension(d)) => dim_to_px(d.value, &d.unit),
                    Some(
                        PropertyValue::TokenRef(_)
                        | PropertyValue::Literal(_)
                        | PropertyValue::DataRef(_),
                    )
                    | None => None,
                }),
                anchored: view.is_some_and(|v| v.anchored),
                in_flow: placed_by_parent && !absolute,
            });
        }
        match node {
            Node::Frame(f) => {
                let places = f.layout.as_ref().is_some_and(|l| l.positions_children());
                collect_authored(&f.children, places, out);
            }
            Node::Group(g) => collect_authored(&g.children, false, out),
            Node::Unknown(u) => collect_authored(&u.children, false, out),
            Node::Table(t) => {
                for row in &t.rows {
                    for cell in &row.cells {
                        collect_authored(&cell.children, true, out);
                    }
                }
            }
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
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_) => {}
        }
    }
}

/// `true` when the node draws through a blend layer, mask, filter, or blur:
/// its paint, and its children's, is not plain source-over paint.
pub(super) fn own_effects(node: &Node) -> bool {
    let effect = |blend: &Option<String>, mask: bool, filter: bool, blur: &Option<Dimension>| {
        blend.as_deref().is_some_and(|b| b != "normal")
            || mask
            || filter
            || blur.as_ref().is_some_and(|d| d.value != 0.0)
    };
    match node {
        Node::Rect(n) => effect(&n.blend_mode, n.mask.is_some(), n.filter.is_some(), &n.blur),
        Node::Ellipse(n) => effect(&n.blend_mode, n.mask.is_some(), n.filter.is_some(), &n.blur),
        Node::Frame(n) => effect(&n.blend_mode, n.mask.is_some(), n.filter.is_some(), &n.blur),
        Node::Group(n) => effect(&n.blend_mode, n.mask.is_some(), n.filter.is_some(), &n.blur),
        Node::Image(n) => effect(&n.blend_mode, n.mask.is_some(), n.filter.is_some(), &n.blur),
        Node::Text(n) => effect(&n.blend_mode, n.mask.is_some(), n.filter.is_some(), &n.blur),
        Node::Line(_)
        | Node::Code(_)
        | Node::Polygon(_)
        | Node::Polyline(_)
        | Node::Path(_)
        | Node::Instance(_)
        | Node::Field(_)
        | Node::Toc(_)
        | Node::Footnote(_)
        | Node::Table(_)
        | Node::Shape(_)
        | Node::Connector(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Mesh(_)
        | Node::Unknown(_) => false,
    }
}

/// The opaque region `node` paints over its unrotated box `rect`: a solid
/// fill with full alpha (`rect`, `ellipse`, `frame`, `shape`), or a JPEG
/// image that fills its box (`fit` other than `contain` / `none`). PNG,
/// WebP, and SVG images can hold transparency and never count.
pub(super) fn occluder_of(node: &Node, rect: LayoutBox, env: PaintEnv<'_>) -> Option<Occluder> {
    let radius = |own: &Option<PropertyValue>, style: &Option<String>| {
        let prop = own
            .as_ref()
            .or_else(|| style_prop(style, env.style_map, "radius"));
        resolve_property_dimension_px(prop, env.resolved, 0.0)
    };
    let rounded = |r: f64| {
        if r > 0.0 {
            Coverage::Rounded(r)
        } else {
            Coverage::Rect
        }
    };
    let shape = match node {
        Node::Rect(n) => {
            opaque_fill(&n.id, &n.fill, &n.style, env)?;
            let corner = [&n.radius_tl, &n.radius_tr, &n.radius_br, &n.radius_bl]
                .into_iter()
                .map(|p| resolve_property_dimension_px(p.as_ref(), env.resolved, 0.0))
                .fold(0.0_f64, f64::max);
            rounded(radius(&n.radius, &n.style).max(corner))
        }
        Node::Ellipse(n) => {
            opaque_fill(&n.id, &n.fill, &n.style, env)?;
            Coverage::Ellipse
        }
        Node::Frame(n) => {
            opaque_fill(&n.id, &n.fill, &n.style, env)?;
            rounded(radius(&n.radius, &n.style))
        }
        Node::Shape(n) => {
            opaque_fill(&n.id, &n.fill, &n.style, env)?;
            match n.kind.as_deref() {
                Some("ellipse") => Coverage::Ellipse,
                Some("decision") => Coverage::Diamond,
                Some("terminator") => Coverage::Rounded(rect.h / 2.0),
                _ => rounded(radius(&n.radius, &n.style)),
            }
        }
        Node::Image(n) => {
            let src = env
                .assets
                .iter()
                .find(|a| a.id == n.asset)?
                .src
                .to_ascii_lowercase();
            let jpeg = src.ends_with(".jpg") || src.ends_with(".jpeg");
            let fills = !matches!(n.fit.as_deref(), Some("contain" | "none"));
            if !(jpeg && fills) {
                return None;
            }
            match n.clip.as_deref() {
                Some("ellipse" | "circle") => Coverage::Ellipse,
                Some("rounded") => rounded(resolve_property_dimension_px(
                    n.clip_radius.as_ref(),
                    env.resolved,
                    0.0,
                )),
                _ => Coverage::Rect,
            }
        }
        Node::Line(_)
        | Node::Text(_)
        | Node::Code(_)
        | Node::Group(_)
        | Node::Polygon(_)
        | Node::Polyline(_)
        | Node::Path(_)
        | Node::Instance(_)
        | Node::Field(_)
        | Node::Toc(_)
        | Node::Footnote(_)
        | Node::Table(_)
        | Node::Connector(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Mesh(_)
        | Node::Unknown(_) => return None,
    };
    (rect.w > 0.0 && rect.h > 0.0).then_some(Occluder {
        region: rect,
        shape,
    })
}

/// `Some(())` when the fill (own, else the style's) is a solid colour with
/// full alpha.
fn opaque_fill(
    id: &str,
    fill: &Option<PropertyValue>,
    style: &Option<String>,
    env: PaintEnv<'_>,
) -> Option<()> {
    let prop = fill
        .as_ref()
        .or_else(|| style_prop(style, env.style_map, "fill"))?;
    if resolve_property_gradient(prop, env.resolved, id).is_some() {
        return None;
    }
    let mut scratch: Vec<Diagnostic> = Vec::new();
    let color = resolve_property_color(prop, env.resolved, &mut scratch, id)?;
    (color.a == u8::MAX).then_some(())
}
