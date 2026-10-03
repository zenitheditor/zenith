//! The page-layout pre-pass: lower each page's layout frames to absolute
//! geometry before the text-chain and table-flow pre-passes, then run the
//! core geometry checks on the lowered pages.

use std::borrow::Cow;
use std::collections::BTreeMap;

use zenith_core::{
    Diagnostic, Document, FontProvider, Page, ResolvedToken, Style, dim_to_px,
    layout_geometry_checks, subtree_uses_layout,
};
use zenith_geometry::Point2;
use zenith_layout::RustybuzzEngine;

use crate::layout::{LayoutBox, lower_nodes};

use super::super::IntrinsicEnv;
use super::super::field::{
    ConnectorTargetKind, FieldCtx, PathConnectorGeometry, PortTarget, SectionAssignment,
    compute_live_area,
};
use super::super::footnote::collect_footnote_markers;
use super::super::markdown_resolve::MdBlockMap;

/// The pages compile reads, plus what lowering them reported.
pub(super) struct PageLayouts<'p> {
    /// The lowered document. Borrowed when no page holds a layout frame.
    pub(super) doc: Cow<'p, Document>,
    /// Per page: layout diagnostics, then the geometry-check diagnostics.
    pub(super) diagnostics: Vec<Vec<Diagnostic>>,
    /// Per page: resolved layout boxes by node id.
    pub(super) boxes: Vec<BTreeMap<String, LayoutBox>>,
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
        };
    }
    let mut lowered = doc.clone();
    let mut diagnostics = Vec::with_capacity(count);
    let mut boxes = Vec::with_capacity(count);
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
            continue;
        }
        let result = lower_page(doc, page, index, &mut target.children, env);
        diagnostics.push(result.0);
        boxes.push(result.1);
    }
    for (page_diags, geometry) in diagnostics.iter_mut().zip(layout_geometry_checks(&lowered)) {
        page_diags.extend(geometry);
    }
    PageLayouts {
        doc: Cow::Owned(lowered),
        diagnostics,
        boxes,
    }
}

/// Lower one page's children (a clone of `page.children`) with the page's
/// own field context.
fn lower_page(
    doc: &Document,
    page: &Page,
    index: usize,
    children: &mut [zenith_core::Node],
    env: LayoutPassEnv<'_>,
) -> (Vec<Diagnostic>, BTreeMap<String, LayoutBox>) {
    let page_w = dim_to_px(page.width.value, &page.width.unit).unwrap_or(0.0);
    let page_h = dim_to_px(page.height.value, &page.height.unit).unwrap_or(0.0);
    let is_recto = doc.page_is_recto(page, index + 1);
    let live_area = compute_live_area(
        doc,
        page,
        page_w,
        page_h,
        is_recto,
        doc.mirror_margins.unwrap_or(false),
        doc.page_progression.as_deref() == Some("rtl"),
    );
    let footnote_markers = collect_footnote_markers(page);
    let node_boxes: BTreeMap<String, (f64, f64, f64, f64)> = BTreeMap::new();
    let target_kinds: BTreeMap<String, ConnectorTargetKind> = BTreeMap::new();
    let rings: BTreeMap<String, Vec<Point2>> = BTreeMap::new();
    let paths: BTreeMap<String, PathConnectorGeometry> = BTreeMap::new();
    let ports: BTreeMap<String, BTreeMap<String, PortTarget>> = BTreeMap::new();
    let section = env.section_assignments.get(index).and_then(|a| *a);
    let field_ctx = FieldCtx {
        page_index_1based: index + 1,
        is_recto,
        live_area,
        page_index_by_node_id: env.page_index_by_node_id,
        footnote_markers: &footnote_markers,
        node_boxes: &node_boxes,
        connector_target_kinds: &target_kinds,
        connector_outline_boxes: &node_boxes,
        connector_closed_rings: &rings,
        connector_open_polylines: &rings,
        connector_path_geometries: &paths,
        port_map: &ports,
        total_pages: doc.body.pages.len(),
        pages: &doc.body.pages,
        section_page_index: section.map(|a| a.page_index_in_section),
        section_page_count: section.map(|a| a.page_count),
        section_folio_start: section.map(|a| a.folio_start),
        section_folio_style: section.and_then(|a| a.folio_style),
        section_name: section.map(|a| a.name),
    };
    let lowered = lower_nodes(
        children,
        IntrinsicEnv {
            resolved: env.resolved,
            style_map: env.style_map,
            fonts: env.fonts,
            engine: env.engine,
            md_blocks: env.md_blocks,
            field_ctx: &field_ctx,
            page_block_styles: &page.block_styles,
            doc_block_styles: &doc.body.block_styles,
        },
    );
    (lowered.diagnostics, lowered.boxes)
}
