//! Resolution driver: the public [`resolve_tokens`] entry point, the alias-chain
//! walk with cycle detection, and the gradient/shadow cross-check passes.
//!
//! # Algorithm overview
//!
//! 1. Build an index `id → &Token`, detecting duplicates with
//!    `token.duplicate_id` (first definition wins).
//! 2. Walk every token in source order:
//!    - If `Unknown` type → `token.unknown_type` (Warning); skip resolution.
//!    - If `Reference` → follow the alias chain iteratively with a visited set
//!      to detect cycles. Bounded by the number of distinct token IDs, so it
//!      can never loop infinitely.
//!    - If `Literal` → validate shape for the declared type.
//! 3. Emit `token.invalid_value`, `token.unknown_reference`,
//!    `token.cyclic_reference`, and `token.type_mismatch` as appropriate.
//! 4. Populate `resolved` only for tokens that passed all checks.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::token::{Token, TokenBlock, TokenLiteral, TokenType, TokenValue};
use crate::diagnostics::Diagnostic;

use super::types::{ResolvedToken, ResolvedValue, TokenResolution};
use super::validate::{type_name_of, validate_literal};

/// Resolve all tokens in `block`, collecting diagnostics without hard-failing.
///
/// Tokens that cannot be resolved (e.g., due to an unknown reference or cycle)
/// are omitted from `resolved`; all other tokens are resolved and included.
pub fn resolve_tokens(block: &TokenBlock) -> TokenResolution {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let mut resolved: BTreeMap<String, ResolvedToken> = BTreeMap::new();

    // ── Step 1: build index, detecting duplicate IDs ─────────────────────
    // `index` maps id → token reference (first definition wins).
    let mut index: BTreeMap<&str, &Token> = BTreeMap::new();
    // Track which IDs have been seen for deterministic duplicate detection.
    let mut seen_ids: BTreeSet<&str> = BTreeSet::new();

    for token in &block.tokens {
        if seen_ids.contains(token.id.as_str()) {
            diagnostics.push(Diagnostic::error(
                "token.duplicate_id",
                format!(
                    "token '{}' is defined more than once; the second definition is ignored",
                    token.id
                ),
                token.source_span,
                Some(token.id.clone()),
            ));
            // First definition already in index; skip.
        } else {
            seen_ids.insert(token.id.as_str());
            index.insert(token.id.as_str(), token);
        }
    }

    // ── Step 2: resolve each first-definition token ───────────────────────
    for token in &block.tokens {
        // Only process the canonical (first-definition) entry for each ID.
        // `index.get()` returns None for duplicates (which were never inserted),
        // and Some(ptr) != token for any future edge-case; neither path panics.
        let Some(canonical) = index.get(token.id.as_str()) else {
            continue;
        };
        if !std::ptr::eq(*canonical, token) {
            continue;
        }

        // Unknown type → advisory warning, skip resolution.
        if let TokenType::Unknown(ref type_name) = token.token_type {
            diagnostics.push(Diagnostic::warning(
                "token.unknown_type",
                format!(
                    "token '{}' has unrecognized type '{}' (version-relative; \
                     this type may be valid in a later schema version)",
                    token.id, type_name
                ),
                token.source_span,
                Some(token.id.clone()),
            ));
            continue;
        }

        // Resolve to a concrete literal (following aliases as needed).
        match resolve_token_to_literal(token, &index, &mut diagnostics) {
            Some((literal, resolved_type)) => {
                // Type must match the declaring token's type.
                if resolved_type != token.token_type {
                    diagnostics.push(Diagnostic::error(
                        "token.type_mismatch",
                        format!(
                            "token '{}' has declared type '{}' but its alias \
                             chain resolves to a token of type '{}'",
                            token.id,
                            type_name_of(&token.token_type),
                            type_name_of(&resolved_type),
                        ),
                        token.source_span,
                        Some(token.id.clone()),
                    ));
                    continue;
                }

                // Validate the literal's shape against the declared type.
                match validate_literal(
                    &token.id,
                    &token.token_type,
                    &literal,
                    token.source_span,
                    &mut diagnostics,
                ) {
                    Some(rv) => {
                        resolved.insert(
                            token.id.clone(),
                            ResolvedToken {
                                token_type: token.token_type.clone(),
                                value: rv,
                            },
                        );
                    }
                    None => {
                        // validate_literal already pushed a diagnostic.
                    }
                }
            }
            None => {
                // resolve_token_to_literal already pushed a diagnostic.
            }
        }
    }

    // ── Step 3: gradient stop-color cross-check ───────────────────────────
    // Now that every token's resolved value is known, verify that each
    // gradient stop references a token that exists AND resolved to a Color.
    // Iterate the resolved map (BTreeMap → deterministic id order) and clone
    // out the gradient stop lists so we don't borrow `resolved` while reading
    // other entries from it.
    let gradient_stops: Vec<(String, Vec<String>)> = resolved
        .iter()
        .filter_map(|(id, rt)| match &rt.value {
            ResolvedValue::Gradient(g) => Some((
                id.clone(),
                g.stops
                    .iter()
                    .map(|(_, color_id)| color_id.clone())
                    .collect(),
            )),
            ResolvedValue::Color(_)
            | ResolvedValue::CmykColor { .. }
            | ResolvedValue::Dimension(_)
            | ResolvedValue::Number(_)
            | ResolvedValue::FontFamily(_)
            | ResolvedValue::FontWeight(_)
            | ResolvedValue::Shadow(_)
            | ResolvedValue::Filter(_)
            | ResolvedValue::Mask(_) => None,
        })
        .collect();

    for (id, stop_color_ids) in &gradient_stops {
        // The declaring token's span, looked up via the source index.
        let span = index.get(id.as_str()).and_then(|t| t.source_span);
        for color_token_id in stop_color_ids {
            match resolved.get(color_token_id.as_str()) {
                None => diagnostics.push(Diagnostic::error(
                    "gradient.stop_unresolved",
                    format!(
                        "gradient '{}' stop references unknown token '{}'",
                        id, color_token_id
                    ),
                    span,
                    Some(id.clone()),
                )),
                Some(rt) if rt.value.as_color_hex().is_none() => {
                    diagnostics.push(Diagnostic::error(
                        "gradient.stop_wrong_type",
                        format!(
                            "gradient '{}' stop references token '{}' of type '{}' \
                             but a color token is required",
                            id,
                            color_token_id,
                            type_name_of(&rt.token_type),
                        ),
                        span,
                        Some(id.clone()),
                    ));
                }
                Some(_) => {}
            }
        }
    }

    // ── Step 4: shadow layer-color cross-check ────────────────────────────
    // Now that every token's resolved value is known, verify that each shadow
    // layer references a token that exists AND resolved to a Color. Iterate the
    // resolved map (BTreeMap → deterministic id order) and clone out the layer
    // color lists so we don't borrow `resolved` while reading other entries.
    let shadow_layers: Vec<(String, Vec<String>)> = resolved
        .iter()
        .filter_map(|(id, rt)| match &rt.value {
            ResolvedValue::Shadow(s) => Some((
                id.clone(),
                s.layers
                    .iter()
                    .map(|layer| layer.color_token.clone())
                    .collect(),
            )),
            ResolvedValue::Color(_)
            | ResolvedValue::CmykColor { .. }
            | ResolvedValue::Dimension(_)
            | ResolvedValue::Number(_)
            | ResolvedValue::FontFamily(_)
            | ResolvedValue::FontWeight(_)
            | ResolvedValue::Gradient(_)
            | ResolvedValue::Filter(_)
            | ResolvedValue::Mask(_) => None,
        })
        .collect();

    for (id, layer_color_ids) in &shadow_layers {
        // The declaring token's span, looked up via the source index.
        let span = index.get(id.as_str()).and_then(|t| t.source_span);
        for color_token_id in layer_color_ids {
            match resolved.get(color_token_id.as_str()) {
                None => diagnostics.push(Diagnostic::error(
                    "shadow.layer_unresolved",
                    format!(
                        "shadow '{}' layer references unknown token '{}'",
                        id, color_token_id
                    ),
                    span,
                    Some(id.clone()),
                )),
                Some(rt) if rt.value.as_color_hex().is_none() => {
                    diagnostics.push(Diagnostic::error(
                        "shadow.layer_wrong_type",
                        format!(
                            "shadow '{}' layer references token '{}' of type '{}' \
                             but a color token is required",
                            id,
                            color_token_id,
                            type_name_of(&rt.token_type),
                        ),
                        span,
                        Some(id.clone()),
                    ));
                }
                Some(_) => {}
            }
        }
    }

    TokenResolution {
        resolved,
        diagnostics,
    }
}

// ── Alias-chain resolution ────────────────────────────────────────────────────

/// Follow the alias chain from `start` until a literal is reached, or until a
/// cycle / missing reference is detected.
///
/// Returns `Some((literal, type_of_literal_token))` on success.
/// Pushes exactly one diagnostic and returns `None` on failure.
///
/// The walk is **iterative** and terminates in at most `index.len()` steps,
/// so it is safe against arbitrarily long or cyclic chains.
fn resolve_token_to_literal<'a>(
    start: &'a Token,
    index: &BTreeMap<&str, &'a Token>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(TokenLiteral, TokenType)> {
    // visited tracks IDs we've stepped through, used for cycle detection.
    let mut visited: BTreeSet<&str> = BTreeSet::new();
    let mut current: &Token = start;

    loop {
        match &current.value {
            TokenValue::Literal(lit) => {
                return Some((lit.clone(), current.token_type.clone()));
            }
            TokenValue::Reference { token_id } => {
                // Check for a cycle: if we've seen this target before we're
                // in a cycle.
                if visited.contains(token_id.as_str()) {
                    diagnostics.push(Diagnostic::error(
                        "token.cyclic_reference",
                        format!(
                            "token '{}' participates in a cyclic alias chain \
                             (cycle detected at '{}')",
                            start.id, token_id
                        ),
                        start.source_span,
                        Some(start.id.clone()),
                    ));
                    return None;
                }

                // Check for self-reference before we insert current into
                // visited, so `a → a` is caught on the first step.
                if token_id == &current.id {
                    diagnostics.push(Diagnostic::error(
                        "token.cyclic_reference",
                        format!("token '{}' references itself", current.id),
                        current.source_span,
                        Some(current.id.clone()),
                    ));
                    return None;
                }

                // Record that we've visited the current node *before* following
                // the reference, so we detect `a → b → a` correctly.
                visited.insert(current.id.as_str());

                // Resolve the reference target.
                match index.get(token_id.as_str()) {
                    Some(next) => {
                        current = next;
                    }
                    None => {
                        diagnostics.push(Diagnostic::error(
                            "token.unknown_reference",
                            format!(
                                "token '{}' references '{}' which does not exist",
                                start.id, token_id
                            ),
                            start.source_span,
                            Some(start.id.clone()),
                        ));
                        return None;
                    }
                }
            }
        }
    }
}
