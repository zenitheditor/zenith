//! Scalar literal, alias-chain, cycle, reference, type-mismatch, duplicate-id,
//! and invalid-value token-resolution integration tests.
//!
//! Exercises the public token-resolution API (`zenith_core::resolve_tokens`)
//! against built `TokenBlock`s, checking resolved values and diagnostics.

use zenith_core::{
    Diagnostic, GradientKind, GradientLiteral, GradientStopRef, ResolvedValue, Severity, Token,
    TokenBlock, TokenLiteral, TokenType, TokenValue, resolve_tokens,
};
use zenith_core::{Dimension, Unit};

// ── Builder helpers ───────────────────────────────────────────────────

fn literal_token(id: &str, token_type: TokenType, literal: TokenLiteral) -> Token {
    Token {
        id: id.to_owned(),
        token_type,
        value: TokenValue::Literal(literal),
        set: None,
        source_span: None,
    }
}

fn alias_token(id: &str, token_type: TokenType, target: &str) -> Token {
    Token {
        id: id.to_owned(),
        token_type,
        value: TokenValue::Reference {
            token_id: target.to_owned(),
        },
        set: None,
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
        set: None,
        source_span: None,
    }
}

// ── Literal resolution tests ──────────────────────────────────────────

#[test]
fn resolves_color_literal() {
    let b = block(vec![literal_token(
        "color.text.primary",
        TokenType::Color,
        TokenLiteral::String("#111827".to_owned()),
    )]);
    let r = resolve_tokens(&b);
    assert!(
        r.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        r.diagnostics
    );
    assert_eq!(
        r.resolved["color.text.primary"].value,
        ResolvedValue::Color("#111827".to_owned())
    );
}

#[test]
fn resolves_color_with_alpha() {
    let b = block(vec![literal_token(
        "color.bg",
        TokenType::Color,
        TokenLiteral::String("#ffffff80".to_owned()),
    )]);
    let r = resolve_tokens(&b);
    assert!(r.diagnostics.is_empty());
    assert!(matches!(
        r.resolved["color.bg"].value,
        ResolvedValue::Color(_)
    ));
}

#[test]
fn resolves_dimension_literal() {
    let b = block(vec![literal_token(
        "size.text.title",
        TokenType::Dimension,
        TokenLiteral::Dimension(Dimension {
            value: 48.0,
            unit: Unit::Pt,
        }),
    )]);
    let r = resolve_tokens(&b);
    assert!(
        r.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        r.diagnostics
    );
    assert_eq!(
        r.resolved["size.text.title"].value,
        ResolvedValue::Dimension(Dimension {
            value: 48.0,
            unit: Unit::Pt
        })
    );
}

#[test]
fn resolves_number_literal() {
    let b = block(vec![literal_token(
        "lineheight.title",
        TokenType::Number,
        TokenLiteral::Number(1.05),
    )]);
    let r = resolve_tokens(&b);
    assert!(
        r.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        r.diagnostics
    );
    assert_eq!(
        r.resolved["lineheight.title"].value,
        ResolvedValue::Number(1.05)
    );
}

#[test]
fn resolves_font_family_literal() {
    let b = block(vec![literal_token(
        "font.family.body",
        TokenType::FontFamily,
        TokenLiteral::String("Inter".to_owned()),
    )]);
    let r = resolve_tokens(&b);
    assert!(
        r.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        r.diagnostics
    );
    assert_eq!(
        r.resolved["font.family.body"].value,
        ResolvedValue::FontFamily("Inter".to_owned())
    );
}

#[test]
fn resolves_font_weight_literal() {
    let b = block(vec![literal_token(
        "font.weight.bold",
        TokenType::FontWeight,
        TokenLiteral::Number(700.0),
    )]);
    let r = resolve_tokens(&b);
    assert!(
        r.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        r.diagnostics
    );
    assert_eq!(
        r.resolved["font.weight.bold"].value,
        ResolvedValue::FontWeight(700)
    );
}

// ── Alias chain resolution ────────────────────────────────────────────

#[test]
fn alias_chain_resolves_to_literal() {
    // a → b → "#aabbcc" literal
    let b = block(vec![
        alias_token("color.a", TokenType::Color, "color.b"),
        alias_token("color.b", TokenType::Color, "color.c"),
        literal_token(
            "color.c",
            TokenType::Color,
            TokenLiteral::String("#aabbcc".to_owned()),
        ),
    ]);
    let r = resolve_tokens(&b);
    assert!(
        r.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        r.diagnostics
    );
    // All three should be present in the resolved map.
    assert!(r.resolved.contains_key("color.a"), "color.a missing");
    assert!(r.resolved.contains_key("color.b"), "color.b missing");
    assert!(r.resolved.contains_key("color.c"), "color.c missing");
    assert_eq!(
        r.resolved["color.a"].value,
        ResolvedValue::Color("#aabbcc".to_owned())
    );
}

// ── Cycle detection ───────────────────────────────────────────────────

#[test]
fn self_cycle_produces_diagnostic_and_terminates() {
    let b = block(vec![alias_token(
        "color.self",
        TokenType::Color,
        "color.self",
    )]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "token.cyclic_reference"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
    assert!(!r.resolved.contains_key("color.self"));
}

#[test]
fn two_cycle_produces_diagnostic_and_terminates() {
    // a → b → a
    let b = block(vec![
        alias_token("color.a", TokenType::Color, "color.b"),
        alias_token("color.b", TokenType::Color, "color.a"),
    ]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "token.cyclic_reference"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
    // Neither should be resolved.
    assert!(!r.resolved.contains_key("color.a"));
    assert!(!r.resolved.contains_key("color.b"));
}

// ── Unknown reference ─────────────────────────────────────────────────

#[test]
fn unknown_reference_produces_diagnostic() {
    let b = block(vec![alias_token(
        "color.missing-target",
        TokenType::Color,
        "color.does.not.exist",
    )]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "token.unknown_reference"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
    assert!(!r.resolved.contains_key("color.missing-target"));
}

// ── Type mismatch ─────────────────────────────────────────────────────

#[test]
fn cross_type_alias_produces_type_mismatch() {
    // color.bad → size.text.title (dimension) — type mismatch
    let b = block(vec![
        alias_token("color.bad", TokenType::Color, "size.text.title"),
        literal_token(
            "size.text.title",
            TokenType::Dimension,
            TokenLiteral::Dimension(Dimension {
                value: 48.0,
                unit: Unit::Pt,
            }),
        ),
    ]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "token.type_mismatch"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
    assert!(!r.resolved.contains_key("color.bad"));
    // size.text.title itself should resolve fine.
    assert!(r.resolved.contains_key("size.text.title"));
}

// ── Duplicate ID ──────────────────────────────────────────────────────

#[test]
fn duplicate_id_produces_diagnostic_and_first_wins() {
    let b = block(vec![
        literal_token(
            "color.dup",
            TokenType::Color,
            TokenLiteral::String("#111111".to_owned()),
        ),
        literal_token(
            "color.dup",
            TokenType::Color,
            TokenLiteral::String("#222222".to_owned()),
        ),
    ]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "token.duplicate_id"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
    // First definition (#111111) should win.
    assert_eq!(
        r.resolved["color.dup"].value,
        ResolvedValue::Color("#111111".to_owned())
    );
}

// ── Invalid value ─────────────────────────────────────────────────────

#[test]
fn invalid_color_hex_produces_diagnostic() {
    let b = block(vec![literal_token(
        "color.bad",
        TokenType::Color,
        TokenLiteral::String("#xyz".to_owned()),
    )]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "token.invalid_value"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
    assert!(!r.resolved.contains_key("color.bad"));
}

#[test]
fn resolves_cmyk_color_to_hex_and_carries_channels() {
    let b = block(vec![literal_token(
        "color.accent.violet",
        TokenType::Color,
        TokenLiteral::String("cmyk(59,85,0,7)".to_owned()),
    )]);
    let r = resolve_tokens(&b);
    assert!(
        r.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        r.diagnostics
    );
    match &r.resolved["color.accent.violet"].value {
        ResolvedValue::CmykColor { hex, c, m, y, k } => {
            assert_eq!(hex, "#6124ed");
            assert_eq!((*c, *m, *y, *k), (59.0, 85.0, 0.0, 7.0));
        }
        other => panic!("expected CmykColor, got {other:?}"),
    }
}

#[test]
fn cmyk_zero_resolves_to_white_hex() {
    let b = block(vec![literal_token(
        "color.white",
        TokenType::Color,
        TokenLiteral::String("cmyk(0,0,0,0)".to_owned()),
    )]);
    let r = resolve_tokens(&b);
    assert!(r.diagnostics.is_empty());
    assert_eq!(
        r.resolved["color.white"].value.as_color_hex(),
        Some("#ffffff")
    );
}

#[test]
fn malformed_cmyk_produces_invalid_value() {
    let b = block(vec![literal_token(
        "color.bad-cmyk",
        TokenType::Color,
        TokenLiteral::String("cmyk(59,85,0,200)".to_owned()),
    )]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "token.invalid_value"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
    assert!(!r.resolved.contains_key("color.bad-cmyk"));
}

#[test]
fn hex_color_still_resolves_to_color_variant_unchanged() {
    // Regression guard: an sRGB hex token must remain a plain `Color`,
    // carrying no CMYK, byte-for-byte as before.
    let b = block(vec![literal_token(
        "color.hex",
        TokenType::Color,
        TokenLiteral::String("#112233".to_owned()),
    )]);
    let r = resolve_tokens(&b);
    assert!(r.diagnostics.is_empty());
    assert_eq!(
        r.resolved["color.hex"].value,
        ResolvedValue::Color("#112233".to_owned())
    );
    assert_eq!(r.resolved["color.hex"].value.cmyk(), None);
}

#[test]
fn cmyk_color_works_as_gradient_stop() {
    let b = block(vec![
        literal_token(
            "color.top",
            TokenType::Color,
            TokenLiteral::String("cmyk(59,85,0,7)".to_owned()),
        ),
        literal_token(
            "color.bottom",
            TokenType::Color,
            TokenLiteral::String("#334455".to_owned()),
        ),
        gradient_token(
            "gradient.bg",
            90.0,
            vec![(0.0, "color.top"), (1.0, "color.bottom")],
        ),
    ]);
    let r = resolve_tokens(&b);
    assert!(
        r.diagnostics.is_empty(),
        "a CMYK color must be a valid gradient stop; got: {:?}",
        r.diagnostics
    );
}

#[test]
fn font_weight_out_of_range_produces_diagnostic() {
    let b = block(vec![literal_token(
        "font.weight.heavy",
        TokenType::FontWeight,
        TokenLiteral::Number(1000.0),
    )]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "token.invalid_value"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
    assert!(!r.resolved.contains_key("font.weight.heavy"));
}

#[test]
fn font_weight_fractional_produces_diagnostic() {
    let b = block(vec![literal_token(
        "font.weight.frac",
        TokenType::FontWeight,
        TokenLiteral::Number(450.5),
    )]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "token.invalid_value"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
}

#[test]
fn number_nan_produces_diagnostic() {
    let b = block(vec![literal_token(
        "lineheight.nan",
        TokenType::Number,
        TokenLiteral::Number(f64::NAN),
    )]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "token.invalid_value"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
}

#[test]
fn number_inf_produces_diagnostic() {
    let b = block(vec![literal_token(
        "lineheight.inf",
        TokenType::Number,
        TokenLiteral::Number(f64::INFINITY),
    )]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "token.invalid_value"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
}

#[test]
fn dimension_wrong_literal_type_produces_diagnostic() {
    // A color token given a Dimension literal should produce invalid_value.
    let b = block(vec![literal_token(
        "color.bad-shape",
        TokenType::Color,
        TokenLiteral::Dimension(Dimension {
            value: 10.0,
            unit: Unit::Px,
        }),
    )]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "token.invalid_value"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
}

// ── Unknown type ──────────────────────────────────────────────────────

#[test]
fn unknown_type_produces_warning_and_is_not_resolved() {
    let b = block(vec![literal_token(
        "gradient.hero",
        TokenType::Unknown("gradient".to_owned()),
        TokenLiteral::String("linear-gradient(...)".to_owned()),
    )]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "token.unknown_type"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
    let unknown_diag = r
        .diagnostics
        .iter()
        .find(|d| d.code == "token.unknown_type")
        .expect("should exist");
    assert_eq!(unknown_diag.severity, Severity::Warning);
    assert!(!r.resolved.contains_key("gradient.hero"));
}

// ── Negative dimension allowed ────────────────────────────────────────

#[test]
fn negative_dimension_is_allowed_at_token_layer() {
    let b = block(vec![literal_token(
        "size.offset",
        TokenType::Dimension,
        TokenLiteral::Dimension(Dimension {
            value: -4.0,
            unit: Unit::Px,
        }),
    )]);
    let r = resolve_tokens(&b);
    assert!(
        r.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        r.diagnostics
    );
    assert!(r.resolved.contains_key("size.offset"));
}

// ── Dimension unknown unit ────────────────────────────────────────────

#[test]
fn dimension_unknown_unit_produces_invalid_value() {
    let b = block(vec![literal_token(
        "size.bad-unit",
        TokenType::Dimension,
        TokenLiteral::Dimension(Dimension {
            value: 10.0,
            unit: Unit::Unknown("em".to_owned()),
        }),
    )]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "token.invalid_value"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
}

// ── Font family empty ─────────────────────────────────────────────────

#[test]
fn empty_font_family_produces_invalid_value() {
    let b = block(vec![literal_token(
        "font.family.empty",
        TokenType::FontFamily,
        TokenLiteral::String(String::new()),
    )]);
    let r = resolve_tokens(&b);
    assert!(
        has_code(&r.diagnostics, "token.invalid_value"),
        "codes: {:?}",
        codes(&r.diagnostics)
    );
}
