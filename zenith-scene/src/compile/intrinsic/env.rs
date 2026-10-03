//! [`IntrinsicEnv`]: the read-only lookups intrinsic measurement needs.

use std::cell::Cell;
use std::collections::BTreeMap;

use zenith_core::{BlockStyle, FontProvider, Node, ResolvedToken, Style};
use zenith_layout::RustybuzzEngine;

use super::super::field::FieldCtx;
use super::super::imports::ImportScopes;
use super::super::markdown_resolve::MdBlockMap;
use super::super::{ComponentMap, NodeCtx, RenderCtx};

/// Intrinsic pixel size `(w, h)` of each image / SVG asset, by asset id.
pub(crate) type ImageSizes = BTreeMap<String, (f64, f64)>;

/// Where a position-dependent text node sat in the previous layout pass: its
/// written `x` / `y` and the render translation compile adds to them.
///
/// A text with a `text-exclusion` (runaround) wraps against a box at a fixed
/// page position, so its height depends on its own position. A measure probe
/// compiles it at this position, with the same translation, so the hug height
/// equals the rendered height bit for bit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ProbeAt {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) dx: f64,
    pub(crate) dy: f64,
}

/// [`ProbeAt`] by node id.
pub(crate) type ProbeHints = BTreeMap<String, ProbeAt>;

/// Borrowed environment for intrinsic-size measurement.
///
/// Build it from a page compile's [`NodeCtx`] with
/// [`IntrinsicEnv::from_node_ctx`], or field by field in the page pre-pass.
#[derive(Clone, Copy)]
pub(crate) struct IntrinsicEnv<'a> {
    pub(in crate::compile) resolved: &'a BTreeMap<String, ResolvedToken>,
    pub(in crate::compile) style_map: &'a BTreeMap<&'a str, &'a Style>,
    pub(in crate::compile) fonts: &'a dyn FontProvider,
    pub(in crate::compile) engine: &'a RustybuzzEngine<'a>,
    pub(in crate::compile) md_blocks: &'a MdBlockMap,
    /// Page context for `field` and `toc` resolution. Its `node_boxes` are
    /// the runaround boxes a measure probe wraps against.
    pub(in crate::compile) field_ctx: &'a FieldCtx<'a>,
    pub(in crate::compile) page_block_styles: &'a [BlockStyle],
    pub(in crate::compile) doc_block_styles: &'a [BlockStyle],
    /// Component definitions, for sizing an `instance` from its expansion.
    pub(in crate::compile) components: &'a ComponentMap<'a>,
    /// Imported document scopes, for sizing an imported `instance`.
    pub(in crate::compile) imports: &'a ImportScopes<'a>,
    /// Intrinsic pixel size of each image / SVG asset.
    pub(in crate::compile) image_sizes: &'a ImageSizes,
    /// The render context of the node list being lowered (page root: bleed
    /// offset and baseline grid). Measure probes compile under it.
    pub(in crate::compile) base_ctx: RenderCtx,
    /// Positions of position-dependent text from the previous layout pass.
    pub(in crate::compile) hints: &'a ProbeHints,
    /// Scratch compiles run by measure probes (text height, code size).
    pub(in crate::compile) probes: &'a Cell<usize>,
    /// Instance expansions above this measurement (0 at a page or an
    /// expanded list); bounds the measure of nested instances.
    pub(in crate::compile) nesting: usize,
}

impl<'a> IntrinsicEnv<'a> {
    /// The measure environment of a page compile, lowering a node list that
    /// compiles under `base_ctx`.
    pub(in crate::compile) fn from_node_ctx(
        cx: NodeCtx<'a>,
        base_ctx: RenderCtx,
        hints: &'a ProbeHints,
        probes: &'a Cell<usize>,
    ) -> Self {
        Self {
            resolved: cx.resolved,
            style_map: cx.style_map,
            fonts: cx.fonts,
            engine: cx.engine,
            md_blocks: cx.md_blocks,
            field_ctx: cx.field_ctx,
            page_block_styles: cx.page_block_styles,
            doc_block_styles: cx.doc_block_styles,
            components: cx.components,
            imports: cx.imports,
            image_sizes: cx.image_sizes,
            base_ctx,
            hints,
            probes,
            nesting: 0,
        }
    }

    /// Resolved token table.
    pub(crate) fn resolved(&self) -> &'a BTreeMap<String, ResolvedToken> {
        self.resolved
    }

    /// Style id → style lookup.
    pub(crate) fn style_map(&self) -> &'a BTreeMap<&'a str, &'a Style> {
        self.style_map
    }

    /// The render translation of the node list being lowered.
    pub(crate) fn base_translation(&self) -> (f64, f64) {
        (self.base_ctx.dx, self.base_ctx.dy)
    }

    /// `true` when `node`'s hug height depends on its own position: a text
    /// that wraps around a `text-exclusion` box, or a text / field / toc whose
    /// lines snap to the page baseline grid (the snap offset depends on `y`).
    pub(crate) fn is_position_dependent(&self, node: &Node) -> bool {
        let grid = self.base_ctx.baseline_grid.is_some();
        match node {
            Node::Text(t) => t.chain.is_none() && (t.text_exclusion.is_some() || grid),
            Node::Field(_) | Node::Toc(_) => grid,
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
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
            | Node::Unknown(_) => false,
        }
    }

    pub(in crate::compile) fn count_probe(&self) {
        self.probes.set(self.probes.get().saturating_add(1));
    }
}
