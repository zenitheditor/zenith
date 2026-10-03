//! Shared value types of the contrast check: paint context, backdrop
//! candidates, sampled colours, and the walk environment.

use std::collections::BTreeMap;

use crate::ast::style::Style;
use crate::tokens::ResolvedToken;

use super::geometry::{CoverageShape, RectPx, Rotation};
use super::label::LabelInk;

/// Below this APCA magnitude the text is effectively painted into its backdrop,
/// which is a stronger signal than ordinary sub-threshold contrast.
pub(super) const INVISIBLE_LC_FLOOR: f64 = 15.0;
pub(super) const MIN_PAINT_ALPHA: f64 = 1.0 / 255.0;
/// The engine default text and label colour.
pub(super) const BLACK: (u8, u8, u8) = (0, 0, 0);

#[derive(Clone, Copy)]
pub(super) struct PaintCtx<'a> {
    pub(super) dx: f64,
    pub(super) dy: f64,
    pub(super) clip: Option<RectPx>,
    pub(super) opacity: f64,
    /// True when an ancestor `group`/`frame` carries a transform the validator
    /// cannot model geometrically (rotation) or a paint-altering effect
    /// (mask/filter/blur/non-normal blend). Every candidate under it is forced
    /// to an [`BackdropPaint::Indeterminate`] paint.
    pub(super) unmodeled: bool,
    pub(super) page_bg_rgb: Option<(u8, u8, u8)>,
    pub(super) page_size: (f64, f64),
    /// The style a table header cell paints on each of its direct `text`
    /// children that sets no `style` of its own. `None` everywhere else, and
    /// below the first container.
    pub(super) header_style: Option<&'a str>,
}

pub(super) struct BackdropCandidate {
    pub(super) paint: BackdropPaint,
    pub(super) bounds: RectPx,
    pub(super) shape: CoverageShape,
    /// Rigid rotation applied to this candidate about its box center, if any.
    pub(super) rotation: Option<Rotation>,
}

impl BackdropCandidate {
    pub(super) fn covers(&self, x: f64, y: f64) -> bool {
        let (x, y) = match self.rotation {
            Some(rot) => rot.inverse_map(x, y),
            None => (x, y),
        };
        self.shape.contains_point(self.bounds, x, y)
    }
}

#[derive(Debug)]
pub(super) enum BackdropPaint {
    Solid(PaintColor),
    Gradient(Vec<PaintColor>),
    Indeterminate,
}

impl BackdropPaint {
    pub(super) fn as_solid_rgb(&self) -> Option<(u8, u8, u8)> {
        match self {
            BackdropPaint::Solid(color) if color.alpha >= 1.0 => Some(color.rgb),
            BackdropPaint::Solid(_) => None,
            BackdropPaint::Gradient(_) | BackdropPaint::Indeterminate => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PaintColor {
    pub(super) rgb: (u8, u8, u8),
    pub(super) alpha: f64,
}

#[derive(Clone, Copy)]
pub(super) struct SampledBackdrop {
    pub(super) rgb: (u8, u8, u8),
    pub(super) source: &'static str,
}

#[derive(Clone, Copy)]
pub(super) struct ContrastSample {
    pub(super) lc: f64,
    pub(super) source: &'static str,
}

#[derive(Clone, Copy)]
pub(super) struct ContrastEnv<'a> {
    pub(super) resolved_tokens: &'a BTreeMap<String, ResolvedToken>,
    pub(super) style_map: &'a BTreeMap<&'a str, &'a Style>,
    /// `None`: judge text nodes. `Some`: judge only the labels with measured
    /// ink (the compile-stage label pass).
    pub(super) labels: Option<&'a BTreeMap<String, LabelInk>>,
}
