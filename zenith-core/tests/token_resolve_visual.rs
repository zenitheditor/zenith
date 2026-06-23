//! Gradient, shadow, filter, and mask token-resolution integration tests.
//!
//! Exercises the public token-resolution API (`zenith_core::resolve_tokens`)
//! against built `TokenBlock`s carrying visual literals, checking resolved
//! values, clamping, and cross-check diagnostics.

use zenith_core::{
    Diagnostic, FilterKind, FilterLiteral, FilterOp, GradientKind, GradientLiteral,
    GradientStopRef, MaskLiteral, MaskShape, ResolvedMask, ResolvedValue, ShadowLayerRef,
    ShadowLiteral, Token, TokenBlock, TokenLiteral, TokenType, TokenValue, resolve_tokens,
};
use zenith_core::{Dimension, Unit};

// ── Builder helpers ───────────────────────────────────────────────────

fn literal_token(id: &str, token_type: TokenType, literal: TokenLiteral) -> Token {
    Token {
        id: id.to_owned(),
        token_type,
        value: TokenValue::Literal(literal),
        source_span: None,
    }
}

fn block(tokens: Vec<Token>) -> TokenBlock {
    TokenBlock {
        format: "zenith-token-v1".to_owned(),
        tokens,
    }
}

fn has_code(diagnostics: &[Diagnostic], code: &str) -> bool {
    diagnostics.iter().any(|d| d.code == code)
}

fn codes(diagnostics: &[Diagnostic]) -> Vec<&str> {
    diagnostics.iter().map(|d| d.code.as_str()).collect()
}

fn gradient_token(id: &str, angle_deg: f64, stops: Vec<(f64, &str)>) -> Token {
    Token {
        id: id.to_owned(),
        token_type: TokenType::Gradient,
        value: TokenValue::Literal(TokenLiteral::Gradient(GradientLiteral {
            kind: GradientKind::Linear,
            angle_deg,
            center_x: None,
            center_y: None,
            radius: None,
            stops: stops
                .into_iter()
                .map(|(offset, color)| GradientStopRef {
                    offset,
                    color_token: color.to_owned(),
                })
                .collect(),
        })),
        source_span: None,
    }
}

fn radial_gradient_token(
    id: &str,
    center_x: Option<f64>,
    center_y: Option<f64>,
    radius: Option<f64>,
    stops: Vec<(f64, &str)>,
) -> Token {
    Token {
        id: id.to_owned(),
        token_type: TokenType::Gradient,
        value: TokenValue::Literal(TokenLiteral::Gradient(GradientLiteral {
            kind: GradientKind::Radial,
            angle_deg: 90.0,
            center_x,
            center_y,
            radius,
            stops: stops
                .into_iter()
                .map(|(offset, color)| GradientStopRef {
                    offset,
                    color_token: color.to_owned(),
                })
                .collect(),
        })),
        source_span: None,
    }
}

fn shadow_token(id: &str, layers: Vec<(f64, f64, f64, &str)>) -> Token {
    Token {
        id: id.to_owned(),
        token_type: TokenType::Shadow,
        value: TokenValue::Literal(TokenLiteral::Shadow(ShadowLiteral {
            layers: layers
                .into_iter()
                .map(|(dx, dy, blur, color)| ShadowLayerRef {
                    dx,
                    dy,
                    blur,
                    color_token: color.to_owned(),
                })
                .collect(),
        })),
        source_span: None,
    }
}

fn filter_token(id: &str, ops: Vec<(FilterKind, Option<f64>)>) -> Token {
    Token {
        id: id.to_owned(),
        token_type: TokenType::Filter,
        value: TokenValue::Literal(TokenLiteral::Filter(FilterLiteral {
            ops: ops
                .into_iter()
                .map(|(kind, amount)| FilterOp {
                    kind,
                    amount,
                    shadow: None,
                    highlight: None,
                    seed: None,
                    scale: None,
                })
                .collect(),
        })),
        source_span: None,
    }
}

/// Build a filter token with a single `duotone` op carrying the given
/// shadow/highlight color token ids (either may be `None` to exercise the
/// missing-color diagnostic).
fn duotone_filter_token(
    id: &str,
    shadow: Option<&str>,
    highlight: Option<&str>,
    amount: Option<f64>,
) -> Token {
    Token {
        id: id.to_owned(),
        token_type: TokenType::Filter,
        value: TokenValue::Literal(TokenLiteral::Filter(FilterLiteral {
            ops: vec![FilterOp {
                kind: FilterKind::Duotone,
                amount,
                shadow: shadow.map(str::to_owned),
                highlight: highlight.map(str::to_owned),
                seed: None,
                scale: None,
            }],
        })),
        source_span: None,
    }
}

/// Build a filter token with a single `noise` op carrying the given seed/scale/
/// amount (any may be `None` to exercise defaults or invalid-scale diagnostics).
fn noise_filter_token(
    id: &str,
    seed: Option<i64>,
    scale: Option<f64>,
    amount: Option<f64>,
) -> Token {
    Token {
        id: id.to_owned(),
        token_type: TokenType::Filter,
        value: TokenValue::Literal(TokenLiteral::Filter(FilterLiteral {
            ops: vec![FilterOp {
                kind: FilterKind::Noise,
                amount,
                shadow: None,
                highlight: None,
                seed,
                scale,
            }],
        })),
        source_span: None,
    }
}

fn mask_token(
    id: &str,
    shape: MaskShape,
    radius: Option<f64>,
    feather: f64,
    invert: bool,
) -> Token {
    Token {
        id: id.to_owned(),
        token_type: TokenType::Mask,
        value: TokenValue::Literal(TokenLiteral::Mask(MaskLiteral {
            shape,
            radius,
            feather,
            invert,
        })),
        source_span: None,
    }
}

// ── Gradient (linear + radial) resolution ─────────────────────────────

#[test]
fn resolves_gradient_with_clamped_offsets() {
    let b = block(vec![
        literal_token(
            "color.top",
            TokenType::Color,
            TokenLiteral::String("#001122".to_owned()),
        ),
        literal_token(
            "color.bottom",
            TokenType::Color,
            TokenLiteral::String("#334455".to_owned()),
        ),
        // Offsets out of range get clamped into 0.0..=1.0.
        gradient_token(
            "gradient.bg.hero",
            90.0,
            vec![(-0.5, "color.top"), (1.5, "color.bottom")],
        ),
    ]);
    let r = resolve_tokens(&b);
    assert!(
        r.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        r.diagnostics
    );
    match &r.resolved["gradient.bg.hero"].value {
        ResolvedValue::Gradient(g) => {
            assert_eq!(g.angle_deg, 90.0);
            assert_eq!(
                g.stops,
                vec![
                    (0.0, "color.top".to_owned()),
                    (1.0, "color.bottom".to_owned()),
                ]
            );
        }
        other => panic!("expected gradient, got {other:?}"),
    }
}

#[test]
fn gradient_with_one_stop_produces_too_few_stops() {
    let b = block(vec![
        literal_token(
            "color.top",
            TokenType::Color,
            TokenLiteral::String("#001122".to_owned()),
        ),
        gradient_token("gradient.bad", 90.0, vec![(0.0, "color.top")]),
    ]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "gradient.too_few_stops"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
    assert!(!r.resolved.contains_key("gradient.bad"));
}

#[test]
fn gradient_stop_missing_token_produces_stop_unresolved() {
    let b = block(vec![
        literal_token(
            "color.top",
            TokenType::Color,
            TokenLiteral::String("#001122".to_owned()),
        ),
        gradient_token(
            "gradient.bg",
            90.0,
            vec![(0.0, "color.top"), (1.0, "color.does.not.exist")],
        ),
    ]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "gradient.stop_unresolved"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
}

#[test]
fn gradient_stop_wrong_type_produces_stop_wrong_type() {
    let b = block(vec![
        literal_token(
            "color.top",
            TokenType::Color,
            TokenLiteral::String("#001122".to_owned()),
        ),
        literal_token(
            "size.not-a-color",
            TokenType::Dimension,
            TokenLiteral::Dimension(Dimension {
                value: 4.0,
                unit: Unit::Px,
            }),
        ),
        gradient_token(
            "gradient.bg",
            90.0,
            vec![(0.0, "color.top"), (1.0, "size.not-a-color")],
        ),
    ]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "gradient.stop_wrong_type"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
}

#[test]
fn resolves_radial_gradient_with_params() {
    let b = block(vec![
        literal_token(
            "color.inner",
            TokenType::Color,
            TokenLiteral::String("#ffffff".to_owned()),
        ),
        literal_token(
            "color.outer",
            TokenType::Color,
            TokenLiteral::String("#000000".to_owned()),
        ),
        radial_gradient_token(
            "gradient.radial.hero",
            Some(0.5),
            Some(0.5),
            Some(0.8),
            vec![(0.0, "color.inner"), (1.0, "color.outer")],
        ),
    ]);
    let r = resolve_tokens(&b);
    assert!(
        r.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        r.diagnostics
    );
    match &r.resolved["gradient.radial.hero"].value {
        ResolvedValue::Gradient(g) => {
            assert_eq!(g.kind, GradientKind::Radial);
            assert_eq!(g.center_x, Some(0.5));
            assert_eq!(g.center_y, Some(0.5));
            assert_eq!(g.radius, Some(0.8));
            assert_eq!(
                g.stops,
                vec![
                    (0.0, "color.inner".to_owned()),
                    (1.0, "color.outer".to_owned()),
                ]
            );
        }
        other => panic!("expected gradient, got {other:?}"),
    }
}

#[test]
fn radial_gradient_zero_radius_produces_invalid_radius() {
    let b = block(vec![
        literal_token(
            "color.a",
            TokenType::Color,
            TokenLiteral::String("#aabbcc".to_owned()),
        ),
        literal_token(
            "color.b",
            TokenType::Color,
            TokenLiteral::String("#112233".to_owned()),
        ),
        radial_gradient_token(
            "gradient.bad.radius",
            None,
            None,
            Some(0.0), // zero radius → invalid
            vec![(0.0, "color.a"), (1.0, "color.b")],
        ),
    ]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "gradient.invalid_radius"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
    assert!(!r.resolved.contains_key("gradient.bad.radius"));
}

// ── Shadow resolution and layer cross-check ───────────────────────────

#[test]
fn resolves_shadow_with_clamped_blur() {
    let b = block(vec![
        literal_token(
            "color.shadow.black",
            TokenType::Color,
            TokenLiteral::String("#000000".to_owned()),
        ),
        // Negative blur is clamped to 0; offsets pass through.
        shadow_token(
            "shadow.headline",
            vec![(8.0, 8.0, -4.0, "color.shadow.black")],
        ),
    ]);
    let r = resolve_tokens(&b);
    assert!(
        r.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        r.diagnostics
    );
    match &r.resolved["shadow.headline"].value {
        ResolvedValue::Shadow(s) => {
            assert_eq!(s.layers.len(), 1);
            let layer = &s.layers[0];
            assert_eq!(layer.dx, 8.0);
            assert_eq!(layer.dy, 8.0);
            assert_eq!(layer.blur, 0.0);
            assert_eq!(layer.color_token, "color.shadow.black");
        }
        other => panic!("expected shadow, got {other:?}"),
    }
}

#[test]
fn empty_shadow_produces_no_layers() {
    let b = block(vec![shadow_token("shadow.empty", vec![])]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "shadow.no_layers"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
    assert!(!r.resolved.contains_key("shadow.empty"));
}

#[test]
fn shadow_layer_missing_token_produces_layer_unresolved() {
    let b = block(vec![shadow_token(
        "shadow.bad",
        vec![(0.0, 0.0, 20.0, "color.does.not.exist")],
    )]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "shadow.layer_unresolved"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
}

#[test]
fn shadow_layer_wrong_type_produces_layer_wrong_type() {
    let b = block(vec![
        literal_token(
            "size.not-a-color",
            TokenType::Dimension,
            TokenLiteral::Dimension(Dimension {
                value: 4.0,
                unit: Unit::Px,
            }),
        ),
        shadow_token("shadow.bad", vec![(0.0, 0.0, 20.0, "size.not-a-color")]),
    ]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "shadow.layer_wrong_type"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
}

// ── Filter (incl. duotone) resolution ─────────────────────────────────

#[test]
fn resolves_filter_with_ops() {
    let b = block(vec![filter_token(
        "filter.photo",
        vec![
            (FilterKind::Grayscale, Some(0.5)),
            (FilterKind::HueRotate, None),
        ],
    )]);
    let r = resolve_tokens(&b);
    assert!(
        r.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        r.diagnostics
    );
    match &r.resolved["filter.photo"].value {
        ResolvedValue::Filter(f) => {
            assert_eq!(f.ops.len(), 2);
            assert_eq!(f.ops[0].kind, FilterKind::Grayscale);
            assert_eq!(f.ops[0].amount, Some(0.5));
            assert_eq!(f.ops[1].kind, FilterKind::HueRotate);
            assert_eq!(f.ops[1].amount, None);
        }
        other => panic!("expected filter, got {other:?}"),
    }
}

#[test]
fn empty_filter_produces_no_ops() {
    let b = block(vec![filter_token("filter.empty", vec![])]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "filter.no_ops"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
    assert!(!r.resolved.contains_key("filter.empty"));
}

#[test]
fn filter_non_finite_amount_produces_invalid_amount() {
    let b = block(vec![filter_token(
        "filter.bad",
        vec![(FilterKind::Saturate, Some(f64::NAN))],
    )]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "filter.invalid_amount"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
    assert!(!r.resolved.contains_key("filter.bad"));
}

#[test]
fn filter_wrong_literal_type_produces_invalid_value() {
    let b = block(vec![literal_token(
        "filter.bad-shape",
        TokenType::Filter,
        TokenLiteral::String("grayscale".to_owned()),
    )]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "token.invalid_value"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
    assert!(!r.resolved.contains_key("filter.bad-shape"));
}

#[test]
fn resolves_duotone_with_both_colors() {
    let b = block(vec![
        literal_token(
            "color.sh",
            TokenType::Color,
            TokenLiteral::String("#000000".to_owned()),
        ),
        literal_token(
            "color.hi",
            TokenType::Color,
            TokenLiteral::String("#ffffff".to_owned()),
        ),
        duotone_filter_token("filter.duo", Some("color.sh"), Some("color.hi"), Some(0.8)),
    ]);
    let r = resolve_tokens(&b);
    assert!(
        r.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        r.diagnostics
    );
    match &r.resolved["filter.duo"].value {
        ResolvedValue::Filter(f) => {
            assert_eq!(f.ops.len(), 1);
            assert_eq!(f.ops[0].kind, FilterKind::Duotone);
            assert_eq!(f.ops[0].amount, Some(0.8));
            assert_eq!(f.ops[0].shadow.as_deref(), Some("color.sh"));
            assert_eq!(f.ops[0].highlight.as_deref(), Some("color.hi"));
        }
        other => panic!("expected filter, got {other:?}"),
    }
}

#[test]
fn duotone_missing_highlight_produces_missing_color() {
    let b = block(vec![
        literal_token(
            "color.sh",
            TokenType::Color,
            TokenLiteral::String("#000000".to_owned()),
        ),
        duotone_filter_token("filter.duo", Some("color.sh"), None, None),
    ]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "filter.duotone_missing_color"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
    assert!(!r.resolved.contains_key("filter.duo"));
}

#[test]
fn resolves_noise_with_seed_and_scale() {
    let b = block(vec![noise_filter_token(
        "filter.grain",
        Some(7),
        Some(2.0),
        Some(0.3),
    )]);
    let r = resolve_tokens(&b);
    assert!(
        r.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        r.diagnostics
    );
    match &r.resolved["filter.grain"].value {
        ResolvedValue::Filter(f) => {
            assert_eq!(f.ops.len(), 1);
            assert_eq!(f.ops[0].kind, FilterKind::Noise);
            assert_eq!(f.ops[0].amount, Some(0.3));
            assert_eq!(f.ops[0].seed, Some(7));
            assert_eq!(f.ops[0].scale, Some(2.0));
        }
        other => panic!("expected filter, got {other:?}"),
    }
}

#[test]
fn noise_zero_scale_produces_invalid_scale() {
    let b = block(vec![noise_filter_token(
        "filter.grain",
        Some(0),
        Some(0.0),
        None,
    )]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "filter.invalid_scale"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
    assert!(!r.resolved.contains_key("filter.grain"));
}

#[test]
fn noise_negative_scale_produces_invalid_scale() {
    let b = block(vec![noise_filter_token(
        "filter.grain",
        Some(0),
        Some(-1.0),
        None,
    )]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "filter.invalid_scale"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
    assert!(!r.resolved.contains_key("filter.grain"));
}

// ── Mask resolution ───────────────────────────────────────────────────

#[test]
fn resolves_mask_literal() {
    let b = block(vec![mask_token(
        "mask.vignette",
        MaskShape::RoundedRect,
        Some(40.0),
        60.0,
        true,
    )]);
    let r = resolve_tokens(&b);
    assert!(
        r.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        r.diagnostics
    );
    assert_eq!(
        r.resolved["mask.vignette"].value,
        ResolvedValue::Mask(ResolvedMask {
            shape: MaskShape::RoundedRect,
            radius: Some(40.0),
            feather: 60.0,
            invert: true,
        })
    );
}

#[test]
fn mask_negative_feather_produces_invalid_feather() {
    let b = block(vec![mask_token(
        "mask.bad",
        MaskShape::Rect,
        None,
        -5.0,
        false,
    )]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "mask.invalid_feather"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
    assert!(!r.resolved.contains_key("mask.bad"));
}

#[test]
fn mask_wrong_literal_type_produces_invalid_value() {
    let b = block(vec![literal_token(
        "mask.bad-shape",
        TokenType::Mask,
        TokenLiteral::String("rounded".to_owned()),
    )]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "token.invalid_value"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
    assert!(!r.resolved.contains_key("mask.bad-shape"));
}
