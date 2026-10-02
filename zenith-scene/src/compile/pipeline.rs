//! Render context, compile result, style lookup, and the one-page compile entry.
//!
//! Entry: [`compile_page_inner`]. Public wrappers live in [`super::entry`]. The
//! page emit lives in [`super::session`].

use std::collections::BTreeMap;

use zenith_core::{DataContext, Diagnostic, Document, FontProvider, PropertyValue, Style};

use crate::ir::Scene;

use super::imports::ImportGraph;
use super::session::{DocumentPrep, PageCompiler};

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
    pub(in crate::compile) fn root() -> Self {
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
    pub(in crate::compile) fn root_offset(dx: f64, dy: f64) -> Self {
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

/// Compile one page with a fresh [`DocumentPrep`] and [`PageCompiler`].
///
/// Callers that compile several pages of one document build the two types
/// once and call [`PageCompiler::compile_page`] per page instead.
pub(in crate::compile) fn compile_page_inner(
    doc: &Document,
    fonts: &dyn FontProvider,
    page_index: usize,
    data: Option<&DataContext>,
    imports: Option<&ImportGraph<'_>>,
) -> CompileResult {
    let prep = DocumentPrep::new(doc, data, imports);
    PageCompiler::new(&prep, fonts).compile_page(page_index)
}
