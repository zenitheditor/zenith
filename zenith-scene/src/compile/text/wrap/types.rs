//! Borrowed environment and geometry bundles for the WRAP path.

use std::collections::BTreeMap;

use zenith_core::{PropertyValue, ResolvedToken};
use zenith_layout::{KerningPairAdjustment, TextDirection};

use crate::compile::RenderCtx;
use crate::compile::text::ctx::ShapeEnv;
use crate::ir::Color;

/// The borrowed environment + node-level style the wrap path needs beyond the
/// resolved spans and the geometry. Bundled so [`emit_wrap_path`] stays under the
/// argument lint. `node_fill_prop`/`node_weight_prop`/`color_opacity` drive the
/// bullet marker resolution; `node_boxes` resolves the text-exclusion target.
#[derive(Clone, Copy)]
pub(in crate::compile) struct WrapEnv<'a> {
    pub(in crate::compile) env: ShapeEnv<'a>,
    pub(in crate::compile) resolved: &'a BTreeMap<String, ResolvedToken>,
    pub(in crate::compile) node_boxes: &'a BTreeMap<String, (f64, f64, f64, f64)>,
    pub(in crate::compile) node_fill_prop: Option<&'a PropertyValue>,
    pub(in crate::compile) node_weight_prop: Option<&'a PropertyValue>,
    pub(in crate::compile) color_opacity: f64,
    pub(in crate::compile) ctx: RenderCtx,
}

/// The resolved geometry + style scalars for the wrap path. `font_size`,
/// `align`, `deco_thickness`, `direction`, and `glyph_stroke` are the same
/// scalars the emit consumes; `box_w`/`box_h_opt` bound the measure; `text_x`/
/// `text_y` are the post-`ctx.dy` origin.
#[derive(Clone, Copy)]
pub(in crate::compile) struct WrapGeom<'a> {
    pub(in crate::compile) text_x: f64,
    pub(in crate::compile) text_y: f64,
    pub(in crate::compile) box_w: f64,
    pub(in crate::compile) box_h_opt: Option<f64>,
    pub(in crate::compile) font_size: f32,
    pub(in crate::compile) letter_spacing_px: f32,
    pub(in crate::compile) kerning_pairs: &'a [KerningPairAdjustment],
    pub(in crate::compile) align: &'a str,
    pub(in crate::compile) deco_thickness: f64,
    pub(in crate::compile) direction: TextDirection,
    pub(in crate::compile) glyph_stroke: (Option<Color>, Option<f64>),
}

/// What the wrap path laid out: the line count, and the content height when
/// the page baseline grid snapped the lines.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::compile) struct WrapOutcome {
    /// Laid-out line count.
    pub(in crate::compile) lines: usize,
    /// Under a baseline-grid snap: the snap offset of the first line plus
    /// `lines` times the snapped advance, from the node's text top. `None`
    /// when no snap applied.
    pub(in crate::compile) grid_height: Option<f64>,
}
