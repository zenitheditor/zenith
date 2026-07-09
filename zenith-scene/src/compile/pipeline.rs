//! Page compilation pipeline: token resolve, pre-passes, and display-list emit.
//!
//! Entry: [`compile_page_inner`]. Public wrappers live in [`super::entry`].

use std::collections::BTreeMap;

use zenith_core::{
    DataContext, Diagnostic, Document, FontProvider, PropertyValue, Style, dim_to_px,
    resolve_tokens,
};
use zenith_layout::RustybuzzEngine;

use crate::ir::{Paint, Rect, Scene, SceneCommand};

use super::anchor::build_anchor_map;
use super::chain::resolve_chains_document;
use super::container;
use super::crop;
use super::ctx::NodeCtx;
use super::data_resolve::{scan_for_data_refs, substitute_data_refs};
use super::dispatch::compile_node;
use super::field::{
    FieldCtx, build_connector_targets, build_node_boxes, build_page_index_map, build_port_map,
    build_section_assignments, compute_live_area,
};
use super::footnote;
use super::imports::{self, ImportGraph};
use super::line_jumps;
use super::markdown_resolve::{self, resolve_markdown, scan_for_markdown_text};
use super::page_source::{PageSourceEnv, compile_page_source};
use super::paint::{resolve_property_color, resolve_property_gradient};
use super::table_flow::resolve_table_flows;
use super::{ComponentMap, MasterMap};

// ── Render context ────────────────────────────────────────────────────────────

/// Per-subtree rendering context that cascades through the node tree.
///
/// Each field accumulates transformations as we descend:
/// - `opacity` — multiplied together at each group boundary; leaf nodes
///   apply it on top of their own node-level opacity.
/// - `dx`/`dy` — translation offset accumulated from all ancestor groups
///   with an `x`/`y` property; added to every leaf geometry position.
#[derive(Clone, Copy)]
pub(in crate::compile) struct RenderCtx {
    /// Accumulated opacity multiplier (1.0 = fully opaque).
    pub(super) opacity: f64,
    /// Accumulated x-translation in pixels.
    pub(super) dx: f64,
    /// Accumulated y-translation in pixels.
    pub(super) dy: f64,
    /// Resolved page baseline-grid pitch in pixels, when active on this page.
    /// `Some(g)` with `g > 0.0` snaps text line baselines onto `{0, g, 2g, …}`
    /// measured in the post-`dy` coordinate space; `None` → no grid (the snap is
    /// skipped, byte-identical to before). Cascades unchanged to every child
    /// context so all text on the page shares one grid.
    pub(super) baseline_grid: Option<f64>,
}

impl RenderCtx {
    fn root() -> Self {
        RenderCtx {
            opacity: 1.0,
            dx: 0.0,
            dy: 0.0,
            baseline_grid: None,
        }
    }

    /// Identity context used by the footnote zone's scratch MEASURE pass: the
    /// synthesized footnote text is compiled into a throwaway buffer at the
    /// origin to read its laid-out height before the real (offset) emit. Same
    /// fields as [`RenderCtx::root`].
    pub(super) fn measure() -> Self {
        RenderCtx {
            opacity: 1.0,
            dx: 0.0,
            dy: 0.0,
            baseline_grid: None,
        }
    }

    /// Root context translated by a fixed pixel offset on both axes. Used to
    /// shift all page content into the trim box when a print bleed is active:
    /// authored coordinate `(0, 0)` then lands at the trim corner `(b, b)`.
    fn root_offset(dx: f64, dy: f64) -> Self {
        RenderCtx {
            opacity: 1.0,
            dx,
            dy,
            baseline_grid: None,
        }
    }
}

// ── Public result type ────────────────────────────────────────────────────────

/// The result of compiling a [`Document`] into a [`Scene`].
#[derive(Debug, Clone)]
pub struct CompileResult {
    /// The compiled display list.
    pub scene: Scene,
    /// All diagnostics collected during compilation (may include token-resolution
    /// diagnostics, unit advisories, and unsupported-node advisories).
    pub diagnostics: Vec<Diagnostic>,
}

// ── Style cascade helper ──────────────────────────────────────────────────────

/// Look up a style property value by (style_ref, style_map, key).
///
/// Returns `None` when there is no style reference, the style id is not in the
/// map, or the style does not carry the requested key.
pub(in crate::compile) fn style_prop<'a>(
    style_ref: &Option<String>,
    style_map: &'a BTreeMap<&str, &Style>,
    key: &str,
) -> Option<&'a PropertyValue> {
    let sid = style_ref.as_deref()?;
    style_map.get(sid)?.properties.get(key)
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub(in crate::compile) fn compile_page_inner(
    doc: &Document,
    fonts: &dyn FontProvider,
    page_index: usize,
    data: Option<&DataContext>,
    imports: Option<&ImportGraph<'_>>,
) -> CompileResult {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();

    // ── Step 0: data-binding pre-pass ─────────────────────────────────────
    // Resolve every `(data)"field"` property reference and every span
    // `data-ref` BEFORE compilation so all downstream resolvers only ever see
    // `Literal` / `TokenRef` / `Dimension` values.
    //
    // - `data = Some`: clone the doc once, substitute in place, then compile the
    //   clone. The clone is unavoidable because compilation borrows `doc`
    //   immutably elsewhere; it only happens on the data-binding path.
    // - `data = None`: NEVER clone. A read-only scan emits a single
    //   `data.no_context` advisory iff any ref exists, then the original `doc`
    //   compiles by reference — byte-identical to the no-data-binding path.
    let mut md_blocks: markdown_resolve::MdBlockMap = markdown_resolve::MdBlockMap::new();
    let owned_doc: Option<Document> = match data {
        Some(ctx) => {
            let mut cloned = doc.clone();
            substitute_data_refs(&mut cloned, ctx, &mut diagnostics);
            // ── Step 0b: markdown-resolution pass ────────────────────────
            // For each `text` node with `format="markdown"`, concatenate the
            // (now data-substituted) span texts, replace spans with the parsed
            // inline styled spans, and record the parsed BLOCK list in
            // `md_blocks` (consumed by the block-layout path). Nodes without
            // `format="markdown"` are skipped (byte-identical).
            md_blocks = resolve_markdown(&mut cloned);
            Some(cloned)
        }
        None => {
            // Read-only scan: emit ONE `data.no_context` advisory iff a ref
            // exists. No clone, no mutation — byte-identical when refs are absent.
            if scan_for_data_refs(doc) {
                diagnostics.push(Diagnostic::advisory(
                    "data.no_context",
                    "document contains `(data)` references but no data context was \
                     provided at compile time; the references are left unresolved",
                    None,
                    None,
                ));
            }
            // ── Step 0b: markdown-resolution pass (no-data path) ─────────
            // Even without a data context, `format="markdown"` nodes must be
            // resolved. Clone only when at least one markdown-format text node
            // exists; otherwise skip entirely (byte-identical to before).
            if scan_for_markdown_text(doc) {
                let mut cloned = doc.clone();
                md_blocks = resolve_markdown(&mut cloned);
                Some(cloned)
            } else {
                None
            }
        }
    };
    // From here on, compile against the (possibly substituted) document.
    let doc: &Document = owned_doc.as_ref().unwrap_or(doc);
    let import_scopes = match imports {
        Some(graph) => imports::ImportScopes::from_graph(graph, doc, &mut diagnostics),
        None => imports::ImportScopes::disabled(),
    };

    // ── Step 1: resolve tokens ────────────────────────────────────────────
    let token_resolution = resolve_tokens(&doc.tokens);
    diagnostics.extend(token_resolution.diagnostics);
    let resolved = &token_resolution.resolved;

    // ── Step 1b: build style lookup map ──────────────────────────────────
    let style_map: BTreeMap<&str, &Style> = doc
        .styles
        .styles
        .iter()
        .map(|s| (s.id.as_str(), s))
        .collect();

    // ── Step 1c: build component lookup map ──────────────────────────────
    // Instances expand their referenced component at compile time. First
    // declaration wins on a duplicate id (the validator flags id.duplicate).
    let mut component_map: ComponentMap = BTreeMap::new();
    for comp in &doc.components {
        component_map.entry(comp.id.as_str()).or_insert(comp);
    }

    // ── Step 1d: build master lookup map + page-ref index ────────────────
    // A page's `master` attribute projects the named master's nodes (fields
    // resolved against that page) under the page's own children. The page-ref
    // index maps every node id to the 1-based page that contains it, for
    // `page-ref` field resolution. Both are document-wide and order-stable.
    let mut master_map: MasterMap = BTreeMap::new();
    for master in &doc.masters {
        master_map.entry(master.id.as_str()).or_insert(master);
    }
    let page_index_by_node_id = build_page_index_map(doc);

    // ── Step 2: select the requested page ────────────────────────────────
    let Some(page) = doc.body.pages.get(page_index) else {
        if doc.body.pages.is_empty() {
            diagnostics.push(Diagnostic::advisory(
                "scene.no_pages",
                "document has no pages; an empty scene is returned",
                None,
                Some(doc.body.id.clone()),
            ));
        } else {
            diagnostics.push(Diagnostic::advisory(
                "scene.page_out_of_range",
                format!(
                    "page index {} is out of range; document has {} page(s)",
                    page_index,
                    doc.body.pages.len()
                ),
                None,
                Some(doc.body.id.clone()),
            ));
        }
        return CompileResult {
            scene: Scene::new(0.0, 0.0),
            diagnostics,
        };
    };

    // ── Step 3: page dimensions → pixels ─────────────────────────────────
    let page_w = match dim_to_px(page.width.value, &page.width.unit) {
        Some(v) => v,
        None => {
            diagnostics.push(Diagnostic::advisory(
                "scene.unsupported_unit",
                format!(
                    "page '{}' width uses an unsupported unit; cannot compile scene",
                    page.id
                ),
                page.source_span,
                Some(page.id.clone()),
            ));
            return CompileResult {
                scene: Scene::new(0.0, 0.0),
                diagnostics,
            };
        }
    };
    let page_h = match dim_to_px(page.height.value, &page.height.unit) {
        Some(v) => v,
        None => {
            diagnostics.push(Diagnostic::advisory(
                "scene.unsupported_unit",
                format!(
                    "page '{}' height uses an unsupported unit; cannot compile scene",
                    page.id
                ),
                page.source_span,
                Some(page.id.clone()),
            ));
            return CompileResult {
                scene: Scene::new(0.0, 0.0),
                diagnostics,
            };
        }
    };

    // ── Step 3b: resolve print bleed ─────────────────────────────────────
    // A page may declare a uniform `bleed` margin. When it resolves to a
    // positive pixel value `b`, the media (canvas) box expands to
    // `(page_w + 2b) × (page_h + 2b)`, the trim box is the inner
    // `[b, b, page_w, page_h]`, all content shifts by `(b, b)`, the background
    // fills the whole media box, and crop marks are drawn at the trim corners.
    // An absent / unresolvable / non-positive bleed yields `b = 0`, which is
    // byte-identical to the no-bleed path. The validator surfaces a warning for
    // an unresolvable unit or a negative value; the compiler just ignores it.
    let bleed = page
        .bleed
        .as_ref()
        .and_then(|d| dim_to_px(d.value, &d.unit))
        .filter(|&px| px > 0.0)
        .unwrap_or(0.0);

    // Media box (full canvas including bleed on all four sides).
    let media_w = page_w + 2.0 * bleed;
    let media_h = page_h + 2.0 * bleed;

    let mut scene = Scene::new(media_w, media_h);

    // ── Step 4: outermost media-edge clip (normative rule) ────────
    // The clip covers the entire media box so content and background may bleed
    // into the margin. With bleed = 0 this is exactly the page rectangle.
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: media_w,
        h: media_h,
    });

    // ── Step 5: optional page background (fills the entire media box) ────
    if let Some(bg_prop) = &page.background {
        if let Some(gradient) = resolve_property_gradient(bg_prop, resolved, &page.id) {
            // Page background applies no opacity cascade (mirrors the solid path).
            scene.commands.push(SceneCommand::FillRect {
                x: 0.0,
                y: 0.0,
                w: media_w,
                h: media_h,
                paint: Paint::Gradient(gradient),
            });
        } else if let Some(color) =
            resolve_property_color(bg_prop, resolved, &mut diagnostics, &page.id)
        {
            scene.commands.push(SceneCommand::FillRect {
                x: 0.0,
                y: 0.0,
                w: media_w,
                h: media_h,
                paint: Paint::solid(color),
            });
        }
    }

    // ── Step 5b: anchor pre-pass (PAGE-LOCAL) ────────────────────────────
    // Walk page top-level children once, building a map from node id to the
    // derived (x, y) for nodes that carry a recognized `anchor` attribute and
    // have px-resolvable w/h. Built once; threaded read-only into compile_node.
    let anchors = build_anchor_map(page, page_w, page_h, resolved);

    // ── Step 6: threaded-text chain pre-pass (DOCUMENT-WIDE) ─────────────
    // Resolve every text chain ONCE across ALL pages (deterministic
    // page-then-source-order walk into frames + groups), distributing each
    // chain's source article across every member's box — flowing across page
    // boundaries. The map is keyed by global node id; this page's nodes look up
    // the slice assigned to them. Chains' diagnostics (e.g. a source font
    // fallback) are document-wide and would otherwise be emitted once per page;
    // they are collected into a throwaway buffer here and only the diagnostics
    // attributable to THIS page's chain members would be surfaced — but since
    // distribution is global, we keep the page-local behaviour deterministic by
    // discarding the pre-pass's own advisories on non-zero pages (they were
    // already surfaced on page 0). Page 0 keeps them.
    let engine = RustybuzzEngine::new();
    let mut chain_diags: Vec<Diagnostic> = Vec::new();
    let chains = resolve_chains_document(
        doc,
        resolved,
        &style_map,
        fonts,
        &engine,
        &md_blocks,
        &mut chain_diags,
    );
    // Multi-page table flow pre-pass (DOCUMENT-WIDE), built ONCE like the chain
    // map and threaded identically into every `compile_node`. Its advisories are
    // document-wide; like the chain diags they surface only on page 0.
    let flows = resolve_table_flows(doc, resolved, &style_map, fonts, &engine, &mut chain_diags);
    if page_index == 0 {
        diagnostics.extend(chain_diags);
    }

    // ── Step 7: build the per-page field context ─────────────────────────
    // The 1-based page index drives the folio + parity (recto = odd, verso =
    // even). The live area mirrors the validator's margin formula so an omitted
    // field x/w auto-mirrors recto/verso via the page margins.
    let page_index_1based = page_index + 1;
    // Single source of truth for parity (explicit page.parity > document
    // page-parity-start > default index%2==1). Mirrors the validator.
    let is_recto = doc.page_is_recto(page, page_index_1based);
    let mirror_margins = doc.mirror_margins.unwrap_or(false);
    // RTL book: the binding margin is mirrored to the opposite side (recto →
    // inner-on-right). Matches the validator's `margin.rs` parity.
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

    // ── Step 7b: collect this page's footnote markers ────────────────────
    // Every `footnote` DIRECT child of the page is auto-numbered 1..N in source
    // order (an explicit `marker` overrides the number but keeps its slot). The
    // ordered map drives both the inline superscript markers (a text span's
    // `footnote_ref` keys in) and the bottom-zone rendering below.
    let footnote_markers = footnote::collect_footnote_markers(page);

    // ── Step 7c: build this page's node bounding-box map ─────────────────
    // Maps every id-bearing page node with a resolvable x/y/w/h rect to its
    // ABSOLUTE page-coordinate box, accumulating group/instance translation
    // (frames are clip-only). Drives text-runaround exclusion lookup. Empty when
    // no node carries a complete rect (byte-identical to before for any text node
    // without `text-exclusion`).
    let node_boxes = build_node_boxes(page, resolved, &component_map, &import_scopes);
    let connector_targets =
        build_connector_targets(page, &node_boxes, resolved, &component_map, &import_scopes);
    let port_map = build_port_map(page, &component_map, &import_scopes);

    // ── Step 7d: compute section assignments (document-wide, one-shot) ───
    // Precompute once (outside any inner loop — this is the single page compile
    // entry point): maps each 0-based page index to its section assignment.
    // The lifetime of the returned assignments is tied to `doc`, which outlives
    // the compile function.
    let section_assignments = build_section_assignments(doc);
    let section_assign = section_assignments.get(page_index).and_then(|opt| *opt);

    let field_ctx = FieldCtx {
        page_index_1based,
        is_recto,
        live_area,
        page_index_by_node_id: &page_index_by_node_id,
        footnote_markers: &footnote_markers,
        node_boxes: &node_boxes,
        connector_target_kinds: &connector_targets.kinds,
        connector_outline_boxes: &connector_targets.outline_boxes,
        port_map: &port_map,
        total_pages: doc.body.pages.len(),
        pages: &doc.body.pages,
        section_page_index: section_assign.map(|a| a.page_index_in_section),
        section_page_count: section_assign.map(|a| a.page_count),
        section_folio_start: section_assign.map(|a| a.folio_start),
        section_folio_style: section_assign.and_then(|a| a.folio_style),
        section_name: section_assign.map(|a| a.name),
    };

    // Bundle the page-wide immutable lookups once; threaded read-only into every
    // top-level `compile_node` (master projection + page children) and cascaded
    // unchanged down the container/table recursion.
    let node_cx = NodeCtx {
        resolved,
        style_map: &style_map,
        components: &component_map,
        imports: &import_scopes,
        fonts,
        engine: &engine,
        chains: &chains,
        flows: &flows,
        anchors: &anchors,
        field_ctx: &field_ctx,
        md_blocks: &md_blocks,
        page_block_styles: &page.block_styles,
        doc_block_styles: &doc.body.block_styles,
    };

    // ── Resolve the page baseline grid ───────────────────────────────────
    // A page may declare `baseline-grid=(px)14`. When it resolves to a positive
    // pixel value `g`, every text node on this page snaps its line baselines
    // onto the grid `{0, g, 2g, …}` (see [`RenderCtx::baseline_grid`]). An
    // absent / unresolvable / non-positive value yields `None`, byte-identical
    // to a page with no grid.
    let baseline_grid: Option<f64> = page
        .baseline_grid
        .as_ref()
        .and_then(|d| dim_to_px(d.value, &d.unit))
        .filter(|g| g.is_finite() && *g > 0.0);

    let mut root_ctx = if bleed > 0.0 {
        // Shift authored coordinates into the trim box. With bleed = 0 this is
        // the identity root context (byte-identical to before).
        RenderCtx::root_offset(bleed, bleed)
    } else {
        RenderCtx::root()
    };
    // Thread the grid into BOTH the bleed and no-bleed root contexts. The grid
    // is measured in the post-`dy` (shifted) coordinate space, the same space
    // the emitted baselines live in, so a bleed-shifted page snaps consistently.
    root_ctx.baseline_grid = baseline_grid;

    // Absolute indices, in document order, of the `StrokePolyline` emitted by
    // each top-level connector (master-projected then page-own). Used only by
    // the opt-in line-jump post-pass; empty/unused when the page declares no
    // `line-jumps`, so the rest of compile is byte-identical.
    let mut connector_strokes: Vec<usize> = Vec::new();

    // ── Step 7a: project the page's master (UNDER its own children) ──────
    // When `page.master` names a declared master, clone the master's children,
    // prefix every projected id with the page id (avoid cross-page collisions),
    // and compile them BEFORE the page's own children so running heads / folios
    // sit behind body text. Fields inside the master resolve against THIS page.
    // An unknown master reference is a hard validation error; here it is simply
    // skipped (the compiler never panics on bad references).
    if let Some(master_id) = &page.master
        && let Some(master) = master_map.get(master_id.as_str())
    {
        let mut projected = master.children.clone();
        let prefix = format!("{}/", page.id);
        container::prefix_ids_in_children(&mut projected, &prefix);
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
            data,
            graph: imports,
            scopes: &import_scopes,
        },
        &mut scene.commands,
        &mut diagnostics,
    );

    // ── Step 7b: page children in source order (z-order: first = bottom) ─
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

    // ── Step 7b′: opt-in connector line-jumps (hops at crossings) ────────
    // Only "arc"/"gap" run; "none"/an unrecognized value/absent leaves the
    // commands untouched, so a page without `line-jumps` is byte-identical.
    if let Some(mode) = page.line_jumps.as_deref()
        && (mode == "arc" || mode == "gap")
    {
        line_jumps::apply_line_jumps(&mut scene.commands, &connector_strokes, mode);
    }

    // ── Step 7c: footnote zone (page furniture, above the bottom margin) ─
    // Rendered AFTER the page's own children (so it paints on top of body
    // content) but inside the media clip. Draws the separator rule plus the
    // stacked, auto-numbered footnotes; warns on body/zone overlap. A page with
    // no footnotes emits nothing here (byte-identical to before).
    footnote::compile_footnote_zone(
        page,
        live_area,
        footnote::FootnoteZoneEnv {
            markers: &footnote_markers,
            resolved,
            style_map: &style_map,
            fonts,
            engine: &engine,
            chains: &chains,
            anchors: &anchors,
            field_ctx: &field_ctx,
        },
        &mut scene.commands,
        &mut diagnostics,
        root_ctx,
    );

    // ── Step 8: close the outermost clip ─────────────────────────────────
    scene.commands.push(SceneCommand::PopClip);

    // ── Step 9: crop / trim marks (only when a bleed is active) ──────────
    // Emitted AFTER content and OUTSIDE the clip so the marks sit on top and
    // live entirely in the bleed margin at the four trim corners.
    if bleed > 0.0 {
        crop::emit_crop_marks(&mut scene.commands, bleed, page_w, page_h);
    }

    // ── Step 10: print trim box ──────────────────────────────────────────
    // When a bleed is active the media box (`scene.width`/`height`) includes
    // the bleed on all four sides; the trim box is the inner page rectangle
    // `[b, b, page_w, page_h]` that the finished piece is cut to. Backends that
    // care about print boxes (PDF) read this; raster backends ignore it. With
    // bleed = 0 the trim box equals the media box, so we leave `trim` as `None`.
    if bleed > 0.0 {
        scene.trim = Some(Rect {
            x: bleed,
            y: bleed,
            w: page_w,
            h: page_h,
        });
    }

    CompileResult { scene, diagnostics }
}
