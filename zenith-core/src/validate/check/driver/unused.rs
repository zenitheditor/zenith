//! Unused-token reporting, grouped by provenance `set`.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::document::Document;
use crate::diagnostics::Diagnostic;

/// Report unused tokens, grouped by their optional provenance `set` id.
///
/// Tokens are grouped by `set` (a `BTreeMap` for deterministic, lexicographic
/// emission). The `None` bucket (tokens with no `set`) is reported exactly as
/// before: one `token.unused` advisory per unreferenced token — this keeps a
/// document that never uses `set` byte-identical to prior output. A
/// `Some(set_id)` bucket with only one member also behaves like today (plain
/// per-token `token.unused`). A multi-token `Some(set_id)` bucket instead
/// collapses into a single `token.set_partially_used` advisory when some (but
/// not all) of its tokens are referenced, and emits nothing when every token
/// in the set is used; per-token `token.unused` is fully suppressed for those
/// members.
pub(super) fn check_unused_tokens(
    doc: &Document,
    referenced_token_ids: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut tokens_by_set: BTreeMap<Option<&str>, Vec<&crate::ast::Token>> = BTreeMap::new();
    for token in &doc.tokens.tokens {
        tokens_by_set
            .entry(token.set.as_deref())
            .or_default()
            .push(token);
    }

    for (set_id, tokens) in &tokens_by_set {
        let Some(set_id) = set_id.filter(|_| tokens.len() > 1) else {
            // No `set` (the default case) or a `set` with exactly one member:
            // report per-token `token.unused`, byte-identical to the
            // pre-`set` behavior.
            for token in tokens {
                if !referenced_token_ids.contains(&token.id) {
                    diagnostics.push(Diagnostic::advisory(
                        "token.unused",
                        format!(
                            "token '{}' is defined but never referenced by any node \
                             visual property or style in this document",
                            token.id
                        ),
                        token.source_span,
                        Some(token.id.clone()),
                    ));
                }
            }
            continue;
        };

        // A multi-token `set`: collapse into at most one advisory for the
        // whole set instead of one per unreferenced member.
        let total = tokens.len();
        let used = tokens
            .iter()
            .filter(|t| referenced_token_ids.contains(&t.id))
            .count();
        if used < total {
            let message = if used == 0 {
                format!("token set '{set_id}' has none of {total} tokens referenced")
            } else {
                format!("token set '{set_id}' has {used} of {total} tokens referenced")
            };
            // Anchor at the first token's span in this set (deterministic:
            // tokens are collected in document order) as a stand-in for "the
            // tokens block" — there is no dedicated `TokenBlock` span.
            let anchor_span = tokens.first().and_then(|t| t.source_span);
            diagnostics.push(Diagnostic::advisory(
                "token.set_partially_used",
                message,
                anchor_span,
                Some(set_id.to_owned()),
            ));
        }
    }
}
