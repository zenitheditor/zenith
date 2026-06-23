//! Per-type literal validation: turn a concrete [`TokenLiteral`] into a
//! [`ResolvedValue`] for the declared [`TokenType`], or push a
//! `token.invalid_value` (and type-specific) diagnostic and return `None`.
//!
//! Gradient stop-color and shadow layer-color cross-checks are NOT done here —
//! they require the fully-resolved token map and run as a second pass in the
//! driver.

use crate::ast::token::{
    FilterKind, FilterLiteral, GradientLiteral, MaskLiteral, ShadowLiteral, TokenLiteral, TokenType,
};
use crate::ast::value::Unit;
use crate::diagnostics::Diagnostic;

use super::types::{
    ResolvedFilter, ResolvedFilterOp, ResolvedGradient, ResolvedMask, ResolvedShadow,
    ResolvedShadowLayer, ResolvedValue,
};

/// Validate `literal` against `token_type`. Returns the [`ResolvedValue`] on
/// success, or pushes `token.invalid_value` and returns `None` on failure.
pub(super) fn validate_literal(
    token_id: &str,
    token_type: &TokenType,
    literal: &TokenLiteral,
    span: Option<crate::ast::Span>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ResolvedValue> {
    match token_type {
        TokenType::Color => validate_color(token_id, literal, span, diagnostics),
        TokenType::Dimension => validate_dimension(token_id, literal, span, diagnostics),
        TokenType::Number => validate_number(token_id, literal, span, diagnostics),
        TokenType::FontFamily => validate_font_family(token_id, literal, span, diagnostics),
        TokenType::FontWeight => validate_font_weight(token_id, literal, span, diagnostics),
        TokenType::Gradient => validate_gradient(token_id, literal, span, diagnostics),
        TokenType::Shadow => validate_shadow(token_id, literal, span, diagnostics),
        TokenType::Filter => validate_filter(token_id, literal, span, diagnostics),
        TokenType::Mask => validate_mask(token_id, literal, span, diagnostics),
        TokenType::Unknown(_) => {
            // Already handled upstream; should not reach here.
            None
        }
    }
}

fn validate_color(
    token_id: &str,
    literal: &TokenLiteral,
    span: Option<crate::ast::Span>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ResolvedValue> {
    match literal {
        TokenLiteral::String(s) => {
            if is_valid_hex_color(s) {
                Some(ResolvedValue::Color(s.clone()))
            } else if s.starts_with("cmyk(") {
                match crate::color::parse_cmyk(s) {
                    Some(cmyk) => Some(ResolvedValue::CmykColor {
                        hex: crate::color::cmyk_to_hex(cmyk),
                        c: cmyk.c,
                        m: cmyk.m,
                        y: cmyk.y,
                        k: cmyk.k,
                    }),
                    None => {
                        diagnostics.push(invalid_value(
                            token_id,
                            &format!(
                                "color token '{}' has value '{}' which is not a valid \
                                 CMYK color; expected 'cmyk(c,m,y,k)' with each channel \
                                 a percentage in 0..=100",
                                token_id, s
                            ),
                            span,
                        ));
                        None
                    }
                }
            } else {
                diagnostics.push(invalid_value(
                    token_id,
                    &format!(
                        "color token '{}' has value '{}' which is not a valid \
                         color; expected sRGB hex '#rrggbb'/'#rrggbbaa' \
                         (lowercase hex digits) or 'cmyk(c,m,y,k)'",
                        token_id, s
                    ),
                    span,
                ));
                None
            }
        }
        other @ (TokenLiteral::Dimension(_)
        | TokenLiteral::Number(_)
        | TokenLiteral::Gradient(_)
        | TokenLiteral::Shadow(_)
        | TokenLiteral::Filter(_)
        | TokenLiteral::Mask(_)) => {
            diagnostics.push(invalid_value(
                token_id,
                &format!(
                    "color token '{}' must have a string literal value (e.g. \"#rrggbb\"), \
                     got {}",
                    token_id,
                    literal_kind_name(other),
                ),
                span,
            ));
            None
        }
    }
}

/// Returns `true` if `s` matches `#[0-9a-fA-F]{6}` or `#[0-9a-fA-F]{8}`.
fn is_valid_hex_color(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.first() != Some(&b'#') {
        return false;
    }
    let hex = &bytes[1..];
    if hex.len() != 6 && hex.len() != 8 {
        return false;
    }
    hex.iter().all(|b| b.is_ascii_hexdigit())
}

fn validate_dimension(
    token_id: &str,
    literal: &TokenLiteral,
    span: Option<crate::ast::Span>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ResolvedValue> {
    match literal {
        TokenLiteral::Dimension(dim) => {
            if matches!(dim.unit, Unit::Unknown(_)) {
                diagnostics.push(invalid_value(
                    token_id,
                    &format!(
                        "dimension token '{}' uses an unrecognized unit; \
                         allowed units are px, pt, pct, deg",
                        token_id
                    ),
                    span,
                ));
                None
            } else {
                // Negative values are allowed at the token layer (per spec:
                // "Negative dimensions are invalid unless the consuming
                // property explicitly allows negative values" — that check
                // belongs to the property/node validation layer, not here).
                Some(ResolvedValue::Dimension(dim.clone()))
            }
        }
        other @ (TokenLiteral::String(_)
        | TokenLiteral::Number(_)
        | TokenLiteral::Gradient(_)
        | TokenLiteral::Shadow(_)
        | TokenLiteral::Filter(_)
        | TokenLiteral::Mask(_)) => {
            diagnostics.push(invalid_value(
                token_id,
                &format!(
                    "dimension token '{}' must have a dimension literal value \
                     (e.g. (px)28), got {}",
                    token_id,
                    literal_kind_name(other),
                ),
                span,
            ));
            None
        }
    }
}

fn validate_number(
    token_id: &str,
    literal: &TokenLiteral,
    span: Option<crate::ast::Span>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ResolvedValue> {
    match literal {
        TokenLiteral::Number(n) => {
            if n.is_finite() {
                Some(ResolvedValue::Number(*n))
            } else {
                diagnostics.push(invalid_value(
                    token_id,
                    &format!(
                        "number token '{}' has non-finite value '{}'; \
                         NaN and ±inf are invalid",
                        token_id, n
                    ),
                    span,
                ));
                None
            }
        }
        other @ (TokenLiteral::String(_)
        | TokenLiteral::Dimension(_)
        | TokenLiteral::Gradient(_)
        | TokenLiteral::Shadow(_)
        | TokenLiteral::Filter(_)
        | TokenLiteral::Mask(_)) => {
            diagnostics.push(invalid_value(
                token_id,
                &format!(
                    "number token '{}' must have a numeric literal value, got {}",
                    token_id,
                    literal_kind_name(other),
                ),
                span,
            ));
            None
        }
    }
}

fn validate_font_family(
    token_id: &str,
    literal: &TokenLiteral,
    span: Option<crate::ast::Span>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ResolvedValue> {
    match literal {
        TokenLiteral::String(s) => {
            if s.is_empty() {
                diagnostics.push(invalid_value(
                    token_id,
                    &format!(
                        "fontFamily token '{}' must not be an empty string",
                        token_id
                    ),
                    span,
                ));
                None
            } else {
                Some(ResolvedValue::FontFamily(s.clone()))
            }
        }
        other @ (TokenLiteral::Dimension(_)
        | TokenLiteral::Number(_)
        | TokenLiteral::Gradient(_)
        | TokenLiteral::Shadow(_)
        | TokenLiteral::Filter(_)
        | TokenLiteral::Mask(_)) => {
            diagnostics.push(invalid_value(
                token_id,
                &format!(
                    "fontFamily token '{}' must have a string literal value, got {}",
                    token_id,
                    literal_kind_name(other),
                ),
                span,
            ));
            None
        }
    }
}

fn validate_font_weight(
    token_id: &str,
    literal: &TokenLiteral,
    span: Option<crate::ast::Span>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ResolvedValue> {
    match literal {
        TokenLiteral::Number(n) => {
            // Must be an integer in [100, 900] in multiples of 1 (the contract
            // says "integer weight, initially 100 through 900").
            let truncated = n.trunc();
            // Check integral (no fractional part) and in-range.
            if (n - truncated).abs() > f64::EPSILON || !(100.0..=900.0).contains(&truncated) {
                diagnostics.push(invalid_value(
                    token_id,
                    &format!(
                        "fontWeight token '{}' has value '{}'; expected an \
                         integer in 100..=900",
                        token_id, n
                    ),
                    span,
                ));
                None
            } else {
                Some(ResolvedValue::FontWeight(truncated as u32))
            }
        }
        other @ (TokenLiteral::String(_)
        | TokenLiteral::Dimension(_)
        | TokenLiteral::Gradient(_)
        | TokenLiteral::Shadow(_)
        | TokenLiteral::Filter(_)
        | TokenLiteral::Mask(_)) => {
            diagnostics.push(invalid_value(
                token_id,
                &format!(
                    "fontWeight token '{}' must have a numeric literal value \
                     (e.g. 700), got {}",
                    token_id,
                    literal_kind_name(other),
                ),
                span,
            ));
            None
        }
    }
}

/// Validate a gradient literal: require ≥2 stops and finite offsets. Offsets are
/// clamped into `0.0..=1.0`. Stop-color existence/type are NOT checked here —
/// that requires the full resolved map and runs as a second pass.
fn validate_gradient(
    token_id: &str,
    literal: &TokenLiteral,
    span: Option<crate::ast::Span>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ResolvedValue> {
    let TokenLiteral::Gradient(GradientLiteral {
        kind,
        angle_deg,
        center_x,
        center_y,
        radius,
        stops,
    }) = literal
    else {
        diagnostics.push(invalid_value(
            token_id,
            &format!(
                "gradient token '{}' must be defined by `stop` child nodes, got {}",
                token_id,
                literal_kind_name(literal),
            ),
            span,
        ));
        return None;
    };

    if stops.len() < 2 {
        diagnostics.push(Diagnostic::error(
            "gradient.too_few_stops",
            format!(
                "gradient token '{}' has {} stop(s); at least 2 are required",
                token_id,
                stops.len()
            ),
            span,
            Some(token_id.to_owned()),
        ));
        return None;
    }

    // Validate radial-specific params.
    if let Some(r) = radius
        && (!r.is_finite() || *r <= 0.0)
    {
        diagnostics.push(Diagnostic::error(
            "gradient.invalid_radius",
            format!(
                "gradient token '{}' has an invalid radius {}; \
                 radius must be a finite positive number",
                token_id, r,
            ),
            span,
            Some(token_id.to_owned()),
        ));
        return None;
    }

    let mut resolved_stops: Vec<(f64, String)> = Vec::with_capacity(stops.len());
    for stop in stops {
        if !stop.offset.is_finite() {
            diagnostics.push(invalid_value(
                token_id,
                &format!(
                    "gradient token '{}' has a non-finite stop offset; \
                     NaN and ±inf are invalid",
                    token_id
                ),
                span,
            ));
            return None;
        }
        let clamped = stop.offset.clamp(0.0, 1.0);
        resolved_stops.push((clamped, stop.color_token.clone()));
    }

    Some(ResolvedValue::Gradient(ResolvedGradient {
        kind: *kind,
        angle_deg: *angle_deg,
        center_x: *center_x,
        center_y: *center_y,
        radius: *radius,
        stops: resolved_stops,
    }))
}

/// Validate a shadow literal: require ≥1 layer, each dx/dy/blur finite, with
/// blur clamped to `>= 0`. Layer-color existence/type are NOT checked here —
/// that requires the full resolved map and runs as a second pass.
fn validate_shadow(
    token_id: &str,
    literal: &TokenLiteral,
    span: Option<crate::ast::Span>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ResolvedValue> {
    let TokenLiteral::Shadow(ShadowLiteral { layers }) = literal else {
        diagnostics.push(invalid_value(
            token_id,
            &format!(
                "shadow token '{}' must be defined by `layer` child nodes, got {}",
                token_id,
                literal_kind_name(literal),
            ),
            span,
        ));
        return None;
    };

    if layers.is_empty() {
        diagnostics.push(Diagnostic::error(
            "shadow.no_layers",
            format!(
                "shadow token '{}' has no layers; at least 1 is required",
                token_id
            ),
            span,
            Some(token_id.to_owned()),
        ));
        return None;
    }

    let mut resolved_layers: Vec<ResolvedShadowLayer> = Vec::with_capacity(layers.len());
    for layer in layers {
        if !layer.dx.is_finite() || !layer.dy.is_finite() || !layer.blur.is_finite() {
            diagnostics.push(invalid_value(
                token_id,
                &format!(
                    "shadow token '{}' has a non-finite layer dx/dy/blur; \
                     NaN and ±inf are invalid",
                    token_id
                ),
                span,
            ));
            return None;
        }
        resolved_layers.push(ResolvedShadowLayer {
            dx: layer.dx,
            dy: layer.dy,
            blur: layer.blur.max(0.0),
            color_token: layer.color_token.clone(),
        });
    }

    Some(ResolvedValue::Shadow(ResolvedShadow {
        layers: resolved_layers,
    }))
}

/// Validate a filter literal: require ≥1 op, each amount (when present) finite.
/// Duotone op color-token existence/type is checked at the scene-compile layer.
fn validate_filter(
    token_id: &str,
    literal: &TokenLiteral,
    span: Option<crate::ast::Span>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ResolvedValue> {
    let TokenLiteral::Filter(FilterLiteral { ops }) = literal else {
        diagnostics.push(invalid_value(
            token_id,
            &format!(
                "filter token '{}' must be defined by op child nodes, got {}",
                token_id,
                literal_kind_name(literal),
            ),
            span,
        ));
        return None;
    };

    if ops.is_empty() {
        diagnostics.push(Diagnostic::error(
            "filter.no_ops",
            format!(
                "filter token '{}' has no ops; at least 1 is required",
                token_id
            ),
            span,
            Some(token_id.to_owned()),
        ));
        return None;
    }

    let mut resolved_ops: Vec<ResolvedFilterOp> = Vec::with_capacity(ops.len());
    for op in ops {
        if let Some(amount) = op.amount
            && !amount.is_finite()
        {
            diagnostics.push(Diagnostic::error(
                "filter.invalid_amount",
                format!(
                    "filter token '{}' has a non-finite op amount; \
                     NaN and ±inf are invalid",
                    token_id
                ),
                span,
                Some(token_id.to_owned()),
            ));
            return None;
        }
        // A noise op's grain cell size must be a positive, finite pixel count.
        if let Some(s) = op.scale
            && (!s.is_finite() || s <= 0.0)
        {
            diagnostics.push(Diagnostic::error(
                "filter.invalid_scale",
                format!(
                    "filter token '{}' has a non-positive or non-finite noise scale; \
                     scale must be > 0",
                    token_id
                ),
                span,
                Some(token_id.to_owned()),
            ));
            return None;
        }
        // A duotone op blends between two color tokens; both are required.
        // Non-duotone ops ignore any stray shadow/highlight props.
        if op.kind == FilterKind::Duotone {
            let missing = match (op.shadow.is_some(), op.highlight.is_some()) {
                (true, true) => None,
                (false, true) => Some("shadow"),
                (true, false) => Some("highlight"),
                (false, false) => Some("shadow and highlight"),
            };
            if let Some(which) = missing {
                diagnostics.push(Diagnostic::error(
                    "filter.duotone_missing_color",
                    format!(
                        "filter token '{}' has a duotone op missing {}; \
                         a duotone op requires both shadow and highlight color tokens",
                        token_id, which
                    ),
                    span,
                    Some(token_id.to_owned()),
                ));
                return None;
            }
        }
        resolved_ops.push(ResolvedFilterOp {
            kind: op.kind,
            amount: op.amount,
            shadow: op.shadow.clone(),
            highlight: op.highlight.clone(),
            seed: op.seed,
            scale: op.scale,
        });
    }

    Some(ResolvedValue::Filter(ResolvedFilter { ops: resolved_ops }))
}

/// Validate a mask literal: feather must be finite and `>= 0`; radius (when
/// present) must be finite and `>= 0`. Masks carry no token references, so there
/// is no transitive cross-check pass.
fn validate_mask(
    token_id: &str,
    literal: &TokenLiteral,
    span: Option<crate::ast::Span>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ResolvedValue> {
    let TokenLiteral::Mask(MaskLiteral {
        shape,
        radius,
        feather,
        invert,
    }) = literal
    else {
        diagnostics.push(invalid_value(
            token_id,
            &format!(
                "mask token '{}' must be defined by a shape child node, got {}",
                token_id,
                literal_kind_name(literal),
            ),
            span,
        ));
        return None;
    };

    if !feather.is_finite() || *feather < 0.0 {
        diagnostics.push(Diagnostic::error(
            "mask.invalid_feather",
            format!(
                "mask token '{}' has an invalid feather {}; \
                 feather must be a finite number >= 0",
                token_id, feather,
            ),
            span,
            Some(token_id.to_owned()),
        ));
        return None;
    }

    if let Some(r) = radius
        && (!r.is_finite() || *r < 0.0)
    {
        diagnostics.push(Diagnostic::error(
            "mask.invalid_radius",
            format!(
                "mask token '{}' has an invalid radius {}; \
                 radius must be a finite number >= 0",
                token_id, r,
            ),
            span,
            Some(token_id.to_owned()),
        ));
        return None;
    }

    Some(ResolvedValue::Mask(ResolvedMask {
        shape: *shape,
        radius: *radius,
        feather: *feather,
        invert: *invert,
    }))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn invalid_value(token_id: &str, message: &str, span: Option<crate::ast::Span>) -> Diagnostic {
    Diagnostic::error(
        "token.invalid_value",
        message,
        span,
        Some(token_id.to_owned()),
    )
}

fn literal_kind_name(lit: &TokenLiteral) -> &'static str {
    match lit {
        TokenLiteral::String(_) => "a string literal",
        TokenLiteral::Dimension(_) => "a dimension literal",
        TokenLiteral::Number(_) => "a number literal",
        TokenLiteral::Gradient(_) => "a gradient literal",
        TokenLiteral::Shadow(_) => "a shadow literal",
        TokenLiteral::Filter(_) => "a filter literal",
        TokenLiteral::Mask(_) => "a mask literal",
    }
}

/// The human-readable type name used in diagnostics. Shared with the driver's
/// type-mismatch and cross-check passes.
pub(super) fn type_name_of(t: &TokenType) -> &str {
    match t {
        TokenType::Color => "color",
        TokenType::Dimension => "dimension",
        TokenType::Number => "number",
        TokenType::FontFamily => "fontFamily",
        TokenType::FontWeight => "fontWeight",
        TokenType::Gradient => "gradient",
        TokenType::Shadow => "shadow",
        TokenType::Filter => "filter",
        TokenType::Mask => "mask",
        TokenType::Unknown(s) => s.as_str(),
    }
}
