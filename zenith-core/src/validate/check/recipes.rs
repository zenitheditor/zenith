//! Validation for the top-level `recipes` block.
//!
//! Checks performed (all `Error` severity):
//!
//! 1. **`recipe.duplicate_id`** — two `recipe` entries share the same `id`.
//!    Recipe ids live in their own namespace (they are not document node ids)
//!    so a dedicated check is used rather than the global `register_id` funnel.
//! 2. **`recipe.unknown_palette_token`** — a `palette` entry names a token id
//!    that is either undeclared or declared with a non-color type.
//! 3. **`recipe.unknown_expanded_node`** — an `expanded` entry names a node id
//!    that does not exist anywhere in the document (pages, masters, components).
//! 4. **`recipe.unknown_bounds`** — `recipe.bounds` names an id that is neither
//!    a declared page id nor a document-wide node id.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::document::Document;
use crate::ast::token::TokenType;
use crate::diagnostics::Diagnostic;

/// Validate the `recipes` block of `doc`.
///
/// `page_ids` — the set of page ids present in the document body.
///
/// `all_node_ids` — the set of all node ids across pages, masters, and
/// components. Built once by the driver before calling this function.
///
/// `token_type_map` — a map from token id to `TokenType` for every declared
/// token. Built once by the driver before calling this function; used to
/// distinguish "not a declared token" from "declared but non-color type".
pub(in crate::validate::check) fn check_recipes(
    doc: &Document,
    page_ids: &BTreeSet<&str>,
    all_node_ids: &BTreeSet<String>,
    token_type_map: &BTreeMap<&str, &TokenType>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // ── 1. Duplicate recipe id detection ─────────────────────────────────────
    // Recipe ids live in the recipes namespace; they are NOT document node ids.
    // We track them in a local BTreeSet and emit `recipe.duplicate_id` for each
    // duplicate (matching the pattern of `variant.duplicate_id`).
    let mut seen_recipe_ids: BTreeSet<&str> = BTreeSet::new();

    for recipe in &doc.recipes {
        if !seen_recipe_ids.insert(recipe.id.as_str()) {
            diagnostics.push(Diagnostic::error(
                "recipe.duplicate_id",
                format!(
                    "recipe '{}': id is declared more than once; \
                     recipe ids must be unique within the recipes block",
                    recipe.id
                ),
                recipe.source_span,
                Some(recipe.id.clone()),
            ));
        }

        // ── 2. Unknown / non-color palette tokens ─────────────────────────────
        for token_id in &recipe.palette {
            let msg = match token_type_map.get(token_id.as_str()) {
                None => Some(format!(
                    "recipe '{}': palette token '{}' is not declared in the tokens block",
                    recipe.id, token_id
                )),
                Some(TokenType::Color) => None,
                Some(TokenType::Dimension) => Some(format!(
                    "recipe '{}': palette token '{}' is declared but has type 'dimension', \
                     not 'color'; palette entries must be color tokens",
                    recipe.id, token_id
                )),
                Some(TokenType::Number) => Some(format!(
                    "recipe '{}': palette token '{}' is declared but has type 'number', \
                     not 'color'; palette entries must be color tokens",
                    recipe.id, token_id
                )),
                Some(TokenType::FontFamily) => Some(format!(
                    "recipe '{}': palette token '{}' is declared but has type 'fontFamily', \
                     not 'color'; palette entries must be color tokens",
                    recipe.id, token_id
                )),
                Some(TokenType::FontWeight) => Some(format!(
                    "recipe '{}': palette token '{}' is declared but has type 'fontWeight', \
                     not 'color'; palette entries must be color tokens",
                    recipe.id, token_id
                )),
                Some(TokenType::Gradient) => Some(format!(
                    "recipe '{}': palette token '{}' is declared but has type 'gradient', \
                     not 'color'; palette entries must be color tokens",
                    recipe.id, token_id
                )),
                Some(TokenType::Shadow) => Some(format!(
                    "recipe '{}': palette token '{}' is declared but has type 'shadow', \
                     not 'color'; palette entries must be color tokens",
                    recipe.id, token_id
                )),
                Some(TokenType::Filter) => Some(format!(
                    "recipe '{}': palette token '{}' is declared but has type 'filter', \
                     not 'color'; palette entries must be color tokens",
                    recipe.id, token_id
                )),
                Some(TokenType::Mask) => Some(format!(
                    "recipe '{}': palette token '{}' is declared but has type 'mask', \
                     not 'color'; palette entries must be color tokens",
                    recipe.id, token_id
                )),
                Some(TokenType::Unknown(type_name)) => Some(format!(
                    "recipe '{}': palette token '{}' is declared but has type '{}', \
                     not 'color'; palette entries must be color tokens",
                    recipe.id, token_id, type_name
                )),
            };
            if let Some(msg) = msg {
                diagnostics.push(Diagnostic::error(
                    "recipe.unknown_palette_token",
                    msg,
                    recipe.source_span,
                    Some(recipe.id.clone()),
                ));
            }
        }

        // ── 3. Unknown expanded node ids ──────────────────────────────────────
        for node_id in &recipe.expanded {
            if !all_node_ids.contains(node_id.as_str()) {
                diagnostics.push(Diagnostic::error(
                    "recipe.unknown_expanded_node",
                    format!(
                        "recipe '{}': expanded node '{}' does not exist anywhere in this document",
                        recipe.id, node_id
                    ),
                    recipe.source_span,
                    Some(recipe.id.clone()),
                ));
            }
        }

        // ── 4. Unknown bounds id ──────────────────────────────────────────────
        // `bounds` may name a page id OR any document node id (e.g. a frame).
        // Only checked when bounds is present.
        if let Some(bounds_id) = &recipe.bounds {
            let known_as_page = page_ids.contains(bounds_id.as_str());
            let known_as_node = all_node_ids.contains(bounds_id.as_str());
            if !known_as_page && !known_as_node {
                diagnostics.push(Diagnostic::error(
                    "recipe.unknown_bounds",
                    format!(
                        "recipe '{}': bounds '{}' does not reference a declared page \
                         or node id in this document",
                        recipe.id, bounds_id
                    ),
                    recipe.source_span,
                    Some(recipe.id.clone()),
                ));
            }
        }
    }
}
