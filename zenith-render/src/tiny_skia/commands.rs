//! Drawing-command dispatch and shared draw context.
//!
//! [`draw_command`] routes each *drawing* [`SceneCommand`] to its handler in
//! the [`draw`](super::draw) submodules. Structural / capture commands
//! (clip, transform, layer, and the blur/shadow/filter/mask brackets) are
//! handled by the render loop in [`backend`](super::backend) and reach this
//! dispatcher only as already-consumed markers — they are matched explicitly
//! (no wildcard over [`SceneCommand`]) and no-op here.

use tiny_skia::{LineCap, StrokeDash, Transform};
use zenith_core::{AssetProvider, FontProvider};
use zenith_scene::{LineCap as IrLineCap, SceneCommand};

use super::draw;

/// Read-only per-command draw context shared by every handler.
///
/// `current_ts` is the active affine transform; `effective_clip` is the top of
/// the clip stack (already intersected with all enclosing clips); `width` /
/// `height` are the pixmap dimensions. A `Copy` bundle so handlers take few
/// arguments without re-deriving any of these per arm — byte-identical to the
/// prior inline reads.
#[derive(Clone, Copy)]
pub(in crate::tiny_skia) struct DrawCtx {
    pub(in crate::tiny_skia) current_ts: Transform,
    pub(in crate::tiny_skia) effective_clip: (f64, f64, f64, f64),
    pub(in crate::tiny_skia) width: u32,
    pub(in crate::tiny_skia) height: u32,
}

/// Dispatch one drawing command to its handler.
///
/// Exhaustive over [`SceneCommand`]: every drawing variant routes to a
/// [`draw`] handler; `DrawSvgAsset` is a documented v0 no-op here (the
/// compiler pre-resolves SVG assets to `DrawImage`); and the structural /
/// capture variants are consumed by the render loop before dispatch, so they
/// no-op (matched explicitly, never via a wildcard).
pub(in crate::tiny_skia) fn draw_command(
    target: &mut tiny_skia::Pixmap,
    ctx: DrawCtx,
    cmd: &SceneCommand,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
    svg_fontdb: &mut Option<resvg::usvg::fontdb::Database>,
) {
    match cmd {
        SceneCommand::FillRect { .. } => draw::shapes::fill_rect(target, ctx, cmd),
        SceneCommand::FillEllipse { .. } => draw::shapes::fill_ellipse(target, ctx, cmd),
        SceneCommand::StrokeEllipse { .. } => draw::shapes::stroke_ellipse(target, ctx, cmd),
        SceneCommand::StrokeLine { .. } => draw::shapes::stroke_line(target, ctx, cmd),
        SceneCommand::FillPolygon { .. } => draw::shapes::fill_polygon(target, ctx, cmd),
        SceneCommand::StrokePolyline { .. } => draw::shapes::stroke_polyline(target, ctx, cmd),
        SceneCommand::StrokeRect { .. } => draw::shapes::stroke_rect(target, ctx, cmd),
        SceneCommand::FillRoundedRect { .. } => draw::shapes::fill_rounded_rect(target, ctx, cmd),
        SceneCommand::StrokeRoundedRect { .. } => {
            draw::shapes::stroke_rounded_rect(target, ctx, cmd)
        }
        SceneCommand::DrawGlyphRun { .. } => draw::text::draw_glyph_run(target, ctx, cmd, fonts),
        SceneCommand::DrawImage { .. } => {
            draw::image::draw_image(target, ctx, cmd, assets, fonts, svg_fontdb)
        }

        // SVG assets are pre-resolved to a raster (DrawImage) by the compiler;
        // the raster scene IR never emits this variant. Matched explicitly (no
        // wildcard) and a no-op — the documented v0 limitation, byte-identical
        // to the prior `_ => {}` fall-through that also dropped it.
        SceneCommand::DrawSvgAsset { .. } => {}

        // Structural / capture commands are handled by the render loop before
        // this dispatcher runs (they mutate the clip/transform/capture/layer
        // stacks and `continue`). They are listed here only to keep the match
        // exhaustive over `SceneCommand`; reaching them is impossible, and a
        // no-op is the safe identity.
        SceneCommand::PushClip { .. }
        | SceneCommand::PopClip
        | SceneCommand::PushLayer { .. }
        | SceneCommand::PopLayer
        | SceneCommand::PushTransform { .. }
        | SceneCommand::PopTransform
        | SceneCommand::BeginShadow { .. }
        | SceneCommand::EndShadow
        | SceneCommand::BeginBlur { .. }
        | SceneCommand::EndBlur
        | SceneCommand::BeginFilter { .. }
        | SceneCommand::EndFilter
        | SceneCommand::BeginMask { .. }
        | SceneCommand::EndMask => {}
    }
}

// ── Dashed stroke helpers ─────────────────────────────────────────────────────

/// Map an IR [`IrLineCap`] to the tiny-skia [`LineCap`].
///
/// `None` → `LineCap::Butt` (the tiny-skia default; byte-identical to the
/// prior `Stroke::default()` behavior).
pub(in crate::tiny_skia) fn map_line_cap(lc: Option<IrLineCap>) -> LineCap {
    match lc {
        Some(IrLineCap::Round) => LineCap::Round,
        Some(IrLineCap::Square) => LineCap::Square,
        // Butt or absent — matches Stroke::default().line_cap.
        Some(IrLineCap::Butt) | None => LineCap::Butt,
    }
}

/// Build a [`StrokeDash`] from resolved dash/gap pixel values.
///
/// Returns `None` (solid stroke) when `dash` is `None` or `<= 0`.
/// `StrokeDash::new` returns `None` for invalid intervals, which collapses to
/// a solid stroke — an acceptable safe fallback.
pub(in crate::tiny_skia) fn build_stroke_dash(
    dash: Option<f64>,
    gap: Option<f64>,
) -> Option<StrokeDash> {
    let d = dash?;
    if d <= 0.0 {
        return None;
    }
    let g = gap.unwrap_or(d).max(0.0);
    StrokeDash::new(vec![d as f32, g as f32], 0.0)
}
