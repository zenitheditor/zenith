//! Scene compilation: `Document` → `CompileResult`.
//!
//! Entry point: [`compile`].
//!
//! Rect, ellipse, line, text, code, and group nodes are compiled; the page
//! background is emitted first; unknown nodes produce an advisory diagnostic
//! and are skipped.
//!
//! [`compile`] renders page 0; [`compile_page`] renders a chosen page by index.
//!
//! The compiler is split across submodules: [`leaf`] (rect/ellipse/line/
//! polygon/polyline), [`text`] (text + code shaping), [`container`] (group +
//! frame), [`image`], [`paint`] (color/gradient/shadow resolvers), and
//! [`util`] (small geometry/diagnostic helpers). This module keeps the public
//! entry points, the per-subtree [`RenderCtx`], and the [`compile_node`]
//! dispatcher that routes each node kind to its submodule.

mod container;
mod image;
mod leaf;
mod paint;
mod text;
mod util;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use zenith_core::{
    Diagnostic, Document, FontProvider, Node, PropertyValue, ResolvedToken, Style, dim_to_px,
    resolve_tokens,
};
use zenith_layout::RustybuzzEngine;

use crate::ir::{Scene, SceneCommand};

use container::{compile_frame, compile_group};
use image::compile_image;
use leaf::{compile_ellipse, compile_line, compile_polygon, compile_polyline, compile_rect};
use paint::{resolve_property_color, resolve_property_gradient};
use text::{compile_code, compile_text};

// ── Render context ────────────────────────────────────────────────────────────

/// Per-subtree rendering context that cascades through the node tree.
///
/// Each field accumulates transformations as we descend:
/// - `opacity` — multiplied together at each group boundary; leaf nodes
///   apply it on top of their own node-level opacity.
/// - `dx`/`dy` — translation offset accumulated from all ancestor groups
///   with an `x`/`y` property; added to every leaf geometry position.
#[derive(Clone, Copy)]
pub(super) struct RenderCtx {
    /// Accumulated opacity multiplier (1.0 = fully opaque).
    pub(super) opacity: f64,
    /// Accumulated x-translation in pixels.
    pub(super) dx: f64,
    /// Accumulated y-translation in pixels.
    pub(super) dy: f64,
}

impl RenderCtx {
    fn root() -> Self {
        RenderCtx {
            opacity: 1.0,
            dx: 0.0,
            dy: 0.0,
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
pub(super) fn style_prop<'a>(
    style_ref: &Option<String>,
    style_map: &'a BTreeMap<&str, &Style>,
    key: &str,
) -> Option<&'a PropertyValue> {
    let sid = style_ref.as_deref()?;
    style_map.get(sid)?.properties.get(key)
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Compile `doc` into a [`CompileResult`], using `fonts` to shape text nodes.
///
/// [`compile_page`] renders a chosen page; this wrapper renders page 0.  If the
/// document has no pages an empty scene is returned with an advisory diagnostic.
///
/// Pass `&zenith_core::default_provider()` to use the bundled Noto Sans
/// font, which is sufficient for basic text rendering.
///
/// # No-panic guarantee
///
/// This function never calls `unwrap`, `expect`, `panic!`, `todo!`,
/// `unimplemented!`, or performs unchecked indexing.  All failure paths push a
/// diagnostic and continue.
pub fn compile(doc: &Document, fonts: &dyn FontProvider) -> CompileResult {
    compile_page(doc, fonts, 0)
}

/// Compile the page at `page_index` (0-based) of `doc` into a [`CompileResult`],
/// using `fonts` to shape text nodes.
///
/// If the document has no pages an empty scene is returned with a
/// `scene.no_pages` advisory; if `page_index` is out of range (but pages exist)
/// an empty scene is returned with a `scene.page_out_of_range` advisory.
///
/// Pass `&zenith_core::default_provider()` to use the bundled Noto Sans
/// font, which is sufficient for basic text rendering.
///
/// # No-panic guarantee
///
/// This function never calls `unwrap`, `expect`, `panic!`, `todo!`,
/// `unimplemented!`, or performs unchecked indexing (page lookup uses `.get()`).
/// All failure paths push a diagnostic and continue.
pub fn compile_page(doc: &Document, fonts: &dyn FontProvider, page_index: usize) -> CompileResult {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();

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

    let mut scene = Scene::new(page_w, page_h);

    // ── Step 4: outermost page-edge clip (doc 09 normative rule) ─────────
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: page_w,
        h: page_h,
    });

    // ── Step 5: optional page background ─────────────────────────────────
    if let Some(bg_prop) = &page.background {
        if let Some(gradient) = resolve_property_gradient(bg_prop, resolved, &page.id) {
            // Page background applies no opacity cascade (mirrors the solid path).
            scene.commands.push(SceneCommand::FillRectGradient {
                x: 0.0,
                y: 0.0,
                w: page_w,
                h: page_h,
                gradient,
            });
        } else if let Some(color) =
            resolve_property_color(bg_prop, resolved, &mut diagnostics, &page.id)
        {
            scene.commands.push(SceneCommand::FillRect {
                x: 0.0,
                y: 0.0,
                w: page_w,
                h: page_h,
                color,
            });
        }
    }

    // ── Step 6: children in source order (z-order: first = bottom) ───────
    let engine = RustybuzzEngine::new();
    for node in &page.children {
        compile_node(
            node,
            resolved,
            &style_map,
            fonts,
            &engine,
            &mut scene.commands,
            &mut diagnostics,
            RenderCtx::root(),
        );
    }

    // ── Step 7: close the outermost clip ─────────────────────────────────
    scene.commands.push(SceneCommand::PopClip);

    CompileResult { scene, diagnostics }
}

// ── Node dispatch ─────────────────────────────────────────────────────────────

/// The `role` of any node, if set. Used to exclude non-printing nodes
/// (`role="guide"`) from render output.
fn node_role(node: &Node) -> Option<&str> {
    match node {
        Node::Rect(n) => n.role.as_deref(),
        Node::Ellipse(n) => n.role.as_deref(),
        Node::Line(n) => n.role.as_deref(),
        Node::Text(n) => n.role.as_deref(),
        Node::Code(n) => n.role.as_deref(),
        Node::Frame(n) => n.role.as_deref(),
        Node::Group(n) => n.role.as_deref(),
        Node::Image(n) => n.role.as_deref(),
        Node::Polygon(n) => n.role.as_deref(),
        Node::Polyline(n) => n.role.as_deref(),
        Node::Unknown(_) => None,
    }
}

/// Route a single node to the submodule that compiles its kind.
///
/// Each arm forwards the full cascade context to a `compile_*` function; the
/// emitted `SceneCommand` stream is identical to the previous inline match.
#[allow(clippy::too_many_arguments)]
pub(super) fn compile_node(
    node: &Node,
    resolved: &BTreeMap<String, ResolvedToken>,
    style_map: &BTreeMap<&str, &Style>,
    fonts: &dyn FontProvider,
    engine: &RustybuzzEngine,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: RenderCtx,
) {
    // Non-printing guide nodes (`role="guide"`) are excluded from render output
    // entirely — including their subtree when the guide is a group/frame.
    if node_role(node) == Some("guide") {
        return;
    }

    match node {
        Node::Rect(rect) => {
            compile_rect(rect, resolved, style_map, commands, diagnostics, ctx);
        }
        Node::Ellipse(ellipse) => {
            compile_ellipse(ellipse, resolved, style_map, commands, diagnostics, ctx);
        }
        Node::Text(text) => {
            compile_text(
                text,
                resolved,
                style_map,
                fonts,
                engine,
                commands,
                diagnostics,
                ctx,
            );
        }
        Node::Line(line) => {
            compile_line(line, resolved, style_map, commands, diagnostics, ctx);
        }
        Node::Frame(frame) => {
            compile_frame(
                frame,
                resolved,
                style_map,
                fonts,
                engine,
                commands,
                diagnostics,
                ctx,
            );
        }
        Node::Group(group) => {
            compile_group(
                group,
                resolved,
                style_map,
                fonts,
                engine,
                commands,
                diagnostics,
                ctx,
            );
        }
        Node::Image(image) => {
            compile_image(image, resolved, commands, diagnostics, ctx);
        }
        Node::Polygon(poly) => {
            compile_polygon(poly, resolved, style_map, commands, diagnostics, ctx);
        }
        Node::Polyline(poly) => {
            compile_polyline(poly, resolved, style_map, commands, diagnostics, ctx);
        }
        Node::Code(code) => {
            compile_code(
                code,
                resolved,
                style_map,
                fonts,
                engine,
                commands,
                diagnostics,
                ctx,
            );
        }
        Node::Unknown(unknown) => {
            diagnostics.push(Diagnostic::advisory(
                "scene.unsupported_node",
                format!(
                    "unknown node kind '{}' cannot be compiled; the node is skipped \
                     (forward-compatibility: this kind may be supported in a later version)",
                    unknown.kind
                ),
                unknown.source_span,
                None,
            ));
        }
    }
}
