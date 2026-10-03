//! The public `compile_text` entry and the `overflow="autofit"` shrink-to-fit
//! search. Both are thin wrappers over [`compile_text_sized`](super::fit::compile_text_sized).

use zenith_core::{Diagnostic, TextNode};

use crate::compile::RenderCtx;
use crate::compile::text::ctx::TextCompileEnv;
use crate::compile::text::measure::font_size_px;
use crate::compile::text::overflow_mode::TextOverflow;
use crate::compile::util::{resolve_geometry_px, resolve_property_dimension_px};
use crate::ir::SceneCommand;

use super::fit::{compile_text_sized, largest_fitting_px, with_font_size};

/// Compile a `text` leaf node.
///
/// This is the public entry point. It is a thin BLACK-BOX wrapper around
/// [`compile_text_sized`](super::fit::compile_text_sized) (which carries every
/// layout path verbatim):
///
/// - For any node whose `overflow` is NOT `"autofit"` it is a pure pass-through
///   — it forwards every argument unchanged to `compile_text_sized`.
/// - For `overflow="autofit"` it lays the node out at TRIAL font sizes (into
///   throwaway buffers) to find the LARGEST size in `[floor, declared]` whose
///   content fits the box, then performs the single real emit at that size.
///   See [`compile_text_autofit`].
///
/// Returns the laid-out content height in pixels (`line_count * line_height`).
pub(in crate::compile) fn compile_text(
    text: &TextNode,
    env: TextCompileEnv,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: RenderCtx,
) -> f64 {
    // Emit, then (only when the node opts out of selectable text) downgrade this
    // node's glyph runs to outlines. `selectable` is purely a PDF render concern,
    // so it never affects layout — it is applied as a post-pass over exactly the
    // commands this node produced. Default (`None`/`Some(true)`) is byte-identical.
    let start = commands.len();
    let height = match TextOverflow::from_attr(text.overflow.as_deref()) {
        // Pass-through: one sized compile for every non-autofit node.
        TextOverflow::Clip | TextOverflow::Visible | TextOverflow::Fit => {
            compile_text_sized(text, env, commands, diagnostics, ctx)
        }
        TextOverflow::Autofit => compile_text_autofit(text, env, commands, diagnostics, ctx),
    };
    if text.selectable == Some(false) {
        crate::compile::text::shape::mark_runs_unselectable(&mut commands[start..]);
    }
    height
}

/// PowerPoint-style shrink-to-fit for an `overflow="autofit"` text node.
///
/// Runs [`largest_fitting_px`](super::fit::largest_fitting_px) over
/// `[floor, declared]` to find the LARGEST integer-px size whose content fits
/// the box, then performs ONE real emit at that size.
///
/// - The declared node font size (px) is the search ceiling; `font-size-min`
///   (token → dimension) is the floor. When `font-size-min` is absent the floor
///   defaults to `(declared * 0.5).max(8.0)`.
/// - Both `box_w` and `box_h` must resolve; if either is missing autofit cannot
///   measure, so it falls back to a single `compile_text_sized` call (no crash,
///   no silent skip).
/// - If some size fits, the real emit at that size overflows nothing, so it
///   emits no clip and no diagnostic. If NONE fits (even at the floor) the real
///   emit uses the floor and `compile_text_sized` raises `text.fit_failed`,
///   continuing the same downward scan below the floor to name the
///   `font-size-min` that fits. Overflow at the floor is never clipped.
///
/// v0 limitation: a span carrying its OWN explicit `font-size` does not scale —
/// only the node-level font size drives inheriting spans (the typical single-
/// span title inherits, so it scales).
fn compile_text_autofit(
    text: &TextNode,
    env: TextCompileEnv,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: RenderCtx,
) -> f64 {
    // Require both box dimensions to measure fit; otherwise fall back to a
    // single sized compile (documented; no crash).
    let box_w = resolve_geometry_px(text.w.as_ref(), env.resolved);
    let box_h = resolve_geometry_px(text.h.as_ref(), env.resolved);
    let (Some(_bw), Some(_bh)) = (box_w, box_h) else {
        return compile_text_sized(text, env, commands, diagnostics, ctx);
    };

    // Resolve the declared node font size (px) — the search ceiling — and the
    // floor from `font-size-min`, defaulting to `(declared * 0.5).max(8.0)`.
    let declared = f64::from(font_size_px(text, env.resolved, env.style_map));
    let floor = resolve_property_dimension_px(
        text.font_size_min.as_ref(),
        env.resolved,
        (declared * 0.5).max(8.0),
    );
    // Integer-px search bounds. Clamp the floor at/below the ceiling.
    let ceil_px = declared.floor().max(1.0) as i64;
    let floor_px = floor.floor().max(1.0).min(declared.floor().max(1.0)) as i64;

    // Real emit: the fitted size, or the floor (where the genuine
    // `text.fit_failed` surfaces). The mode stays `autofit` on the clone.
    let real_fs = largest_fitting_px(text, env, ctx, ceil_px, floor_px).unwrap_or(floor_px);
    let real = with_font_size(text, real_fs as f64);
    compile_text_sized(&real, env, commands, diagnostics, ctx)
}
