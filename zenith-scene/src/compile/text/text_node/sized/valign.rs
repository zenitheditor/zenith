//! The `v-align` pre-offset of a sized node.

use zenith_core::{Diagnostic, TextNode};

use crate::compile::text::ctx::TextCompileEnv;
use crate::compile::text::measure::{MeasureEnv, measure_text_wrapped_height};
use crate::compile::util::resolve_geometry_px;

/// The px to move the first line down for `v-align="middle"` / `"bottom"`.
///
/// `None` when `v-align` is absent, `top`, or unrecognized, or when the box
/// width or height does not resolve. Mirrors the shape node's label v-align:
/// measure the wrapped height, then apply `(box_h - wrapped_h) / 2` or
/// `box_h - wrapped_h`. Chain members use a pre-distributed layout and never
/// reach this.
pub(super) fn v_align_offset(
    text: &TextNode,
    families: &[String],
    env: TextCompileEnv,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<f64> {
    if !matches!(text.v_align.as_deref(), Some("middle") | Some("bottom")) {
        return None;
    }
    let box_h = resolve_geometry_px(text.h.as_ref(), env.resolved)?;
    let box_w = resolve_geometry_px(text.w.as_ref(), env.resolved)?;
    let wrapped_h = measure_text_wrapped_height(
        text,
        box_w,
        families,
        MeasureEnv {
            resolved: env.resolved,
            style_map: env.style_map,
            fonts: env.fonts,
            engine: env.engine,
        },
        diagnostics,
    )
    .unwrap_or(0.0);
    Some(match text.v_align.as_deref() {
        Some("bottom") => (box_h - wrapped_h).max(0.0),
        // "middle" and any other matched arm center vertically.
        _ => ((box_h - wrapped_h) / 2.0).max(0.0),
    })
}
