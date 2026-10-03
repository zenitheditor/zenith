//! The per-page box recorder and the declared box of a compiled node.

use std::cell::RefCell;
use std::collections::BTreeMap;

use zenith_core::{Node, PropertyValue};

use crate::ir::SceneCommand;
use crate::layout::LayoutBox;

use super::super::ctx::NodeCtx;
use super::super::field::resolve_field_to_text;
use super::super::pipeline::RenderCtx;
use super::super::text::ShapeEnv;
use super::super::toc::resolve_toc_to_text;
use super::super::util::resolve_geometry_px;
use super::bounds::{Affine, map_box, open_transform, painted};

/// The final geometry of one compiled node, in page-absolute px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompiledBox {
    /// The unrotated box the compile used.
    pub rect: LayoutBox,
    /// The node's own rotation in degrees, when set and non-zero.
    pub rotate: Option<f64>,
    /// The axis-aligned bounds of what the node paints, every transform
    /// applied; `rect` (rotated by `rotate`) when nothing paints.
    pub visual: LayoutBox,
}

/// Collects the final box of every compiled node of one page.
#[derive(Debug)]
pub(in crate::compile) struct BoxRecorder {
    boxes: RefCell<BTreeMap<String, CompiledBox>>,
    /// The transform open where this recorder's command stream starts.
    base: Affine,
}

impl Default for BoxRecorder {
    fn default() -> Self {
        Self {
            boxes: RefCell::new(BTreeMap::new()),
            base: Affine::IDENTITY,
        }
    }
}

/// What [`BoxRecorder::record`] needs from one `compile_node` call.
#[derive(Clone, Copy)]
pub(in crate::compile) struct Compiled<'a> {
    /// The compiled node.
    pub(in crate::compile) node: &'a Node,
    /// The node's compile context.
    pub(in crate::compile) cx: NodeCtx<'a>,
    /// The node's render context.
    pub(in crate::compile) ctx: RenderCtx,
    /// The whole command stream the node emitted into.
    pub(in crate::compile) commands: &'a [SceneCommand],
    /// Index of the node's first command in `commands`.
    pub(in crate::compile) start: usize,
    /// The measured content height (`text` / `code`), else `0.0`.
    pub(in crate::compile) content_h: f64,
}

/// One node placed into a command stream, in render space.
#[derive(Clone, Copy)]
pub(in crate::compile) struct Placed<'a> {
    pub(in crate::compile) id: &'a str,
    /// The unrotated render-space box, when the node declares one.
    pub(in crate::compile) declared: Option<LayoutBox>,
    /// The node's own rotation in degrees.
    pub(in crate::compile) rotate: Option<f64>,
    /// The whole command stream the node emitted into.
    pub(in crate::compile) commands: &'a [SceneCommand],
    /// Index of the node's first command in `commands`.
    pub(in crate::compile) start: usize,
    /// Render position of page `(0, 0)`.
    pub(in crate::compile) page_origin: (f64, f64),
    /// Fonts for glyph ink.
    pub(in crate::compile) shape: ShapeEnv<'a>,
}

impl BoxRecorder {
    /// A recorder for a separate command stream that is spliced into this
    /// recorder's stream after `outer` (the commands emitted so far).
    pub(in crate::compile) fn nested(&self, outer: &[SceneCommand]) -> BoxRecorder {
        BoxRecorder {
            boxes: RefCell::new(BTreeMap::new()),
            base: open_transform(self.base, outer),
        }
    }

    /// Move every record of `child` into this recorder under `prefix` + id.
    pub(in crate::compile) fn absorb(&self, child: BoxRecorder, prefix: &str) {
        let mut boxes = self.boxes.borrow_mut();
        for (id, b) in child.boxes.into_inner() {
            boxes.entry(format!("{prefix}{id}")).or_insert(b);
        }
    }

    /// The recorded boxes, by id.
    pub(in crate::compile) fn into_boxes(self) -> BTreeMap<String, CompiledBox> {
        self.boxes.into_inner()
    }

    /// Record the box of one compiled node.
    pub(in crate::compile) fn record(&self, c: Compiled<'_>) {
        let Some(id) = c.node.id() else {
            return;
        };
        if self.boxes.borrow().contains_key(id) {
            return;
        }
        let resolved_text = match c.node {
            Node::Field(f) => {
                resolve_field_to_text(f, c.cx.field_ctx).map(|t| Node::Text(Box::new(t)))
            }
            Node::Toc(t) => resolve_toc_to_text(
                t,
                c.cx.field_ctx.pages,
                c.cx.field_ctx.page_index_by_node_id,
            )
            .map(|t| Node::Text(Box::new(t))),
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Text(_)
            | Node::Code(_)
            | Node::Frame(_)
            | Node::Group(_)
            | Node::Image(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Instance(_)
            | Node::Footnote(_)
            | Node::Table(_)
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Unknown(_) => None,
        };
        let source = resolved_text.as_ref().unwrap_or(c.node);
        self.place(Placed {
            id,
            declared: declared_box(source, id, c),
            rotate: node_rotate(source),
            commands: c.commands,
            start: c.start,
            page_origin: c.ctx.page_origin,
            shape: ShapeEnv {
                engine: c.cx.engine,
                fonts: c.cx.fonts,
            },
        });
    }

    /// Record one placed node (first record of an id wins).
    pub(in crate::compile) fn place(&self, p: Placed<'_>) {
        if self.boxes.borrow().contains_key(p.id) {
            return;
        }
        let prefix = open_transform(self.base, p.commands.get(..p.start).unwrap_or_default());
        let own = p.commands.get(p.start..).unwrap_or_default();
        let Some(rect) = p
            .declared
            .map(|b| map_box(prefix, b))
            .or_else(|| painted(own, prefix, true, p.shape))
        else {
            return;
        };
        let visual =
            painted(own, prefix, false, p.shape).unwrap_or_else(|| match (p.declared, p.rotate) {
                (Some(b), Some(deg)) => {
                    let spin = Affine::rotate_at(deg, b.x + b.w / 2.0, b.y + b.h / 2.0);
                    map_box(prefix.then(spin), b)
                }
                _ => rect,
            });
        let (ox, oy) = p.page_origin;
        let page = |b: LayoutBox| LayoutBox {
            x: b.x - ox,
            y: b.y - oy,
            ..b
        };
        self.boxes.borrow_mut().insert(
            p.id.to_owned(),
            CompiledBox {
                rect: page(rect),
                rotate: p.rotate,
                visual: page(visual),
            },
        );
    }
}

/// The node's own rotation in degrees, when set and non-zero.
fn node_rotate(node: &Node) -> Option<f64> {
    let dim = match node {
        Node::Rect(n) => n.rotate.as_ref(),
        Node::Ellipse(n) => n.rotate.as_ref(),
        Node::Frame(n) => n.rotate.as_ref(),
        Node::Image(n) => n.rotate.as_ref(),
        Node::Text(n) => n.rotate.as_ref(),
        Node::Code(n) => n.rotate.as_ref(),
        Node::Group(n) => n.rotate.as_ref(),
        Node::Polygon(n) => n.rotate.as_ref(),
        Node::Polyline(n) => n.rotate.as_ref(),
        Node::Path(n) => n.rotate.as_ref(),
        Node::Table(n) => n.rotate.as_ref(),
        Node::Shape(n) => n.rotate.as_ref(),
        Node::Connector(n) => n.rotate.as_ref(),
        Node::Pattern(n) => n.rotate.as_ref(),
        Node::Chart(n) => n.rotate.as_ref(),
        Node::Line(_)
        | Node::Instance(_)
        | Node::Field(_)
        | Node::Toc(_)
        | Node::Footnote(_)
        | Node::Light(_)
        | Node::Mesh(_)
        | Node::Unknown(_) => None,
    }?;
    (dim.value.is_finite() && dim.value != 0.0).then_some(dim.value)
}

/// The render-space box a box node compiles at, when its origin and size
/// resolve. `None` for the kinds whose box comes from their commands.
fn declared_box(node: &Node, id: &str, c: Compiled<'_>) -> Option<LayoutBox> {
    let measured = match node {
        Node::Text(_) | Node::Code(_) => (c.content_h > 0.0).then_some(c.content_h),
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Frame(_)
        | Node::Group(_)
        | Node::Image(_)
        | Node::Field(_)
        | Node::Toc(_)
        | Node::Table(_)
        | Node::Shape(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Mesh(_) => None,
        Node::Line(_)
        | Node::Polygon(_)
        | Node::Polyline(_)
        | Node::Path(_)
        | Node::Instance(_)
        | Node::Footnote(_)
        | Node::Connector(_)
        | Node::Light(_)
        | Node::Unknown(_) => return None,
    };
    let view = node.box_view()?;
    let px = |pv: Option<&PropertyValue>| resolve_geometry_px(pv, c.cx.resolved);
    let anchor = c.cx.anchors.get(id).copied();
    let x = px(view.x).or(anchor.map(|a| a.0))?;
    let y = px(view.y).or(anchor.map(|a| a.1))?;
    let w = px(view.w)?;
    let h = px(view.h).or(measured)?;
    Some(LayoutBox {
        x: x + c.ctx.dx,
        y: y + c.ctx.dy,
        w,
        h,
    })
}
