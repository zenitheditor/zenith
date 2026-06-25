//! Brand-contract validation.
//!
//! Checks every resolved design token's value against the document's
//! [`BrandContract`] and emits `brand.*` Warning diagnostics for any off-contract
//! values. An empty contract (absent `brand { … }` block) is an identity pass.

use std::collections::BTreeMap;

use crate::ast::brand::BrandContract;
use crate::diagnostics::Diagnostic;
use crate::tokens::{ResolvedToken, ResolvedValue};

/// Check all resolved tokens against the brand contract.
///
/// Early-returns when the contract is empty (no categories constrained) to
/// preserve byte-identical output for documents without a `brand` block.
///
/// For each token:
/// - `ResolvedValue::Color` — hex compared case-insensitively against
///   `allowed_colors` (colors are stored lowercase at parse time, so we just
///   lowercase the resolved hex before comparing).
/// - `ResolvedValue::CmykColor` — its sRGB-approximation `hex` field is used
///   for the same case-insensitive comparison (same as `Color`).
/// - `ResolvedValue::FontFamily` — compared against `allowed_fonts` using a
///   case-sensitive equality check (font names are case-sensitive in CSS/OS
///   lookup tables; the palette author is expected to spell them correctly).
/// - `ResolvedValue::FontWeight` — the numeric weight is looked up in
///   `allowed_weights`.
/// - All other variants (`Dimension`, `Number`, `Gradient`, `Shadow`,
///   `Filter`, `Mask`) are not brand-governed and produce no diagnostic.
///
/// An absent category (`None`) means "unconstrained": no diagnostic is emitted
/// regardless of the token's value for that category.
pub(super) fn check_brand_contract(
    contract: &BrandContract,
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if contract.is_empty() {
        return;
    }

    for (token_id, resolved) in resolved_tokens {
        match &resolved.value {
            ResolvedValue::Color(hex) => {
                if let Some(allowed) = &contract.allowed_colors {
                    let hex_lower = hex.to_lowercase();
                    if !allowed.contains(&hex_lower) {
                        diagnostics.push(Diagnostic::warning(
                            "brand.color_off_palette",
                            format!(
                                "token '{token_id}': color '{hex}' is not in the brand palette \
                                 (allowed: {})",
                                format_list(allowed)
                            ),
                            None,
                            Some(token_id.clone()),
                        ));
                    }
                }
            }
            ResolvedValue::CmykColor { hex, .. } => {
                if let Some(allowed) = &contract.allowed_colors {
                    let hex_lower = hex.to_lowercase();
                    if !allowed.contains(&hex_lower) {
                        diagnostics.push(Diagnostic::warning(
                            "brand.color_off_palette",
                            format!(
                                "token '{token_id}': CMYK color (resolved hex '{hex}') is not \
                                 in the brand palette (allowed: {})",
                                format_list(allowed)
                            ),
                            None,
                            Some(token_id.clone()),
                        ));
                    }
                }
            }
            ResolvedValue::FontFamily(family) => {
                if let Some(allowed) = &contract.allowed_fonts {
                    if !allowed.contains(family) {
                        diagnostics.push(Diagnostic::warning(
                            "brand.font_not_allowed",
                            format!(
                                "token '{token_id}': font family '{family}' is not in the brand \
                                 font list (allowed: {})",
                                format_list(allowed)
                            ),
                            None,
                            Some(token_id.clone()),
                        ));
                    }
                }
            }
            ResolvedValue::FontWeight(weight) => {
                if let Some(allowed) = &contract.allowed_weights {
                    if !allowed.contains(weight) {
                        diagnostics.push(Diagnostic::warning(
                            "brand.weight_not_allowed",
                            format!(
                                "token '{token_id}': font weight {weight} is not in the brand \
                                 weight list (allowed: {})",
                                format_weight_list(allowed)
                            ),
                            None,
                            Some(token_id.clone()),
                        ));
                    }
                }
            }
            // Non-color / non-font values are not brand-governed.
            ResolvedValue::Dimension(_)
            | ResolvedValue::Number(_)
            | ResolvedValue::Gradient(_)
            | ResolvedValue::Shadow(_)
            | ResolvedValue::Filter(_)
            | ResolvedValue::Mask(_) => {}
        }
    }
}

/// Format a list of allowed string values for a diagnostic message.
fn format_list(values: &[String]) -> String {
    if values.is_empty() {
        return "(none approved)".to_owned();
    }
    values
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Format a list of allowed weight integers for a diagnostic message.
fn format_weight_list(values: &[u32]) -> String {
    if values.is_empty() {
        return "(none approved)".to_owned();
    }
    values
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::ast::brand::BrandContract;
    use crate::ast::token::TokenType;
    use crate::diagnostics::Severity;
    use crate::tokens::{ResolvedToken, ResolvedValue};

    use super::check_brand_contract;

    fn color_token(hex: &str) -> ResolvedToken {
        ResolvedToken {
            token_type: TokenType::Color,
            value: ResolvedValue::Color(hex.to_owned()),
        }
    }

    fn font_token(family: &str) -> ResolvedToken {
        ResolvedToken {
            token_type: TokenType::FontFamily,
            value: ResolvedValue::FontFamily(family.to_owned()),
        }
    }

    fn weight_token(w: u32) -> ResolvedToken {
        ResolvedToken {
            token_type: TokenType::FontWeight,
            value: ResolvedValue::FontWeight(w),
        }
    }

    fn run(contract: BrandContract, tokens: BTreeMap<String, ResolvedToken>) -> Vec<String> {
        let mut diags = Vec::new();
        check_brand_contract(&contract, &tokens, &mut diags);
        diags.into_iter().map(|d| d.code).collect()
    }

    fn has_code(codes: &[String], code: &str) -> bool {
        codes.iter().any(|c| c == code)
    }

    // ── Empty contract ────────────────────────────────────────────────────────

    #[test]
    fn empty_contract_is_noop() {
        let mut tokens = BTreeMap::new();
        tokens.insert("color.brand".to_owned(), color_token("#ff0000"));
        let codes = run(BrandContract::default(), tokens);
        assert!(
            codes.is_empty(),
            "empty contract must produce no diagnostics"
        );
    }

    // ── Color checks ──────────────────────────────────────────────────────────

    #[test]
    fn color_on_palette_no_diagnostic() {
        let mut tokens = BTreeMap::new();
        tokens.insert("color.brand".to_owned(), color_token("#0b1f33"));
        let contract = BrandContract {
            allowed_colors: Some(vec!["#0b1f33".to_owned()]),
            ..Default::default()
        };
        let codes = run(contract, tokens);
        assert!(
            codes.is_empty(),
            "on-palette color must not fire brand.color_off_palette; got: {codes:?}"
        );
    }

    #[test]
    fn color_off_palette_fires_diagnostic() {
        let mut tokens = BTreeMap::new();
        tokens.insert("color.bad".to_owned(), color_token("#ff0000"));
        let contract = BrandContract {
            allowed_colors: Some(vec!["#0b1f33".to_owned()]),
            ..Default::default()
        };
        let codes = run(contract, tokens);
        assert!(
            has_code(&codes, "brand.color_off_palette"),
            "off-palette color must fire brand.color_off_palette; got: {codes:?}"
        );
    }

    #[test]
    fn color_comparison_is_case_insensitive() {
        // Palette stores lowercase (from parser), token value may be uppercase.
        let mut tokens = BTreeMap::new();
        // Stored as uppercase in the resolved token.
        tokens.insert("color.upper".to_owned(), color_token("#0B1F33"));
        let contract = BrandContract {
            // Palette stored in lowercase (as the parser does).
            allowed_colors: Some(vec!["#0b1f33".to_owned()]),
            ..Default::default()
        };
        let codes = run(contract, tokens);
        assert!(
            codes.is_empty(),
            "color comparison must be case-insensitive; got: {codes:?}"
        );
    }

    #[test]
    fn unconstrained_color_category_no_diagnostic() {
        // allowed_colors is None → unconstrained → no diagnostic regardless of value.
        let mut tokens = BTreeMap::new();
        tokens.insert("color.x".to_owned(), color_token("#deadbe"));
        let contract = BrandContract {
            allowed_colors: None,
            allowed_fonts: Some(vec![]),
            allowed_weights: None,
            source_span: None,
        };
        let codes = run(contract, tokens);
        assert!(
            !has_code(&codes, "brand.color_off_palette"),
            "unconstrained color must not fire brand.color_off_palette; got: {codes:?}"
        );
    }

    // ── CMYK color checks ─────────────────────────────────────────────────────

    #[test]
    fn cmyk_color_on_palette_no_diagnostic() {
        let mut tokens = BTreeMap::new();
        tokens.insert(
            "color.cmyk".to_owned(),
            ResolvedToken {
                token_type: TokenType::Color,
                value: ResolvedValue::CmykColor {
                    hex: "#0b1f33".to_owned(),
                    c: 95.0,
                    m: 68.0,
                    y: 20.0,
                    k: 80.0,
                },
            },
        );
        let contract = BrandContract {
            allowed_colors: Some(vec!["#0b1f33".to_owned()]),
            ..Default::default()
        };
        let codes = run(contract, tokens);
        assert!(
            codes.is_empty(),
            "CMYK on-palette color must not fire diagnostic; got: {codes:?}"
        );
    }

    #[test]
    fn cmyk_color_off_palette_fires_diagnostic() {
        let mut tokens = BTreeMap::new();
        tokens.insert(
            "color.cmyk.off".to_owned(),
            ResolvedToken {
                token_type: TokenType::Color,
                value: ResolvedValue::CmykColor {
                    hex: "#ff0000".to_owned(),
                    c: 0.0,
                    m: 100.0,
                    y: 100.0,
                    k: 0.0,
                },
            },
        );
        let contract = BrandContract {
            allowed_colors: Some(vec!["#0b1f33".to_owned()]),
            ..Default::default()
        };
        let codes = run(contract, tokens);
        assert!(
            has_code(&codes, "brand.color_off_palette"),
            "CMYK off-palette must fire brand.color_off_palette; got: {codes:?}"
        );
    }

    // ── Font checks ───────────────────────────────────────────────────────────

    #[test]
    fn font_on_list_no_diagnostic() {
        let mut tokens = BTreeMap::new();
        tokens.insert("font.body".to_owned(), font_token("Noto Sans"));
        let contract = BrandContract {
            allowed_fonts: Some(vec!["Noto Sans".to_owned()]),
            ..Default::default()
        };
        let codes = run(contract, tokens);
        assert!(
            codes.is_empty(),
            "on-list font must not fire brand.font_not_allowed; got: {codes:?}"
        );
    }

    #[test]
    fn font_off_list_fires_diagnostic() {
        let mut tokens = BTreeMap::new();
        tokens.insert("font.bad".to_owned(), font_token("Comic Sans MS"));
        let contract = BrandContract {
            allowed_fonts: Some(vec!["Noto Sans".to_owned()]),
            ..Default::default()
        };
        let codes = run(contract, tokens);
        assert!(
            has_code(&codes, "brand.font_not_allowed"),
            "off-list font must fire brand.font_not_allowed; got: {codes:?}"
        );
    }

    #[test]
    fn unconstrained_font_category_no_diagnostic() {
        let mut tokens = BTreeMap::new();
        tokens.insert("font.x".to_owned(), font_token("Whatever Font"));
        let contract = BrandContract {
            allowed_fonts: None,
            ..Default::default()
        };
        let codes = run(contract, tokens);
        assert!(
            !has_code(&codes, "brand.font_not_allowed"),
            "unconstrained font must not fire diagnostic; got: {codes:?}"
        );
    }

    // ── Weight checks ─────────────────────────────────────────────────────────

    #[test]
    fn weight_on_list_no_diagnostic() {
        let mut tokens = BTreeMap::new();
        tokens.insert("weight.regular".to_owned(), weight_token(400));
        let contract = BrandContract {
            allowed_weights: Some(vec![400, 700]),
            ..Default::default()
        };
        let codes = run(contract, tokens);
        assert!(
            codes.is_empty(),
            "on-list weight must not fire brand.weight_not_allowed; got: {codes:?}"
        );
    }

    #[test]
    fn weight_off_list_fires_diagnostic() {
        let mut tokens = BTreeMap::new();
        tokens.insert("weight.thin".to_owned(), weight_token(100));
        let contract = BrandContract {
            allowed_weights: Some(vec![400, 700]),
            ..Default::default()
        };
        let codes = run(contract, tokens);
        assert!(
            has_code(&codes, "brand.weight_not_allowed"),
            "off-list weight must fire brand.weight_not_allowed; got: {codes:?}"
        );
    }

    #[test]
    fn unconstrained_weight_category_no_diagnostic() {
        let mut tokens = BTreeMap::new();
        tokens.insert("weight.x".to_owned(), weight_token(300));
        let contract = BrandContract {
            allowed_weights: None,
            ..Default::default()
        };
        let codes = run(contract, tokens);
        assert!(
            !has_code(&codes, "brand.weight_not_allowed"),
            "unconstrained weight must not fire diagnostic; got: {codes:?}"
        );
    }

    // ── Non-color/font/weight tokens are not brand-governed ───────────────────

    #[test]
    fn dimension_token_not_governed() {
        use crate::ast::value::{Dimension, Unit};
        let mut tokens = BTreeMap::new();
        tokens.insert(
            "size.base".to_owned(),
            ResolvedToken {
                token_type: TokenType::Dimension,
                value: ResolvedValue::Dimension(Dimension {
                    value: 16.0,
                    unit: Unit::Px,
                }),
            },
        );
        // Even with all categories constrained, Dimension tokens produce no brand diags.
        let contract = BrandContract {
            allowed_colors: Some(vec![]),
            allowed_fonts: Some(vec![]),
            allowed_weights: Some(vec![]),
            source_span: None,
        };
        let codes = run(contract, tokens);
        assert!(
            codes.is_empty(),
            "dimension token must not produce brand diagnostics; got: {codes:?}"
        );
    }

    // ── Severity is Warning ───────────────────────────────────────────────────

    #[test]
    fn brand_diagnostics_are_warnings() {
        let mut tokens = BTreeMap::new();
        tokens.insert("color.bad".to_owned(), color_token("#ff0000"));
        let contract = BrandContract {
            allowed_colors: Some(vec!["#0b1f33".to_owned()]),
            ..Default::default()
        };
        let mut diags = Vec::new();
        check_brand_contract(&contract, &tokens, &mut diags);
        for d in &diags {
            assert_eq!(
                d.severity,
                Severity::Warning,
                "brand diagnostic must be a Warning, got {:?}",
                d.severity
            );
        }
    }
}
