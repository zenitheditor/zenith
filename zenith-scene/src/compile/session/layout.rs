//! The page-layout pre-pass: lower each page's layout frames to absolute
//! geometry before the text-chain and table-flow pre-passes, then run the
//! core geometry checks on the lowered pages.
//!
//! Each page lowers with the render context, runaround boxes, and anchor
//! environment its compile uses, so measured sizes equal rendered sizes. A
//! page whose hugging text depends on its own position lowers again until
//! positions settle ([`settle`]).

use std::borrow::Cow;
use std::cell::Cell;
use std::collections::BTreeMap;

use zenith_core::{
    Diagnostic, Document, FontProvider, Node, Page, ResolvedToken, Style, dim_to_px,
    layout_geometry_checks, subtree_uses_layout,
};
use zenith_geometry::Point2;
use zenith_layout::RustybuzzEngine;

use crate::layout::{LayoutBox, Lowered, NodeBoxes, lower_nodes, may_depend_on_position, settle};

use super::super::field::{
    ConnectorTargetKind, FieldCtx, PathConnectorGeometry, PortTarget, SectionAssignment,
    build_node_boxes, compute_live_area,
};
use super::super::footnote::collect_footnote_markers;
use super::super::imports::ImportScopes;
use super::super::markdown_resolve::MdBlockMap;
use super::super::{ComponentMap, ImageSizes, IntrinsicEnv, PrePassEnv, ProbeHints, RenderCtx};
use super::compile::{page_bleed, root_render_ctx};

/// Counters of one page's auto-layout pre-pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayoutStats {
    /// Lowering passes run: 0 for a page without a layout frame, 1 unless
    /// position-dependent text made the page lower again.
    pub passes: usize,
    /// Scratch compiles the measure probes ran (text heights, code sizes),
    /// over all passes. Instance and master expansions at compile time are
    /// not counted.
    pub probes: usize,
}

/// The pages compile reads, plus what lowering them reported.
pub(super) struct PageLayouts<'p> {
    /// The lowered document. Borrowed when no page holds a layout frame.
    pub(super) doc: Cow<'p, Document>,
    /// Per page: layout diagnostics, then the geometry-check diagnostics.
    pub(super) diagnostics: Vec<Vec<Diagnostic>>,
    /// Per page: resolved layout boxes by node id.
    pub(super) boxes: Vec<BTreeMap<String, LayoutBox>>,
    /// Per page: pass and probe counts.
    pub(super) stats: Vec<LayoutStats>,
}

/// Read-only inputs of the layout pre-pass.
#[derive(Clone, Copy)]
pub(super) struct LayoutPassEnv<'a> {
    pub(super) resolved: &'a BTreeMap<String, ResolvedToken>,
    pub(super) style_map: &'a BTreeMap<&'a str, &'a Style>,
    pub(super) fonts: &'a dyn FontProvider,
    pub(super) engine: &'a RustybuzzEngine<'a>,
    pub(super) md_blocks: &'a MdBlockMap,
    pub(super) page_index_by_node_id: &'a BTreeMap<String, usize>,
    pub(super) section_assignments: &'a [Option<SectionAssignment<'a>>],
    pub(super) components: &'a ComponentMap<'a>,
    pub(super) imports: &'a ImportScopes<'a>,
    pub(super) image_sizes: &'a ImageSizes,
}

/// Lower every page of `doc` that holds a layout frame.
///
/// Returns `Cow::Borrowed(doc)` when no page holds one, so such a document
/// compiles from the caller's AST unchanged.
pub(super) fn lower_pages<'p>(doc: &'p Document, env: LayoutPassEnv<'_>) -> PageLayouts<'p> {
    let count = doc.body.pages.len();
    if !doc
        .body
        .pages
        .iter()
        .any(|p| subtree_uses_layout(&p.children))
    {
        return PageLayouts {
            doc: Cow::Borrowed(doc),
            diagnostics: vec![Vec::new(); count],
            boxes: vec![BTreeMap::new(); count],
            stats: vec![LayoutStats::default(); count],
        };
    }
    let mut lowered = doc.clone();
    let mut diagnostics = Vec::with_capacity(count);
    let mut boxes = Vec::with_capacity(count);
    let mut stats = Vec::with_capacity(count);
    for (index, (page, target)) in doc
        .body
        .pages
        .iter()
        .zip(lowered.body.pages.iter_mut())
        .enumerate()
    {
        if !subtree_uses_layout(&page.children) {
            diagnostics.push(Vec::new());
            boxes.push(BTreeMap::new());
            stats.push(LayoutStats::default());
            continue;
        }
        let result = lower_page(doc, page, index, &mut target.children, env);
        diagnostics.push(result.0);
        boxes.push(result.1);
        stats.push(result.2);
    }
    for (page_diags, geometry) in diagnostics.iter_mut().zip(layout_geometry_checks(&lowered)) {
        page_diags.extend(geometry);
    }
    PageLayouts {
        doc: Cow::Owned(lowered),
        diagnostics,
        boxes,
        stats,
    }
}

/// The per-page inputs of every lowering pass of one page.
struct PageScope<'a> {
    doc: &'a Document,
    page: &'a Page,
    index: usize,
    env: LayoutPassEnv<'a>,
    is_recto: bool,
    live_area: Option<(f64, f64, f64, f64)>,
    footnote_markers: BTreeMap<String, String>,
    no_boxes: NodeBoxes,
    target_kinds: BTreeMap<String, ConnectorTargetKind>,
    rings: BTreeMap<String, Vec<Point2>>,
    paths: BTreeMap<String, PathConnectorGeometry>,
    ports: BTreeMap<String, BTreeMap<String, PortTarget>>,
    base_ctx: RenderCtx,
    probes: Cell<usize>,
}

impl PageScope<'_> {
    /// The page's field context, with `node_boxes` as the runaround boxes.
    fn field_ctx<'s>(&'s self, node_boxes: &'s NodeBoxes) -> FieldCtx<'s> {
        let section = self
            .env
            .section_assignments
            .get(self.index)
            .and_then(|a| *a);
        FieldCtx {
            page_index_1based: self.index + 1,
            is_recto: self.is_recto,
            live_area: self.live_area,
            page_index_by_node_id: self.env.page_index_by_node_id,
            footnote_markers: &self.footnote_markers,
            node_boxes,
            connector_target_kinds: &self.target_kinds,
            connector_outline_boxes: &self.no_boxes,
            connector_closed_rings: &self.rings,
            connector_open_polylines: &self.rings,
            connector_path_geometries: &self.paths,
            port_map: &self.ports,
            total_pages: self.doc.body.pages.len(),
            pages: &self.doc.body.pages,
            section_page_index: section.map(|a| a.page_index_in_section),
            section_page_count: section.map(|a| a.page_count),
            section_folio_start: section.map(|a| a.folio_start),
            section_folio_style: section.and_then(|a| a.folio_style),
            section_name: section.map(|a| a.name),
        }
    }

    /// The measure environment over `field_ctx` and probe `hints`.
    fn intrinsic<'s>(
        &'s self,
        field_ctx: &'s FieldCtx<'s>,
        hints: &'s ProbeHints,
    ) -> IntrinsicEnv<'s> {
        IntrinsicEnv {
            resolved: self.env.resolved,
            style_map: self.env.style_map,
            fonts: self.env.fonts,
            engine: self.env.engine,
            md_blocks: self.env.md_blocks,
            field_ctx,
            page_block_styles: &self.page.block_styles,
            doc_block_styles: &self.doc.body.block_styles,
            components: self.env.components,
            imports: self.env.imports,
            image_sizes: self.env.image_sizes,
            base_ctx: self.base_ctx,
            hints,
            probes: &self.probes,
            nesting: 0,
        }
    }

    /// One lowering pass over `list`.
    fn pass(
        &self,
        list: &mut [Node],
        hints: &ProbeHints,
        boxes: &NodeBoxes,
        anchors: PrePassEnv<'_>,
    ) -> Lowered {
        let field_ctx = self.field_ctx(boxes);
        lower_nodes(list, self.intrinsic(&field_ctx, hints), Some(anchors))
    }
}

/// Lower one page's children (`children`, a clone of `page.children`) with
/// the page's own field context, render context, and anchor environment.
fn lower_page(
    doc: &Document,
    page: &Page,
    index: usize,
    children: &mut [Node],
    env: LayoutPassEnv<'_>,
) -> (Vec<Diagnostic>, BTreeMap<String, LayoutBox>, LayoutStats) {
    let page_w = dim_to_px(page.width.value, &page.width.unit).unwrap_or(0.0);
    let page_h = dim_to_px(page.height.value, &page.height.unit).unwrap_or(0.0);
    let is_recto = doc.page_is_recto(page, index + 1);
    let scope = PageScope {
        doc,
        page,
        index,
        env,
        is_recto,
        live_area: compute_live_area(
            doc,
            page,
            page_w,
            page_h,
            is_recto,
            doc.mirror_margins.unwrap_or(false),
            doc.page_progression.as_deref() == Some("rtl"),
        ),
        footnote_markers: collect_footnote_markers(page),
        no_boxes: NodeBoxes::new(),
        target_kinds: BTreeMap::new(),
        rings: BTreeMap::new(),
        paths: BTreeMap::new(),
        ports: BTreeMap::new(),
        base_ctx: root_render_ctx(page, page_bleed(page)),
        probes: Cell::new(0),
    };
    let anchors = PrePassEnv {
        page_w,
        page_h,
        safe_zones: &page.safe_zones,
        resolved: env.resolved,
    };
    let no_hints = ProbeHints::new();
    let dependent = {
        let field_ctx = scope.field_ctx(&scope.no_boxes);
        may_depend_on_position(&page.children, &scope.intrinsic(&field_ctx, &no_hints))
    };
    let rebox = |nodes: &[Node]| build_node_boxes(nodes, env.resolved, env.components, env.imports);
    let (lowered, passes) = if dependent {
        settle(
            &page.children,
            children,
            rebox(&page.children),
            Some(&rebox),
            &mut |list, hints, boxes| scope.pass(list, hints, boxes, anchors),
        )
    } else {
        (scope.pass(children, &no_hints, &scope.no_boxes, anchors), 1)
    };
    let stats = LayoutStats {
        passes,
        probes: scope.probes.get(),
    };
    (lowered.diagnostics, lowered.boxes, stats)
}
