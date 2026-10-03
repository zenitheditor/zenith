//! [`PageCompiler::compile_page`]: compile one page into a display list.

use zenith_core::{Diagnostic, FontProvider, Page, dim_to_px};
use zenith_layout::RustybuzzEngine;

use crate::ir::{Paint, Rect, Scene, SceneCommand};

use std::collections::BTreeMap;

use super::super::anchor::build_anchor_map;
use super::super::boxes::{BoxRecorder, CompiledBox};
use super::super::container;
use super::super::crop;
use super::super::ctx::NodeCtx;
use super::super::dispatch::compile_node;
use super::super::field::{
    FieldCtx, build_connector_targets, build_node_boxes, build_port_map, compute_live_area,
};
use super::super::footnote;
use super::super::intrinsic::lower_expanded;
use super::super::line_jumps;
use super::super::page_source::{PageSourceEnv, compile_page_source};
use super::super::paint::{resolve_property_color, resolve_property_gradient};
use super::super::text::ShapeEnv;
use super::super::{CompileResult, RenderCtx};
use super::fonts::FontsRef;
use super::label_contrast;
use super::page::PageCompiler;

impl<F: ?Sized + FontProvider> PageCompiler<'_, F> {
    /// Compile the page at `page_index` (0-based) into a [`CompileResult`].
    ///
    /// Diagnostics: the shared document diagnostics first, then this page's.
    /// Page 0 also reports the text-chain and table-flow diagnostics. Repeats
    /// are removed, first occurrence kept. An empty document or an
    /// out-of-range index returns an empty scene with a `scene.no_pages` or
    /// `scene.page_out_of_range` advisory.
    #[must_use]
    pub fn compile_page(&self, page_index: usize) -> CompileResult {
        self.compile_page_with(page_index, true, None)
    }

    /// Compile the page at `page_index` with only its own diagnostics.
    ///
    /// The document diagnostics are left out: the shared data, import, and
    /// token diagnostics, and the page-0 chain and table-flow diagnostics.
    /// Read them once through [`PageCompiler::document_diagnostics`]. The
    /// scene is identical to [`PageCompiler::compile_page`].
    #[must_use]
    pub fn compile_page_local(&self, page_index: usize) -> CompileResult {
        self.compile_page_with(page_index, false, None)
    }

    /// The final geometry of every node compiled on page `page_index`, by id,
    /// in page-absolute px: the unrotated box (anchors resolved, text heights
    /// measured, footnotes in their zone, line / path / connector boxes from
    /// their stroked bounds), the node's rotation, and its visual bounds.
    /// Master projections record as `<page-id>/<id>`, instance content as
    /// `<instance-id>/<id>`, and pattern motif instances as
    /// `<pattern-id>/<index>/<motif-id>`. Guide nodes and nodes that fail to
    /// compile have no box. Empty for an out-of-range index.
    #[must_use]
    pub fn compiled_boxes(&self, page_index: usize) -> BTreeMap<String, CompiledBox> {
        let recorder = BoxRecorder::default();
        let _ = self.compile_page_with(page_index, false, Some(&recorder));
        recorder.into_boxes()
    }

    /// Document diagnostics, reported once per document.
    ///
    /// The shared data, import, and token diagnostics first, then the
    /// text-chain and table-flow diagnostics. Repeats are removed.
    #[must_use]
    pub fn document_diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics = self.prep.shared_diagnostics.clone();
        diagnostics.extend(self.page0_diagnostics.iter().cloned());
        self.prep.id_aliases.scrub(&mut diagnostics);
        Diagnostic::dedup(diagnostics)
    }

    fn compile_page_with(
        &self,
        page_index: usize,
        with_document: bool,
        boxes: Option<&BoxRecorder>,
    ) -> CompileResult {
        let prep = self.prep;
        let doc: &zenith_core::Document = &self.lowered;
        let mut diagnostics: Vec<Diagnostic> = if with_document {
            prep.shared_diagnostics.clone()
        } else {
            Vec::new()
        };

        let Some(page) = doc.body.pages.get(page_index) else {
            diagnostics.push(missing_page_diagnostic(doc, page_index));
            return empty_result(diagnostics);
        };
        let Some(page_w) = page_dimension_px(page, "width", &mut diagnostics) else {
            return empty_result(diagnostics);
        };
        let Some(page_h) = page_dimension_px(page, "height", &mut diagnostics) else {
            return empty_result(diagnostics);
        };

        let bleed = page_bleed(page);
        let media_w = page_w + 2.0 * bleed;
        let media_h = page_h + 2.0 * bleed;

        let mut scene = Scene::new(media_w, media_h);
        // Outermost media-edge clip.
        scene.commands.push(SceneCommand::PushClip {
            x: 0.0,
            y: 0.0,
            w: media_w,
            h: media_h,
        });
        self.emit_background(page, media_w, media_h, &mut scene, &mut diagnostics);

        let resolved = &prep.resolved;
        let anchors = build_anchor_map(page, page_w, page_h, resolved);

        // Document-wide chain and flow diagnostics surface on page 0 only.
        if with_document && page_index == 0 {
            diagnostics.extend(self.page0_diagnostics.iter().cloned());
        }
        // This page's auto-layout and lowered-geometry diagnostics.
        if let Some(layout) = self.page_layout_diagnostics.get(page_index) {
            diagnostics.extend(layout.iter().cloned());
        }

        // One engine per page compile. Each face parses at most once here.
        let fonts_ref = FontsRef(self.fonts);
        let fonts: &dyn FontProvider = &fonts_ref;
        let engine = RustybuzzEngine::new(&self.font_faces);

        // Per-page field context. Parity and live area mirror the validator.
        let page_index_1based = page_index + 1;
        let is_recto = doc.page_is_recto(page, page_index_1based);
        let mirror_margins = doc.mirror_margins.unwrap_or(false);
        let rtl_book = doc.page_progression.as_deref() == Some("rtl");
        let live_area = compute_live_area(
            doc,
            page,
            page_w,
            page_h,
            is_recto,
            mirror_margins,
            rtl_book,
        );
        let footnote_markers = footnote::collect_footnote_markers(page);
        let import_scopes = &prep.import_scopes;
        let node_boxes =
            build_node_boxes(&page.children, resolved, &self.component_map, import_scopes);
        let connector_targets = build_connector_targets(
            page,
            &node_boxes,
            resolved,
            &self.component_map,
            import_scopes,
        );
        let port_map = build_port_map(page, &self.component_map, import_scopes);
        let section_assign = self
            .section_assignments
            .get(page_index)
            .and_then(|opt| *opt);

        let field_ctx = FieldCtx {
            page_index_1based,
            is_recto,
            live_area,
            page_index_by_node_id: &self.page_index_by_node_id,
            footnote_markers: &footnote_markers,
            node_boxes: &node_boxes,
            connector_target_kinds: &connector_targets.kinds,
            connector_outline_boxes: &connector_targets.outline_boxes,
            connector_closed_rings: &connector_targets.closed_rings,
            connector_open_polylines: &connector_targets.open_polylines,
            connector_path_geometries: &connector_targets.path_geometries,
            port_map: &port_map,
            total_pages: doc.body.pages.len(),
            pages: &doc.body.pages,
            section_page_index: section_assign.map(|a| a.page_index_in_section),
            section_page_count: section_assign.map(|a| a.page_count),
            section_folio_start: section_assign.map(|a| a.folio_start),
            section_folio_style: section_assign.and_then(|a| a.folio_style),
            section_name: section_assign.map(|a| a.name),
        };

        let node_cx = NodeCtx {
            resolved,
            style_map: &self.style_map,
            components: &self.component_map,
            imports: import_scopes,
            fonts,
            engine: &engine,
            chains: &self.chains,
            flows: &self.flows,
            anchors: &anchors,
            field_ctx: &field_ctx,
            md_blocks: &prep.md_blocks,
            page_block_styles: &page.block_styles,
            doc_block_styles: &doc.body.block_styles,
            image_sizes: &prep.image_sizes,
            boxes,
        };

        let root_ctx = root_render_ctx(page, bleed);

        // Indices of each top-level connector's `StrokePolyline`, for the
        // opt-in line-jump post-pass.
        let mut connector_strokes: Vec<usize> = Vec::new();

        // Master projection, under the page's own children. Projected ids get
        // the page id prefix. An unknown master is skipped.
        if let Some(master_id) = &page.master
            && let Some(master) = self.master_map.get(master_id.as_str())
        {
            let mut projected = master.children.clone();
            let prefix = format!("{}/", page.id);
            container::prefix_ids_in_children(&mut projected, &prefix);
            // Masters lower per page: a field measures against this page.
            lower_expanded(&mut projected, node_cx, root_ctx, &mut diagnostics);
            for node in &projected {
                compile_node(
                    node,
                    node_cx,
                    &mut scene.commands,
                    &mut diagnostics,
                    &mut connector_strokes,
                    root_ctx,
                );
            }
        }

        compile_page_source(
            PageSourceEnv {
                page,
                page_w,
                page_h,
                root_ctx,
                fonts,
                data: prep.data,
                graph: prep.imports,
                scopes: import_scopes,
            },
            &mut scene.commands,
            &mut diagnostics,
        );

        // Page children in source order (first = bottom).
        for node in &page.children {
            compile_node(
                node,
                node_cx,
                &mut scene.commands,
                &mut diagnostics,
                &mut connector_strokes,
                root_ctx,
            );
        }

        // Opt-in connector line-jumps. Only "arc" and "gap" change commands.
        if let Some(mode) = page.line_jumps.as_deref()
            && (mode == "arc" || mode == "gap")
        {
            line_jumps::apply_line_jumps(&mut scene.commands, &connector_strokes, mode);
        }

        // Footnote zone, on top of body content and inside the media clip.
        footnote::compile_footnote_zone(
            page,
            live_area,
            footnote::FootnoteZoneEnv {
                markers: &footnote_markers,
                resolved,
                style_map: &self.style_map,
                fonts,
                engine: &engine,
                chains: &self.chains,
                anchors: &anchors,
                field_ctx: &field_ctx,
                boxes,
            },
            &mut scene.commands,
            &mut diagnostics,
            root_ctx,
        );

        scene.commands.push(SceneCommand::PopClip);

        // Crop marks sit outside the clip, in the bleed margin. The trim box is
        // the inner page rectangle. With no bleed, `trim` stays `None`.
        if bleed > 0.0 {
            crop::emit_crop_marks(&mut scene.commands, bleed, page_w, page_h);
            scene.trim = Some(Rect {
                x: bleed,
                y: bleed,
                w: page_w,
                h: page_h,
            });
        }

        // Shape and connector label contrast, judged on the drawn label ink.
        diagnostics.extend(label_contrast::label_contrast(
            &scene.commands,
            page,
            bleed,
            resolved,
            &self.style_map,
            ShapeEnv {
                engine: &engine,
                fonts,
            },
        ));

        // Internal defaults copy ids never leave the compile.
        prep.id_aliases.scrub(&mut diagnostics);

        CompileResult {
            scene,
            diagnostics: Diagnostic::dedup(diagnostics),
        }
    }

    /// Fill the whole media box with the page background, when one is set.
    fn emit_background(
        &self,
        page: &Page,
        media_w: f64,
        media_h: f64,
        scene: &mut Scene,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(bg_prop) = &page.background else {
            return;
        };
        let resolved = &self.prep.resolved;
        let paint = if let Some(gradient) = resolve_property_gradient(bg_prop, resolved, &page.id) {
            Paint::Gradient(gradient)
        } else if let Some(color) = resolve_property_color(bg_prop, resolved, diagnostics, &page.id)
        {
            Paint::solid(color)
        } else {
            return;
        };
        scene.commands.push(SceneCommand::FillRect {
            x: 0.0,
            y: 0.0,
            w: media_w,
            h: media_h,
            paint,
        });
    }
}

/// Advisory for a page index with no page: no pages at all, or out of range.
fn missing_page_diagnostic(doc: &zenith_core::Document, page_index: usize) -> Diagnostic {
    if doc.body.pages.is_empty() {
        Diagnostic::advisory(
            "scene.no_pages",
            "document has no pages; an empty scene is returned",
            None,
            Some(doc.body.id.clone()),
        )
    } else {
        Diagnostic::advisory(
            "scene.page_out_of_range",
            format!(
                "page index {} is out of range; document has {} page(s)",
                page_index,
                doc.body.pages.len()
            ),
            None,
            Some(doc.body.id.clone()),
        )
    }
}

/// Page `axis` ("width" or "height") in pixels. An unsupported unit pushes a
/// `scene.unsupported_unit` advisory and returns `None`.
fn page_dimension_px(page: &Page, axis: &str, diagnostics: &mut Vec<Diagnostic>) -> Option<f64> {
    let dim = if axis == "width" {
        &page.width
    } else {
        &page.height
    };
    let px = dim_to_px(dim.value, &dim.unit);
    if px.is_none() {
        diagnostics.push(Diagnostic::advisory(
            "scene.unsupported_unit",
            format!(
                "page '{}' {axis} uses an unsupported unit; cannot compile scene",
                page.id
            ),
            page.source_span,
            Some(page.id.clone()),
        ));
    }
    px
}

/// Empty scene with `diagnostics`.
fn empty_result(diagnostics: Vec<Diagnostic>) -> CompileResult {
    CompileResult {
        scene: Scene::new(0.0, 0.0),
        diagnostics,
    }
}

/// The page's print bleed in px. An absent, unresolvable, or non-positive
/// bleed is 0, which is the no-bleed path.
pub(super) fn page_bleed(page: &Page) -> f64 {
    page.bleed
        .as_ref()
        .and_then(|d| dim_to_px(d.value, &d.unit))
        .filter(|&px| px > 0.0)
        .unwrap_or(0.0)
}

/// Root render context: shifted into the trim box under a bleed, with the
/// page baseline grid when it resolves to a positive pixel pitch.
pub(super) fn root_render_ctx(page: &Page, bleed: f64) -> RenderCtx {
    let mut root_ctx = if bleed > 0.0 {
        RenderCtx::root_offset(bleed, bleed)
    } else {
        RenderCtx::root()
    };
    root_ctx.baseline_grid = page
        .baseline_grid
        .as_ref()
        .and_then(|d| dim_to_px(d.value, &d.unit))
        .filter(|g| g.is_finite() && *g > 0.0);
    root_ctx
}

#[cfg(test)]
mod tests {
    use zenith_core::{KdlAdapter, KdlSource, default_provider};

    use super::super::super::DocumentPrep;
    use super::*;

    const THREE_PAGES: &str = r##"zenith version=1 {
  project id="proj.s" name="S"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.s" title="S" {
    page id="page.1" w=(px)60 h=(px)40 background=(data)"c"
    page id="page.2" w=(px)60 h=(px)40 background=(data)"c"
    page id="page.3" w=(px)60 h=(px)40 background=(data)"c"
  }
}
"##;

    #[test]
    fn local_compile_omits_document_diagnostics_and_keeps_the_scene() {
        let doc = KdlAdapter.parse(THREE_PAGES.as_bytes()).expect("parse");
        let fonts = default_provider();
        let prep = DocumentPrep::new(&doc, None, None);
        let compiler = PageCompiler::new(&prep, &fonts);
        let document = compiler.document_diagnostics();
        assert_eq!(
            document
                .iter()
                .filter(|d| d.code == "data.no_context")
                .count(),
            1
        );
        for page_index in 0..3 {
            let full = compiler.compile_page(page_index);
            let local = compiler.compile_page_local(page_index);
            assert_eq!(
                full.scene.to_json().expect("scene JSON"),
                local.scene.to_json().expect("scene JSON")
            );
            assert!(local.diagnostics.iter().all(|d| !document.contains(d)));
            let mut joined = document.clone();
            joined.extend(local.diagnostics);
            for d in &full.diagnostics {
                assert!(joined.contains(d), "missing {d:?}");
            }
        }
    }
}
