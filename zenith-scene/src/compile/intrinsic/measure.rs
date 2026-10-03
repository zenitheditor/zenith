//! Natural width and height-at-width of the node kinds that size from their
//! content: `text`, `code`, `field`, `toc`, `shape` (label), `table`, and
//! `image` (asset pixel size).
//!
//! Every measure compiles or shapes into scratch buffers. Its diagnostics are
//! dropped: the real compile reports them once. A text probe compiles under
//! the render context and runaround boxes the final compile uses, at its
//! previous-pass position when its height depends on its position.

use zenith_core::{
    BlockStyle, CodeNode, Diagnostic, ImageNode, Node, ShapeNode, TextNode, dim_to_px,
};

use crate::ir::SceneCommand;

use super::super::RenderCtx;
use super::super::anchor::AnchorMap;
use super::super::chain::ChainAssignments;
use super::super::field::resolve_field_to_text;
use super::super::leaf::label_text_node;
use super::super::markdown_resolve::MdBlockMap;
use super::super::table::table_natural_size;
use super::super::text::{
    CodeExtent, MeasureEnv, TextCompileEnv, compile_code, compile_text, empty_md_blocks,
    measure_text_natural, measure_text_wrapped_height, resolve_text_families,
};
use super::super::toc::resolve_toc_to_text;
use super::super::util::{px_prop, resolve_geometry_px, resolve_property_dimension_px};
use super::{IntrinsicEnv, ProbeAt};

impl IntrinsicEnv<'_> {
    /// The node's natural (unwrapped) content width in px.
    ///
    /// `None` for a kind without intrinsic size (`rect`, `chart`, a chained
    /// `text`, a `shape` without a label, an `image` whose asset size is
    /// unknown, …). A `field` or `toc` that resolves to nothing measures `0`.
    /// An `image` with a fixed `h` keeps its asset's aspect ratio. The layout
    /// engine sizes an `instance` from [`IntrinsicEnv::instance_bounds`].
    pub(crate) fn natural_width(&self, node: &Node) -> Option<f64> {
        match node {
            Node::Text(t) => (t.chain.is_none()).then(|| self.text_natural(t)),
            Node::Code(c) => Some(self.code_extent(c, None).width),
            Node::Image(i) => self.image_natural_width(i),
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
    /// `at` is where a position-dependent text draws, when the layout knows
    /// it; otherwise the probe uses the previous pass's position, if any.
    /// `None` exactly when [`IntrinsicEnv::natural_width`] is `None`.
    pub(crate) fn height_at(&self, node: &Node, width: f64, at: Option<ProbeAt>) -> Option<f64> {
        match node {
            Node::Text(t) => (t.chain.is_none()).then(|| self.text_height(t, width, at)),
            Node::Code(c) => Some(self.code_extent(c, Some(width)).height),
            Node::Image(i) => self.image_height_at(i, width),
            Node::Field(f) => Some(
                resolve_field_to_text(f, self.field_ctx)
                    .map_or(0.0, |t| self.text_height(&t, width, at)),
            ),
            Node::Toc(t) => Some(
                resolve_toc_to_text(
                    t,
                    self.field_ctx.pages,
                    self.field_ctx.page_index_by_node_id,
                )
                .map_or(0.0, |t| self.text_height(&t, width, at)),
            ),
            Node::Shape(s) => self.label_height(s, width),
            Node::Table(t) => Some(table_natural_size(t, Some(width), self.measure_env()).1),
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Frame(_)
            | Node::Group(_)
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
    ///
    /// The probe compiles under the render context of the lowered list (bleed
    /// offset, baseline grid) against the page's runaround boxes. At `at`, or
    /// else at a previous-pass position ([`super::ProbeHints`]), it compiles
    /// where the text draws, so a position-dependent height matches the
    /// render.
    fn text_height(&self, text: &TextNode, width: f64, at: Option<ProbeAt>) -> f64 {
        self.count_probe();
        let mut probe = text.clone();
        let (x, y, ctx) = match at.or_else(|| self.hints.get(&text.id).copied()) {
            Some(at) => (
                at.x,
                at.y,
                RenderCtx {
                    dx: at.dx,
                    dy: at.dy,
                    ..self.base_ctx
                },
            ),
            None => (0.0, 0.0, self.base_ctx),
        };
        probe.x = Some(px_prop(x));
        probe.y = Some(px_prop(y));
        probe.w = Some(px_prop(width));
        probe.h = None;
        let chains = ChainAssignments::new();
        let anchors = AnchorMap::new();
        let env = self.scratch_env(
            &chains,
            &anchors,
            self.md_blocks,
            (self.page_block_styles, self.doc_block_styles),
        );
        let mut commands: Vec<SceneCommand> = Vec::new();
        let mut scratch: Vec<Diagnostic> = Vec::new();
        compile_text(&probe, env, &mut commands, &mut scratch, ctx)
    }

    /// A text compile environment over empty chain and anchor maps and the
    /// page's runaround boxes.
    fn scratch_env<'e>(
        &'e self,
        chains: &'e ChainAssignments,
        anchors: &'e AnchorMap,
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
            node_boxes: self.field_ctx.node_boxes,
            anchors,
            md_blocks,
            page_block_styles,
            doc_block_styles,
        }
    }

    /// The size `compile_code` lays the node out at: its natural width with
    /// no `width`, and its height at `width` otherwise.
    fn code_extent(&self, code: &CodeNode, width: Option<f64>) -> CodeExtent {
        self.count_probe();
        let mut probe = code.clone();
        probe.x = Some(px_prop(0.0));
        probe.y = Some(px_prop(0.0));
        probe.w = width.map(px_prop);
        probe.h = None;
        let chains = ChainAssignments::new();
        let anchors = AnchorMap::new();
        let mut commands: Vec<SceneCommand> = Vec::new();
        let mut scratch: Vec<Diagnostic> = Vec::new();
        compile_code(
            &probe,
            self.scratch_env(&chains, &anchors, empty_md_blocks(), (&[], &[])),
            &mut commands,
            &mut scratch,
            RenderCtx::measure(),
        )
    }

    /// The asset pixel size of `image`, when known and positive.
    fn image_size(&self, image: &ImageNode) -> Option<(f64, f64)> {
        self.image_sizes
            .get(&image.asset)
            .copied()
            .filter(|(w, h)| *w > 0.0 && *h > 0.0)
    }

    /// The asset width, or the width that keeps the aspect at a fixed `h`.
    fn image_natural_width(&self, image: &ImageNode) -> Option<f64> {
        let (iw, ih) = self.image_size(image)?;
        Some(match resolve_geometry_px(image.h.as_ref(), self.resolved) {
            Some(h) if h != ih => h * iw / ih,
            Some(_) | None => iw,
        })
    }

    /// The height that keeps the asset's aspect at `width`.
    fn image_height_at(&self, image: &ImageNode, width: f64) -> Option<f64> {
        let (iw, ih) = self.image_size(image)?;
        Some(if width == iw { ih } else { width * ih / iw })
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
