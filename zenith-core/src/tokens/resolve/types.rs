//! Resolved token value types: the public output shapes produced by
//! [`resolve_tokens`](super::resolve_tokens) — colors, gradients, shadows,
//! filters, masks, dimensions, and font refs.

use std::collections::BTreeMap;

use crate::ast::token::{GradientKind, MaskShape, TokenType};
use crate::ast::value::Dimension;
use crate::diagnostics::Diagnostic;

// ── Public types ─────────────────────────────────────────────────────────────

/// The resolved, validated value of a single design token.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedValue {
    /// An sRGB-origin color, stored as canonical `#rrggbb`/`#rrggbbaa` hex.
    Color(String),
    /// A CMYK-origin color. `hex` is the naive device-conversion sRGB
    /// approximation (so every existing hex consumer works unchanged); `c`,
    /// `m`, `y`, `k` are the original percentages in `0.0..=100.0`, carried so a
    /// future PDF backend can emit native DeviceCMYK.
    CmykColor {
        hex: String,
        c: f32,
        m: f32,
        y: f32,
        k: f32,
    },
    Dimension(Dimension),
    Number(f64),
    FontFamily(String),
    FontWeight(u32),
    Gradient(ResolvedGradient),
    Shadow(ResolvedShadow),
    Filter(ResolvedFilter),
    Mask(ResolvedMask),
}

impl ResolvedValue {
    /// The sRGB hex string for any color-origin value (`Color` or `CmykColor`),
    /// or `None` for non-color values. Lets color consumers treat both color
    /// variants uniformly without duplicating match arms.
    pub fn as_color_hex(&self) -> Option<&str> {
        match self {
            ResolvedValue::Color(hex) => Some(hex.as_str()),
            ResolvedValue::CmykColor { hex, .. } => Some(hex.as_str()),
            ResolvedValue::Dimension(_)
            | ResolvedValue::Number(_)
            | ResolvedValue::FontFamily(_)
            | ResolvedValue::FontWeight(_)
            | ResolvedValue::Gradient(_)
            | ResolvedValue::Shadow(_)
            | ResolvedValue::Filter(_)
            | ResolvedValue::Mask(_) => None,
        }
    }

    /// The original CMYK channels `(c, m, y, k)` for a `CmykColor`, or `None`
    /// for sRGB-origin colors and non-color values.
    pub fn cmyk(&self) -> Option<(f32, f32, f32, f32)> {
        match self {
            ResolvedValue::CmykColor { c, m, y, k, .. } => Some((*c, *m, *y, *k)),
            ResolvedValue::Color(_)
            | ResolvedValue::Dimension(_)
            | ResolvedValue::Number(_)
            | ResolvedValue::FontFamily(_)
            | ResolvedValue::FontWeight(_)
            | ResolvedValue::Gradient(_)
            | ResolvedValue::Shadow(_)
            | ResolvedValue::Filter(_)
            | ResolvedValue::Mask(_) => None,
        }
    }
}

/// A resolved gradient: either linear (angle + stops) or radial
/// (center + radius + stops). Offsets are clamped into `0.0..=1.0`.
/// Stop-color existence and type are checked in a second pass over the
/// fully-resolved token map.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedGradient {
    /// Whether this is a linear or radial gradient.
    pub kind: GradientKind,
    /// Angle in degrees, clockwise from +x. Relevant only for `kind == Linear`.
    pub angle_deg: f64,
    /// Radial center X fraction of bounding-box width. `None` → 0.5.
    pub center_x: Option<f64>,
    /// Radial center Y fraction of bounding-box height. `None` → 0.5.
    pub center_y: Option<f64>,
    /// Radial radius fraction of box diagonal (`hypot(w,h)/2`). `None` → 1.0.
    pub radius: Option<f64>,
    /// Ordered `(offset, color_token_id)` stops.
    pub stops: Vec<(f64, String)>,
}

/// A resolved shadow: an ordered list of layers. Blur is clamped to `>= 0`.
/// Layer-color existence and type are checked in a second pass over the
/// fully-resolved token map (exactly like gradient stops).
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedShadow {
    /// Ordered list of resolved layers, in source order.
    pub layers: Vec<ResolvedShadowLayer>,
}

/// A single resolved shadow layer: offsets and blur (pixels) plus the id of the
/// color token this layer renders with.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedShadowLayer {
    pub dx: f64,
    pub dy: f64,
    pub blur: f64,
    pub color_token: String,
}

/// A resolved filter: an ordered list of filter ops, applied in source order.
/// Duotone ops carry shadow/highlight color token ids; their existence and type
/// are checked at the scene-compile layer (not here) to keep the resolver
/// single-pass, exactly like shadow/gradient color refs.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedFilter {
    /// Ordered list of resolved ops, in source order.
    pub ops: Vec<ResolvedFilterOp>,
}

/// A single resolved filter op: a kind plus an optional finite amount. A
/// `Duotone` op also carries its shadow/highlight color token ids (validated to
/// both be present); other kinds leave them `None`. Their existence/type is
/// checked at the scene-compile layer (the resolver records them as referenced
/// in the visual check, exactly like shadow/gradient color refs).
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedFilterOp {
    pub kind: crate::ast::token::FilterKind,
    pub amount: Option<f64>,
    pub shadow: Option<String>,
    pub highlight: Option<String>,
    /// Grain pattern seed — `Some` only for `Noise` ops; `None` defaults to 0.
    pub seed: Option<i64>,
    /// Grain cell size in pixels — `Some` only for `Noise` ops; `None` defaults
    /// to 1.0. Validated to be finite and `> 0` when present.
    pub scale: Option<f64>,
}

/// A resolved mask: a spatial coverage shape plus a feather and invert flag.
/// Masks carry no token references, so there is no transitive cross-check pass.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedMask {
    pub shape: MaskShape,
    pub radius: Option<f64>,
    pub feather: f64,
    pub invert: bool,
}

/// A successfully resolved token (type + value pair).
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedToken {
    pub token_type: TokenType,
    pub value: ResolvedValue,
}

/// The outcome of resolving a [`TokenBlock`](crate::ast::token::TokenBlock).
///
/// `resolved` contains only tokens that passed all validation checks.
/// `diagnostics` contains every problem found (may be non-empty even when
/// some tokens resolved successfully).
#[derive(Debug, Clone)]
pub struct TokenResolution {
    /// Successfully resolved tokens, keyed by token ID, sorted by ID.
    pub resolved: BTreeMap<String, ResolvedToken>,
    /// All diagnostics collected during resolution.
    pub diagnostics: Vec<Diagnostic>,
}
