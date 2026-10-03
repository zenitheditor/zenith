//! Visual-property validation: token-reference integrity, type compatibility,
//! and raw-literal detection.
//!
//! [`check_visual_prop`] is the single entry point used by the node walk, the
//! page-background check, and the style-block check. It also records every
//! referenced token id (transitively for gradient/shadow tokens) so the
//! unused-token pass can diff against the defined token ids.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::Span;
use crate::ast::block_style::BlockStyle;
use crate::ast::token::TokenType;
use crate::ast::value::PropertyValue;
use crate::diagnostics::{Diagnostic, FixHint};
use crate::suggest::{
    LiteralValue, best_token, exact_token, find_token_suggestion, format_candidate_list,
    literal_text, raw_literal_message, replace_token_ref_fix, unknown_reference_message,
};
use crate::tokens::{ResolvedToken, ResolvedValue};

/// The expected token type for a visual property.
///
/// Only the subset of visual properties that have defined expectations in v0
/// are listed here. Properties with no expectation (e.g. `line-height`,
/// `padding`, `gap`) are skipped to avoid false-positives — the contract
/// says "if a property has no defined expectation yet, skip it."
#[derive(Debug, Clone, Copy)]
pub(super) enum VisualExpect {
    Color,
    /// A fill/background slot that accepts either a color or a gradient token.
    ColorOrGradient,
    Dimension,
    FontFamily,
    FontWeight,
    /// A shadow slot that accepts a shadow token.
    Shadow,
    /// A filter slot that accepts a filter token.
    Filter,
    /// A mask slot that accepts a mask token.
    Mask,
}

/// Check a single visual property value:
/// - `None` → no-op (property is optional).
/// - `TokenRef(id)` → record the reference; check existence and type compat.
/// - `Literal(...)` → `token.raw_visual_literal` (Error).
pub(super) fn check_visual_prop(
    node_id: &str,
    prop_name: &str,
    value: Option<&PropertyValue>,
    expect: VisualExpect,
    referenced_token_ids: &mut BTreeSet<String>,
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(pv) = value else {
        return;
    };

    match pv {
        PropertyValue::TokenRef(token_id) => {
            // Record as referenced (for unused-token check).
            referenced_token_ids.insert(token_id.clone());

            // Existence check.
            let Some(resolved) = resolved_tokens.get(token_id.as_str()) else {
                let accepted = accepted_token_ids(expect, resolved_tokens);
                diagnostics.push(
                    Diagnostic::error(
                        "token.unknown_reference",
                        unknown_reference_message(
                            node_id,
                            prop_name,
                            token_id,
                            &unknown_token_hint(token_id, expect, resolved_tokens),
                        ),
                        None,
                        Some(node_id.to_owned()),
                    )
                    .with_fix(replace_token_ref_fix(prop_name, token_id, accepted)),
                );
                return;
            };

            // If this is a gradient token, its stop color tokens are referenced
            // transitively — record them so they are not falsely flagged
            // `token.unused`.
            if let ResolvedValue::Gradient(g) = &resolved.value {
                for (_, color_id) in &g.stops {
                    referenced_token_ids.insert(color_id.clone());
                }
            }

            // Likewise, a shadow token references its per-layer color tokens
            // transitively — record them so they are not falsely flagged
            // `token.unused`.
            if let ResolvedValue::Shadow(s) = &resolved.value {
                for layer in &s.layers {
                    referenced_token_ids.insert(layer.color_token.clone());
                }
            }

            // A filter token may carry duotone ops that reference shadow/highlight
            // color tokens transitively — record them so they are not falsely
            // flagged `token.unused`.
            if let ResolvedValue::Filter(f) = &resolved.value {
                for op in &f.ops {
                    if let Some(c) = &op.shadow {
                        referenced_token_ids.insert(c.clone());
                    }
                    if let Some(c) = &op.highlight {
                        referenced_token_ids.insert(c.clone());
                    }
                }
            }

            // Type compatibility check.
            let type_ok = expect_accepts(expect, &resolved.token_type);

            if !type_ok {
                diagnostics.push(Diagnostic::error(
                    "token.incompatible_property",
                    format!(
                        "node '{}': property '{}' expects a {} token but \
                         '{}' is of type '{}'",
                        node_id,
                        prop_name,
                        visual_expect_name(expect),
                        token_id,
                        token_type_name(&resolved.token_type),
                    ),
                    None,
                    Some(node_id.to_owned()),
                ));
            }
        }

        PropertyValue::Literal(_) | PropertyValue::Dimension(_) => {
            let (hint, fix) = raw_literal_analysis(expect, prop_name, pv, resolved_tokens);
            diagnostics.push(
                Diagnostic::error(
                    "token.raw_visual_literal",
                    raw_literal_message(node_id, prop_name, &hint),
                    None,
                    Some(node_id.to_owned()),
                )
                .with_fix(fix),
            );
        }

        // A data-binding reference is a valid future-facing value; no error is
        // emitted here — scene-side resolution emits `data.missing_field` or
        // `data.no_context` advisories at compile time when needed.
        PropertyValue::DataRef(_) => {}
    }
}

/// Check all token-referencing properties in a slice of `block role="…"` decls.
///
/// Called at the three block-style scopes (document body, page, text node) so
/// that tokens referenced ONLY via a `block` decl are recorded as used and any
/// missing or wrong-type token reference is diagnosed. Non-token fields
/// (`align`, `italic`, `space_before`, `space_after`) carry no token ref —
/// they are skipped.
pub(super) fn check_block_styles(
    scope_id: &str,
    block_styles: &[BlockStyle],
    referenced_token_ids: &mut BTreeSet<String>,
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for bs in block_styles {
        let label = format!("{scope_id}[block role=\"{}\"]", bs.role);
        check_visual_prop(
            &label,
            "font-family",
            bs.font_family.as_ref(),
            VisualExpect::FontFamily,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
        check_visual_prop(
            &label,
            "font-size",
            bs.font_size.as_ref(),
            VisualExpect::Dimension,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
        check_visual_prop(
            &label,
            "font-weight",
            bs.font_weight.as_ref(),
            VisualExpect::FontWeight,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
        check_visual_prop(
            &label,
            "fill",
            bs.fill.as_ref(),
            VisualExpect::Color,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
    }
}

/// Attach `span` to every `token.unknown_reference` and
/// `token.raw_visual_literal` diagnostic at index `from` or later that has no
/// span yet.
///
/// The visual-property checks run without a source position; the caller that
/// owns the node (or page, or style) fills it in afterwards so `validate`
/// prints line numbers. Diagnostics that already carry a span (for example
/// from a nested child) are left untouched.
pub(super) fn attach_visual_spans(diagnostics: &mut [Diagnostic], from: usize, span: Option<Span>) {
    let Some(tail) = diagnostics.get_mut(from..) else {
        return;
    };
    for d in tail {
        if d.span.is_none()
            && matches!(
                d.code.as_str(),
                "token.unknown_reference" | "token.raw_visual_literal"
            )
        {
            d.span = span;
        }
    }
}

/// `true` when a property with expectation `expect` accepts a token of type
/// `token_type`.
fn expect_accepts(expect: VisualExpect, token_type: &TokenType) -> bool {
    match expect {
        VisualExpect::Color => matches!(token_type, TokenType::Color),
        VisualExpect::ColorOrGradient => {
            matches!(token_type, TokenType::Color | TokenType::Gradient)
        }
        VisualExpect::Dimension => matches!(token_type, TokenType::Dimension),
        VisualExpect::FontFamily => matches!(token_type, TokenType::FontFamily),
        VisualExpect::FontWeight => matches!(token_type, TokenType::FontWeight),
        VisualExpect::Shadow => matches!(token_type, TokenType::Shadow),
        VisualExpect::Filter => matches!(token_type, TokenType::Filter),
        VisualExpect::Mask => matches!(token_type, TokenType::Mask),
    }
}

/// Ids of every resolved token that a property with `expect` accepts, sorted.
fn accepted_token_ids(
    expect: VisualExpect,
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
) -> Vec<&str> {
    resolved_tokens
        .iter()
        .filter(|(_, t)| expect_accepts(expect, &t.token_type))
        .map(|(id, _)| id.as_str())
        .collect()
}

/// Next-action text for an unknown token reference: a did-you-mean when a
/// declared token of an accepted type is close, otherwise the declared list.
///
/// Ranking: a declared prefix of `token_id` first (`color.primary.500` →
/// `color.primary`), then edit distance, then the dotted-segment fallback.
pub(super) fn unknown_token_hint(
    token_id: &str,
    expect: VisualExpect,
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
) -> String {
    let ids = accepted_token_ids(expect, resolved_tokens);
    match find_token_suggestion(token_id, ids.iter().copied()) {
        Some(s) => format!("did you mean '{s}'?"),
        None => format_candidate_list(&format!("{} tokens", visual_expect_name(expect)), ids),
    }
}

/// Next-action text for a raw visual literal. See [`raw_literal_analysis`].
pub(super) fn raw_literal_hint(
    expect: VisualExpect,
    prop_name: &str,
    value: &PropertyValue,
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
) -> String {
    raw_literal_analysis(expect, prop_name, value, resolved_tokens).0
}

/// Next-action text and fix hint for a raw visual literal.
///
/// The text names the expected token type, the token with the same value
/// (else the nearest value), and the declared candidates. Without a value
/// match, an example reference stands in. The hint exists when `zenith fix`
/// can reference or mint a token for the value.
fn raw_literal_analysis(
    expect: VisualExpect,
    prop_name: &str,
    value: &PropertyValue,
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
) -> (String, Option<FixHint>) {
    let label = visual_expect_name(expect);
    let ids = accepted_token_ids(expect, resolved_tokens);
    let lit = literal_value(expect, value);
    let exact = lit
        .as_ref()
        .and_then(|l| exact_token(l, prop_name, resolved_tokens))
        .filter(|id| ids.contains(id));
    let best = lit
        .as_ref()
        .and_then(|l| best_token(l, prop_name, resolved_tokens))
        .filter(|m| ids.contains(&m.id));
    let nearest = best.as_ref().filter(|m| Some(m.id) != exact);
    let fix = match (&lit, mint_type(expect), literal_text(value)) {
        (Some(_), Some(token_type), Some(literal)) => Some(FixHint::RawLiteral {
            property: prop_name.to_owned(),
            literal,
            token_type: token_type.to_owned(),
            exact_match: exact.map(str::to_owned),
            nearest: nearest.map(|m| m.id.to_owned()),
        }),
        _ => None,
    };
    let Some(first) = ids.first() else {
        let text = format!(
            "expects a {label} token; no {label} tokens declared — \
             declare one in the `tokens` block"
        );
        return (text, fix);
    };
    let candidates = format_candidate_list(&format!("{label} tokens"), ids.iter().copied());
    let text = match (exact, nearest) {
        (Some(id), _) => {
            let shown = best
                .as_ref()
                .filter(|m| m.id == id)
                .map_or(String::new(), |m| format!(" ({})", m.value));
            format!("expects a {label} token; use (token)\"{id}\"{shown}; {candidates}")
        }
        (None, Some(m)) => format!(
            "expects a {label} token; nearest is (token)\"{}\" ({}); {candidates}",
            m.id, m.value
        ),
        (None, None) => format!("expects a {label} token, e.g. (token)\"{first}\"; {candidates}"),
    };
    (text, fix)
}

/// The token type `zenith fix` mints for a raw literal under `expect`.
fn mint_type(expect: VisualExpect) -> Option<&'static str> {
    match expect {
        VisualExpect::Color | VisualExpect::ColorOrGradient => Some("color"),
        VisualExpect::Dimension => Some("dimension"),
        VisualExpect::FontFamily => Some("fontFamily"),
        VisualExpect::FontWeight => Some("fontWeight"),
        VisualExpect::Shadow | VisualExpect::Filter | VisualExpect::Mask => None,
    }
}

/// Read a raw property value as a [`LiteralValue`] of the kind `expect` takes.
fn literal_value(expect: VisualExpect, value: &PropertyValue) -> Option<LiteralValue> {
    match (expect, value) {
        (VisualExpect::Color | VisualExpect::ColorOrGradient, PropertyValue::Literal(s)) => {
            LiteralValue::color(s)
        }
        (VisualExpect::Dimension, PropertyValue::Dimension(d)) => LiteralValue::dimension(d),
        (VisualExpect::FontFamily, PropertyValue::Literal(s)) => LiteralValue::font_family(s),
        (VisualExpect::FontWeight, PropertyValue::Literal(s)) => LiteralValue::font_weight(s),
        (
            VisualExpect::Color
            | VisualExpect::ColorOrGradient
            | VisualExpect::Dimension
            | VisualExpect::FontFamily
            | VisualExpect::FontWeight
            | VisualExpect::Shadow
            | VisualExpect::Filter
            | VisualExpect::Mask,
            PropertyValue::TokenRef(_)
            | PropertyValue::Literal(_)
            | PropertyValue::Dimension(_)
            | PropertyValue::DataRef(_),
        ) => None,
    }
}

fn visual_expect_name(e: VisualExpect) -> &'static str {
    match e {
        VisualExpect::Color => "color",
        VisualExpect::ColorOrGradient => "color or gradient",
        VisualExpect::Dimension => "dimension",
        VisualExpect::FontFamily => "fontFamily",
        VisualExpect::FontWeight => "fontWeight",
        VisualExpect::Shadow => "shadow",
        VisualExpect::Filter => "filter",
        VisualExpect::Mask => "mask",
    }
}

fn token_type_name(t: &TokenType) -> &str {
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
