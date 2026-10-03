//! Natural width and height-at-width of the node kinds that size from their
//! content: `text`, `code`, `field`, `toc`, `shape` (label), and `table`.
//!
//! Every measure compiles or shapes into scratch buffers. Its diagnostics are
//! dropped: the real compile reports them once.

use std::collections::BTreeMap;

use zenith_core::{BlockStyle, CodeNode, Diagnostic, Node, ShapeNode, TextNode, dim_to_px};

use crate::ir::SceneCommand;

use super::super::RenderCtx;
use super::super::anchor::AnchorMap;
use super::super::chain::ChainAssignments;
use super::super::field::resolve_field_to_text;
use super::super::leaf::label_text_node;
use super::super::markdown_resolve::MdBlockMap;
use super::super::table::table_natural_size;
use super::super::text::{
    MeasureEnv, ShapeEnv, TextCompileEnv, compile_code, compile_text, empty_md_blocks, ink_bounds,
    measure_text_natural, measure_text_wrapped_height, resolve_text_families,
};
use super::super::toc::resolve_toc_to_text;
use super::super::util::{px_prop, resolve_property_dimension_px};
use super::IntrinsicEnv;

impl IntrinsicEnv<'_> {
    /// The node's natural (unwrapped) content width in px.
    ///
    /// `None` for a kind without intrinsic size (`rect`, `image`, `chart`, a
    /// chained `text`, a `shape` without a label, …). A `field` or `toc` that
    /// resolves to nothing measures `0`.
    pub(crate) fn natural_width(&self, node: &Node) -> Option<f64> {
        match node {
            Node::Text(t) => (t.chain.is_none()).then(|| self.text_natural(t)),
            Node::Code(c) => Some(self.code_natural(c)),
            Node::Field(f) => Some(
                resolve_field_to_text(f, self.field_ctx).map_or(0.0, |t| self.text_natural(&t)),
            ),
            Node::Toc(t) => Some(
                resolve_toc_to_text(
                    t,
                    self.field_ctx.pages,
                    self.field_ctx.page_index_by_node_id,
                )
                .map_or(0.0, |t| self.text_natural(&t)),
            ),
            Node::Shape(s) => self.label_natural(s),
            Node::Table(t) => Some(table_natural_size(t, None, self.measure_env()).0),
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Frame(_)
            | Node::Group(_)
            | Node::Image(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Instance(_)
            | Node::Footnote(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Unknown(_) => None,
        }
    }

    /// The node's content height in px when laid out `width` px wide.
    ///
    /// `None` exactly when [`IntrinsicEnv::natural_width`] is `None`.
    pub(crate) fn height_at(&self, node: &Node, width: f64) -> Option<f64> {
        match node {
            Node::Text(t) => (t.chain.is_none()).then(|| self.text_height(t, width)),
            Node::Code(c) => Some(self.code_height(c, width)),
            Node::Field(f) => Some(
                resolve_field_to_text(f, self.field_ctx)
                    .map_or(0.0, |t| self.text_height(&t, width)),
            ),
            Node::Toc(t) => Some(
                resolve_toc_to_text(
                    t,
                    self.field_ctx.pages,
                    self.field_ctx.page_index_by_node_id,
                )
                .map_or(0.0, |t| self.text_height(&t, width)),
            ),
            Node::Shape(s) => self.label_height(s, width),
            Node::Table(t) => Some(table_natural_size(t, Some(width), self.measure_env()).1),
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Frame(_)
            | Node::Group(_)
            | Node::Image(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Instance(_)
            | Node::Footnote(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Unknown(_) => None,
        }
    }

    fn measure_env(&self) -> MeasureEnv<'_> {
        MeasureEnv {
            resolved: self.resolved,
            style_map: self.style_map,
            fonts: self.fonts,
            engine: self.engine,
        }
    }

    /// Widest unwrapped line plus the hanging `padding-left`.
    fn text_natural(&self, text: &TextNode) -> f64 {
        let mut scratch: Vec<Diagnostic> = Vec::new();
        let families = resolve_text_families(
            text,
            self.resolved,
            self.style_map,
            self.fonts,
            &mut scratch,
        );
        let natural =
            measure_text_natural(text, &families, self.measure_env(), &mut scratch).unwrap_or(0.0);
        let padding_left = text
            .padding_left
            .as_ref()
            .and_then(|d| dim_to_px(d.value, &d.unit))
            .unwrap_or(0.0)
            .max(0.0);
        natural + padding_left
    }

    /// The height `compile_text` lays the node out at, `width` px wide.
    fn text_height(&self, text: &TextNode, width: f64) -> f64 {
        let mut probe = text.clone();
        probe.x = Some(px_prop(0.0));
        probe.y = Some(px_prop(0.0));
        probe.w = Some(px_prop(width));
        probe.h = None;
        let chains = ChainAssignments::new();
        let anchors = AnchorMap::new();
        let boxes: BTreeMap<String, (f64, f64, f64, f64)> = BTreeMap::new();
        let env = self.scratch_env(
            &chains,
            &anchors,
            &boxes,
            self.md_blocks,
            (self.page_block_styles, self.doc_block_styles),
        );
        let mut commands: Vec<SceneCommand> = Vec::new();
        let mut scratch: Vec<Diagnostic> = Vec::new();
        compile_text(
            &probe,
            env,
            &mut commands,
            &mut scratch,
            RenderCtx::measure(),
        )
    }

    /// A text compile environment over empty scratch maps.
    fn scratch_env<'e>(
        &'e self,
        chains: &'e ChainAssignments,
        anchors: &'e AnchorMap,
        boxes: &'e BTreeMap<String, (f64, f64, f64, f64)>,
        md_blocks: &'e MdBlockMap,
        (page_block_styles, doc_block_styles): (&'e [BlockStyle], &'e [BlockStyle]),
    ) -> TextCompileEnv<'e> {
        TextCompileEnv {
            resolved: self.resolved,
            style_map: self.style_map,
            fonts: self.fonts,
            engine: self.engine,
            chains,
            footnote_markers: self.field_ctx.footnote_markers,
            node_boxes: boxes,
            anchors,
            md_blocks,
            page_block_styles,
            doc_block_styles,
        }
    }

    fn code_env<'e>(
        &'e self,
        chains: &'e ChainAssignments,
        anchors: &'e AnchorMap,
        boxes: &'e BTreeMap<String, (f64, f64, f64, f64)>,
    ) -> TextCompileEnv<'e> {
        self.scratch_env(chains, anchors, boxes, empty_md_blocks(), (&[], &[]))
    }

    /// The right edge of the code's ink at its origin (gutter included).
    fn code_natural(&self, code: &CodeNode) -> f64 {
        let mut probe = code.clone();
        probe.x = Some(px_prop(0.0));
        probe.y = Some(px_prop(0.0));
        probe.w = None;
        probe.h = None;
        let chains = ChainAssignments::new();
        let anchors = AnchorMap::new();
        let boxes = BTreeMap::new();
        let mut commands: Vec<SceneCommand> = Vec::new();
        let mut scratch: Vec<Diagnostic> = Vec::new();
        let _ = compile_code(
            &probe,
            self.code_env(&chains, &anchors, &boxes),
            &mut commands,
            &mut scratch,
            RenderCtx::measure(),
        );
        ink_bounds(
            &commands,
            ShapeEnv {
                engine: self.engine,
                fonts: self.fonts,
            },
        )
        .map_or(0.0, |ink| ink.right.max(0.0))
    }

    /// The height `compile_code` lays the node out at, `width` px wide.
    fn code_height(&self, code: &CodeNode, width: f64) -> f64 {
        let mut probe = code.clone();
        probe.x = Some(px_prop(0.0));
        probe.y = Some(px_prop(0.0));
        probe.w = Some(px_prop(width));
        probe.h = None;
        let chains = ChainAssignments::new();
        let anchors = AnchorMap::new();
        let boxes = BTreeMap::new();
        let mut commands: Vec<SceneCommand> = Vec::new();
        let mut scratch: Vec<Diagnostic> = Vec::new();
        compile_code(
            &probe,
            self.code_env(&chains, &anchors, &boxes),
            &mut commands,
            &mut scratch,
            RenderCtx::measure(),
        )
    }

    fn label_pad(&self, shape: &ShapeNode) -> f64 {
        resolve_property_dimension_px(shape.padding.as_ref(), self.resolved, 0.0).max(0.0)
    }

    /// Natural label width plus both paddings; `None` without a label.
    fn label_natural(&self, shape: &ShapeNode) -> Option<f64> {
        if shape.spans.is_empty() {
            return None;
        }
        let pad = self.label_pad(shape);
        let label = label_text_node(shape, (0.0, 0.0, 0.0, 0.0));
        let mut scratch: Vec<Diagnostic> = Vec::new();
        let families = resolve_text_families(
            &label,
            self.resolved,
            self.style_map,
            self.fonts,
            &mut scratch,
        );
        let natural = measure_text_natural(&label, &families, self.measure_env(), &mut scratch)
            .unwrap_or(0.0);
        Some(natural + 2.0 * pad)
    }

    /// Wrapped label height at the content width plus both paddings.
    fn label_height(&self, shape: &ShapeNode, width: f64) -> Option<f64> {
        if shape.spans.is_empty() {
            return None;
        }
        let pad = self.label_pad(shape);
        let content_w = (width - 2.0 * pad).max(0.0);
        let label = label_text_node(shape, (0.0, 0.0, content_w, 0.0));
        let mut scratch: Vec<Diagnostic> = Vec::new();
        let families = resolve_text_families(
            &label,
            self.resolved,
            self.style_map,
            self.fonts,
            &mut scratch,
        );
        let h = measure_text_wrapped_height(
            &label,
            content_w,
            &families,
            self.measure_env(),
            &mut scratch,
        )
        .unwrap_or(0.0);
        Some(h + 2.0 * pad)
    }
}
