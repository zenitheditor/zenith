//! `Copy` bundles for the sized layout: the resolved node style and the box
//! layout. They replace long loose argument lists between the sized steps.

use std::collections::BTreeMap;

use zenith_core::{Diagnostic, PropertyValue, ResolvedToken, TextNode};
use zenith_layout::{FontFeature, KerningPairAdjustment, TextDirection};

use crate::compile::RenderCtx;
use crate::compile::paint::resolve_property_color;
use crate::compile::text::ctx::{ShapeEnv, TextCompileEnv};
use crate::compile::util::resolve_property_dimension_px;
use crate::ir::{BlendMode, Color};

/// The node-level style every sized step reads: the compile env, the resolved
/// font/fill/weight/spacing cascade, opacity, blend, and glyph stroke.
#[derive(Clone, Copy)]
pub(super) struct NodeStyle<'a> {
    pub(super) text: &'a TextNode,
    pub(super) env: TextCompileEnv<'a>,
    pub(super) ctx: RenderCtx,
    pub(super) families: &'a [String],
    pub(super) font_size: f32,
    pub(super) fill_prop: Option<&'a PropertyValue>,
    pub(super) weight_prop: Option<&'a PropertyValue>,
    pub(super) features: &'a [FontFeature],
    pub(super) letter_spacing_prop: Option<&'a PropertyValue>,
    pub(super) letter_spacing_px: f32,
    pub(super) kerning_pairs: &'a [KerningPairAdjustment],
    pub(super) direction: TextDirection,
    pub(super) glyph_stroke: (Option<Color>, Option<f64>),
    /// Node opacity alone (`opacity=`), before the `ctx` cascade.
    pub(super) node_opacity: f64,
    /// The non-normal blend mode, when a layer bracket is active.
    pub(super) blend: Option<BlendMode>,
    /// The alpha the blend layer composites at (`node_opacity * ctx.opacity`).
    pub(super) layer_opacity: f64,
    /// The alpha baked into glyph colors: `1.0` under a blend layer.
    pub(super) color_opacity: f64,
}

impl NodeStyle<'_> {
    /// The shaping engine and font provider.
    pub(super) fn shape(&self) -> ShapeEnv<'_> {
        ShapeEnv {
            engine: self.env.engine,
            fonts: self.env.fonts,
        }
    }
}

/// The box geometry of a sized node: origin, resolved box size, alignment, and
/// decoration thickness.
#[derive(Clone, Copy)]
pub(super) struct BoxLayout<'a> {
    pub(super) text_x: f64,
    /// First line top (box top plus any `v-align` offset).
    pub(super) text_y: f64,
    pub(super) box_w: Option<f64>,
    pub(super) box_h: Option<f64>,
    pub(super) align: &'a str,
    pub(super) deco_thickness: f64,
}

/// Resolve the glyph stroke: `None` fields when the node carries no
/// `stroke` / `stroke-width`.
pub(super) fn resolve_glyph_stroke(
    text: &TextNode,
    resolved: &BTreeMap<String, ResolvedToken>,
    diagnostics: &mut Vec<Diagnostic>,
) -> (Option<Color>, Option<f64>) {
    let color = text
        .stroke
        .as_ref()
        .and_then(|p| resolve_property_color(p, resolved, diagnostics, &text.id));
    let width = resolve_property_dimension_px(text.stroke_width.as_ref(), resolved, -1.0);
    (color, if width > 0.0 { Some(width) } else { None })
}
