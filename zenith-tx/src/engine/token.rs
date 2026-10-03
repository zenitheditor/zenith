//! Token op application: [`apply_create_token`] and [`apply_update_token_value`].

use zenith_core::{
    Diagnostic, Document, FilterKind, FilterLiteral, FilterOp, GradientKind, GradientLiteral,
    GradientStopRef, MaskLiteral, MaskShape, ShadowLayerRef, ShadowLiteral, Token, TokenLiteral,
    TokenType, TokenValue,
};

use crate::op::{FilterOpInput, GradientStopInput, ShadowLayerInput};

use super::record_affected;
use super::structure::parse_dimension_str;

/// Structured body inputs for [`apply_create_token`] when the type is not scalar.
pub(super) struct CreateTokenBody<'a> {
    pub layers: &'a [ShadowLayerInput],
    pub filter_ops: &'a [FilterOpInput],
    pub stops: &'a [GradientStopInput],
    pub angle: Option<f64>,
    pub radial: Option<bool>,
    pub center_x: Option<f64>,
    pub center_y: Option<f64>,
    pub radius: Option<f64>,
    pub shape: Option<&'a str>,
    pub feather: Option<f64>,
    pub invert: Option<bool>,
}

/// Scalar identity fields for [`apply_create_token`], bundled to stay under the
/// argument-count lint.
pub(super) struct CreateTokenScalars<'a> {
    pub id: &'a str,
    pub token_type: &'a str,
    pub value: &'a str,
    pub set: Option<&'a str>,
}

// ── Shared value-parsing helper ───────────────────────────────────────────────

/// Parse a literal value string against the given [`TokenType`], producing a
/// [`TokenLiteral`] on success or `None` on failure.
///
/// - `Color` / `FontFamily` → [`TokenLiteral::String`] (verbatim, including any
///   leading `#`).
/// - `Dimension` → [`TokenLiteral::Dimension`] via the canonical `"(unit)value"`
///   parser (e.g. `"(px)40"`).  Returns `None` if the string is not that form or
///   the number is not finite.
/// - `Number` / `FontWeight` → [`TokenLiteral::Number`] via `f64` parse; must be
///   finite.
/// - Structured types (`Gradient` / `Shadow` / `Filter` / `Mask` / `Unknown`)
///   → `None` here; those are built from dedicated op fields.
fn parse_token_literal(token_type: &TokenType, value: &str) -> Option<TokenLiteral> {
    match token_type {
        TokenType::Color | TokenType::FontFamily => Some(TokenLiteral::String(value.to_owned())),
        TokenType::Dimension => {
            let dim = parse_dimension_str(value)?;
            Some(TokenLiteral::Dimension(dim))
        }
        TokenType::Number | TokenType::FontWeight => {
            let n: f64 = value.trim().parse().ok()?;
            if n.is_finite() {
                Some(TokenLiteral::Number(n))
            } else {
                None
            }
        }
        TokenType::Gradient
        | TokenType::Shadow
        | TokenType::Filter
        | TokenType::Mask
        | TokenType::Unknown(_) => None,
    }
}

/// Return a human-readable name for a [`TokenType`] suitable for diagnostic
/// messages.
fn token_type_name(token_type: &TokenType) -> &str {
    match token_type {
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

fn build_shadow_literal(layers: &[ShadowLayerInput]) -> Option<TokenLiteral> {
    if layers.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(layers.len());
    for layer in layers {
        if layer.color.trim().is_empty() || !layer.blur.is_finite() {
            return None;
        }
        if !layer.dx.is_finite() || !layer.dy.is_finite() {
            return None;
        }
        out.push(ShadowLayerRef {
            dx: layer.dx,
            dy: layer.dy,
            blur: layer.blur,
            color_token: layer.color.clone(),
        });
    }
    Some(TokenLiteral::Shadow(ShadowLiteral { layers: out }))
}

fn build_filter_literal(ops: &[FilterOpInput]) -> Option<TokenLiteral> {
    if ops.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(ops.len());
    for op in ops {
        let kind = FilterKind::from_op_name(op.kind.as_str())?;
        if let Some(amount) = op.amount {
            if !amount.is_finite() {
                return None;
            }
        }
        if let Some(scale) = op.scale {
            if !scale.is_finite() || scale <= 0.0 {
                return None;
            }
        }
        // Duotone requires both color token refs at create time (not deferred
        // to post-validation).
        if kind == FilterKind::Duotone {
            let shadow_ok = op.shadow.as_ref().is_some_and(|s| !s.trim().is_empty());
            let highlight_ok = op.highlight.as_ref().is_some_and(|s| !s.trim().is_empty());
            if !shadow_ok || !highlight_ok {
                return None;
            }
        }
        out.push(FilterOp {
            kind,
            amount: op.amount,
            shadow: op.shadow.clone(),
            highlight: op.highlight.clone(),
            seed: op.seed,
            scale: op.scale,
        });
    }
    Some(TokenLiteral::Filter(FilterLiteral { ops: out }))
}

fn build_gradient_literal(body: &CreateTokenBody<'_>) -> Option<TokenLiteral> {
    if body.stops.len() < 2 {
        return None;
    }
    let mut stops = Vec::with_capacity(body.stops.len());
    for stop in body.stops {
        if !stop.offset.is_finite() || !(0.0..=1.0).contains(&stop.offset) {
            return None;
        }
        if stop.color.trim().is_empty() {
            return None;
        }
        stops.push(GradientStopRef {
            offset: stop.offset,
            color_token: stop.color.clone(),
        });
    }
    // Stops must be sorted by offset (engine / formatter contract).
    for window in stops.windows(2) {
        if window[0].offset > window[1].offset {
            return None;
        }
    }

    let is_radial = body.radial.unwrap_or(false);
    if is_radial {
        if let Some(cx) = body.center_x {
            if !cx.is_finite() {
                return None;
            }
        }
        if let Some(cy) = body.center_y {
            if !cy.is_finite() {
                return None;
            }
        }
        if let Some(r) = body.radius {
            if !r.is_finite() || r <= 0.0 {
                return None;
            }
        }
        Some(TokenLiteral::Gradient(GradientLiteral {
            kind: GradientKind::Radial,
            angle_deg: 0.0,
            center_x: body.center_x,
            center_y: body.center_y,
            radius: body.radius,
            stops,
        }))
    } else {
        let angle = body.angle.unwrap_or(0.0);
        if !angle.is_finite() {
            return None;
        }
        Some(TokenLiteral::Gradient(GradientLiteral {
            kind: GradientKind::Linear,
            angle_deg: angle,
            center_x: None,
            center_y: None,
            radius: None,
            stops,
        }))
    }
}

fn build_mask_literal(body: &CreateTokenBody<'_>) -> Option<TokenLiteral> {
    let shape_name = body.shape?;
    let shape = MaskShape::from_shape_name(shape_name)?;
    let feather = body.feather.unwrap_or(0.0);
    if !feather.is_finite() || feather < 0.0 {
        return None;
    }
    let radius = match shape {
        MaskShape::RoundedRect => {
            let r = body.radius?;
            if !r.is_finite() || r < 0.0 {
                return None;
            }
            Some(r)
        }
        MaskShape::Rect | MaskShape::Ellipse => None,
    };
    Some(TokenLiteral::Mask(MaskLiteral {
        shape,
        radius,
        feather,
        invert: body.invert.unwrap_or(false),
    }))
}

// ── CreateToken ───────────────────────────────────────────────────────────────

/// Create a new design token in `doc.tokens.tokens`.
///
/// Eagerly rejects with `tx.duplicate_id` if a token with `id` already exists.
/// Scalar types parse `value`. Structured types use the dedicated body fields.
pub(super) fn apply_create_token(
    scalars: &CreateTokenScalars<'_>,
    body: &CreateTokenBody<'_>,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    let CreateTokenScalars {
        id,
        token_type,
        value,
        set,
    } = *scalars;

    // Eager duplicate-id check.
    if doc.tokens.tokens.iter().any(|t| t.id == id) {
        diagnostics.push(Diagnostic::error(
            "tx.duplicate_id",
            format!("create_token: a token with id {:?} already exists", id),
            None,
            Some(id.to_owned()),
        ));
        return;
    }

    let ty = TokenType::from_type_name(token_type);

    let lit = match &ty {
        TokenType::Color
        | TokenType::Dimension
        | TokenType::Number
        | TokenType::FontFamily
        | TokenType::FontWeight => {
            if value.is_empty() {
                diagnostics.push(Diagnostic::error(
                    "tx.invalid_value",
                    format!(
                        "create_token: value is required for token type {:?}",
                        token_type_name(&ty)
                    ),
                    None,
                    Some(id.to_owned()),
                ));
                return;
            }
            match parse_token_literal(&ty, value) {
                Some(lit) => lit,
                None => {
                    diagnostics.push(Diagnostic::error(
                        "tx.invalid_value",
                        format!(
                            "create_token: value {:?} is not valid for token type {:?}",
                            value,
                            token_type_name(&ty)
                        ),
                        None,
                        Some(id.to_owned()),
                    ));
                    return;
                }
            }
        }
        TokenType::Shadow => match build_shadow_literal(body.layers) {
            Some(lit) => lit,
            None => {
                diagnostics.push(Diagnostic::error(
                    "tx.invalid_value",
                    "create_token: type \"shadow\" requires a non-empty layers array \
                     of {dx, dy, blur, color} (color is a color token id)"
                        .to_owned(),
                    None,
                    Some(id.to_owned()),
                ));
                return;
            }
        },
        TokenType::Filter => match build_filter_literal(body.filter_ops) {
            Some(lit) => lit,
            None => {
                diagnostics.push(Diagnostic::error(
                    "tx.invalid_value",
                    "create_token: type \"filter\" requires a non-empty filter_ops array \
                     of {kind, …} (kind is a filter op name, e.g. noise, grayscale); \
                     duotone requires non-empty shadow and highlight color token ids"
                        .to_owned(),
                    None,
                    Some(id.to_owned()),
                ));
                return;
            }
        },
        TokenType::Gradient => match build_gradient_literal(body) {
            Some(lit) => lit,
            None => {
                diagnostics.push(Diagnostic::error(
                    "tx.invalid_value",
                    "create_token: type \"gradient\" requires at least two stops \
                     [{offset, color}, …] with offsets in 0..=1 sorted ascending; \
                     optional angle (linear) or radial/center_x/center_y/radius"
                        .to_owned(),
                    None,
                    Some(id.to_owned()),
                ));
                return;
            }
        },
        TokenType::Mask => match build_mask_literal(body) {
            Some(lit) => lit,
            None => {
                diagnostics.push(Diagnostic::error(
                    "tx.invalid_value",
                    "create_token: type \"mask\" requires shape \
                     (\"rect\"|\"rounded\"|\"ellipse\"); rounded needs radius; \
                     optional feather (>=0) and invert"
                        .to_owned(),
                    None,
                    Some(id.to_owned()),
                ));
                return;
            }
        },
        TokenType::Unknown(_) => {
            diagnostics.push(Diagnostic::error(
                "tx.invalid_value",
                format!(
                    "create_token: token type {:?} is not supported via this op",
                    token_type_name(&ty)
                ),
                None,
                Some(id.to_owned()),
            ));
            return;
        }
    };

    doc.tokens.tokens.push(Token {
        id: id.to_owned(),
        token_type: ty,
        value: TokenValue::Literal(lit),
        set: set.map(str::to_owned),
        source_span: None,
    });

    record_affected(id, affected);
}

// ── UpdateTokenValue ──────────────────────────────────────────────────────────

/// Replace the literal value of an existing token, preserving its declared type.
///
/// Rejects with `tx.unknown_token` if no token with `id` exists.  Rejects with
/// `tx.invalid_value` if the token is a structured type (unsupported via this
/// op), or if `value` does not parse for the token's existing type.  On success
/// replaces `token.value` and records `id` in `affected`.
///
/// `set` is tri-state. `Some(Some(id))` re-stamps the token's `set`
/// provenance (e.g. a theme apply re-skinning the token to a new theme/pack).
/// `Some(None)` removes it. `None` leaves the existing `set` untouched.
pub(super) fn apply_update_token_value(
    id: &str,
    value: &str,
    set: Option<Option<&str>>,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    // Find the token index first (shared borrow), then mutate.
    let Some(idx) = doc.tokens.tokens.iter().position(|t| t.id == id) else {
        diagnostics.push(Diagnostic::error(
            "tx.unknown_token",
            format!("update_token_value: no token with id {:?} exists", id),
            None,
            Some(id.to_owned()),
        ));
        return;
    };

    // Clone the type so we can release the shared borrow before mutating.
    let Some(ty) = doc.tokens.tokens.get(idx).map(|t| t.token_type.clone()) else {
        return; // unreachable: idx is valid for this Vec
    };

    // Reject unsupported complex types for this scalar-value op.
    match &ty {
        TokenType::Gradient
        | TokenType::Shadow
        | TokenType::Filter
        | TokenType::Mask
        | TokenType::Unknown(_) => {
            diagnostics.push(Diagnostic::error(
                "tx.invalid_value",
                format!(
                    "update_token_value: token {:?} has type {:?} which cannot be \
                     updated via this op (recreate with create_token or edit source)",
                    id,
                    token_type_name(&ty)
                ),
                None,
                Some(id.to_owned()),
            ));
            return;
        }
        TokenType::Color
        | TokenType::Dimension
        | TokenType::Number
        | TokenType::FontFamily
        | TokenType::FontWeight => {}
    }

    let Some(lit) = parse_token_literal(&ty, value) else {
        diagnostics.push(Diagnostic::error(
            "tx.invalid_value",
            format!(
                "update_token_value: value {:?} is not valid for token type {:?}",
                value,
                token_type_name(&ty)
            ),
            None,
            Some(id.to_owned()),
        ));
        return;
    };

    if let Some(token) = doc.tokens.tokens.get_mut(idx) {
        token.value = TokenValue::Literal(lit);
        if let Some(set_id) = set {
            token.set = set_id.map(str::to_owned);
        }
    }

    record_affected(id, affected);
}
