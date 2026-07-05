//! Pure candidate-page merge and token-reconciliation helpers.
//!
//! [`merge_candidate_page`] deep-copies a source page's content into a target
//! page in place, suffixing every descendant id. [`reconcile_candidate_tokens`]
//! additively upserts a candidate's token palette into a deliverable's token
//! block. Both transforms are pure — no filesystem access, no session I/O, no
//! validation. The caller is responsible for validating the mutated document.

use zenith_core::ast::document::Page;
use zenith_core::ast::token::TokenBlock;

use crate::engine::structure::{suffix_ids_in_children, suffix_zone_and_fold_ids};

/// Reconcile the candidate's token palette into `target_tokens` using an
/// **additive upsert** strategy:
///
/// - Iterate `candidate_tokens.tokens` in source order.
/// - For each candidate token whose `id` matches an existing token in
///   `target_tokens.tokens`, **replace** that token in place (preserving the
///   deliverable's slot position for the shared id).
/// - For each candidate token whose `id` is absent from `target_tokens`,
///   **append** it (in candidate source order).
/// - Tokens that exist only in `target_tokens` are left untouched.
///
/// The result is deterministic: no `HashMap`/`HashSet` intermediaries; the
/// final order is the deliverable's original order (with in-place replacements)
/// followed by candidate-only tokens in their source order.
///
/// Pure transform — the caller must re-validate the mutated document.
pub fn reconcile_candidate_tokens(candidate_tokens: &TokenBlock, target_tokens: &mut TokenBlock) {
    for cand_token in &candidate_tokens.tokens {
        match target_tokens
            .tokens
            .iter_mut()
            .find(|t| t.id == cand_token.id)
        {
            Some(existing) => {
                *existing = cand_token.clone();
            }
            None => {
                target_tokens.tokens.push(cand_token.clone());
            }
        }
    }
}

/// Merge a candidate source page's content into `target` in place: deep-copy
/// the source's children, safe-zones, and folds with every descendant id
/// suffixed by `id_suffix`, replacing the target's content.
///
/// Pure transform — no filesystem, no validation (the caller validates the
/// resulting document).
pub fn merge_candidate_page(source: &Page, target: &mut Page, id_suffix: &str) {
    let mut children = source.children.clone();
    let mut safe_zones = source.safe_zones.clone();
    let mut folds = source.folds.clone();

    suffix_ids_in_children(&mut children, id_suffix);
    suffix_zone_and_fold_ids(&mut safe_zones, &mut folds, id_suffix);

    target.children = children;
    target.safe_zones = safe_zones;
    target.folds = folds;
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use zenith_core::ast::token::{Token, TokenBlock, TokenLiteral, TokenType, TokenValue};
    use zenith_core::{KdlAdapter, KdlSource};

    use super::{merge_candidate_page, reconcile_candidate_tokens};

    // ── reconcile_candidate_tokens unit tests ─────────────────────────────────

    fn make_color_token(id: &str, value: &str) -> Token {
        Token {
            id: id.to_owned(),
            token_type: TokenType::Color,
            value: TokenValue::Literal(TokenLiteral::String(value.to_owned())),
            set: None,
            source_span: None,
        }
    }

    fn make_token_block(tokens: Vec<Token>) -> TokenBlock {
        TokenBlock {
            format: "zenith-token-v1".to_owned(),
            tokens,
        }
    }

    /// Shared id → value replaced in place; deliverable-only id retained;
    /// candidate-only id appended; order is deterministic.
    #[test]
    fn reconcile_shared_id_replaced_in_place() {
        // Deliverable palette: [shared="#ff0000", deliverable-only="#00ff00"]
        let mut target = make_token_block(vec![
            make_color_token("color.shared", "#ff0000"),
            make_color_token("color.del-only", "#00ff00"),
        ]);
        // Candidate palette: [shared="#0000ff" (different value), cand-only="#aabbcc"]
        let candidate = make_token_block(vec![
            make_color_token("color.shared", "#0000ff"),
            make_color_token("color.cand-only", "#aabbcc"),
        ]);

        reconcile_candidate_tokens(&candidate, &mut target);

        // 3 tokens total: shared (replaced), del-only (kept), cand-only (appended)
        assert_eq!(
            target.tokens.len(),
            3,
            "reconciled block must have 3 tokens; got {:?}",
            target.tokens.iter().map(|t| &t.id).collect::<Vec<_>>()
        );

        // shared → candidate's value
        let shared = target
            .tokens
            .iter()
            .find(|t| t.id == "color.shared")
            .unwrap();
        assert_eq!(
            shared.value,
            TokenValue::Literal(TokenLiteral::String("#0000ff".to_owned())),
            "shared id must carry the candidate's value"
        );

        // deliverable-only → still present, unchanged
        let del_only = target
            .tokens
            .iter()
            .find(|t| t.id == "color.del-only")
            .unwrap();
        assert_eq!(
            del_only.value,
            TokenValue::Literal(TokenLiteral::String("#00ff00".to_owned())),
            "deliverable-only token must be retained unchanged"
        );

        // candidate-only → appended
        assert!(
            target.tokens.iter().any(|t| t.id == "color.cand-only"),
            "candidate-only token must be appended"
        );
    }

    /// Deliverable-slot order is preserved; candidate-only tokens follow.
    #[test]
    fn reconcile_preserves_deliverable_order() {
        let mut target = make_token_block(vec![
            make_color_token("tok.a", "#aaa"),
            make_color_token("tok.b", "#bbb"),
            make_color_token("tok.c", "#ccc"),
        ]);
        // Candidate replaces tok.b and adds tok.d (in candidate source order).
        let candidate = make_token_block(vec![
            make_color_token("tok.b", "#BBB"),
            make_color_token("tok.d", "#ddd"),
        ]);

        reconcile_candidate_tokens(&candidate, &mut target);

        let ids: Vec<&str> = target.tokens.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(
            ids,
            ["tok.a", "tok.b", "tok.c", "tok.d"],
            "order must be deliverable order then appended candidate-only; got {ids:?}"
        );
        // tok.b must have the new value.
        let tok_b = target.tokens.iter().find(|t| t.id == "tok.b").unwrap();
        assert_eq!(
            tok_b.value,
            TokenValue::Literal(TokenLiteral::String("#BBB".to_owned()))
        );
    }

    /// Empty candidate → target is left exactly as-is.
    #[test]
    fn reconcile_empty_candidate_leaves_target_unchanged() {
        let mut target = make_token_block(vec![make_color_token("tok.a", "#aaa")]);
        let candidate = make_token_block(vec![]);

        reconcile_candidate_tokens(&candidate, &mut target);

        assert_eq!(target.tokens.len(), 1);
        assert_eq!(target.tokens[0].id, "tok.a");
    }

    /// Empty deliverable → all candidate tokens appended in source order.
    #[test]
    fn reconcile_empty_target_appends_all_candidate_tokens() {
        let mut target = make_token_block(vec![]);
        let candidate = make_token_block(vec![
            make_color_token("tok.x", "#x"),
            make_color_token("tok.y", "#y"),
        ]);

        reconcile_candidate_tokens(&candidate, &mut target);

        let ids: Vec<&str> = target.tokens.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(
            ids,
            ["tok.x", "tok.y"],
            "all candidate tokens must be appended in order"
        );
    }

    // Parse a minimal document and return its first page (panics on error — tests only).
    fn parse_first_page(src: &str) -> zenith_core::ast::document::Page {
        KdlAdapter
            .parse(src.as_bytes())
            .expect("test doc must parse")
            .body
            .pages
            .into_iter()
            .next()
            .expect("test doc must have at least one page")
    }

    const SOURCE_DOC: &str = r##"zenith version=1 {
  project id="proj.src" name="Src"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.src" title="Src" {
    page id="page.source" w=(px)400 h=(px)300 {
      rect id="rect.a" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      rect id="rect.b" x=(px)100 y=(px)0 w=(px)100 h=(px)100
    }
  }
}
"##;

    const TARGET_DOC: &str = r##"zenith version=1 {
  project id="proj.tgt" name="Tgt"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.tgt" title="Tgt" {
    page id="page.target" w=(px)400 h=(px)300 {
      rect id="old.rect" x=(px)0 y=(px)0 w=(px)50 h=(px)50
    }
  }
}
"##;

    fn rect_id(node: &zenith_core::Node) -> Option<&str> {
        match node {
            zenith_core::Node::Rect(r) => Some(r.id.as_str()),
            _ => None,
        }
    }

    #[test]
    fn children_copied_with_suffixed_ids() {
        let source = parse_first_page(SOURCE_DOC);
        let mut target = parse_first_page(TARGET_DOC);

        merge_candidate_page(&source, &mut target, ".promoted");

        assert_eq!(target.children.len(), 2, "target must have 2 children");
        let ids: Vec<&str> = target.children.iter().filter_map(rect_id).collect();
        assert!(
            ids.contains(&"rect.a.promoted"),
            "rect.a must be suffixed; got {ids:?}"
        );
        assert!(
            ids.contains(&"rect.b.promoted"),
            "rect.b must be suffixed; got {ids:?}"
        );
    }

    #[test]
    fn source_unchanged_after_merge() {
        let source = parse_first_page(SOURCE_DOC);
        let source_children_len = source.children.len();
        let first_id = source
            .children
            .first()
            .and_then(rect_id)
            .unwrap()
            .to_owned();

        let mut target = parse_first_page(TARGET_DOC);
        merge_candidate_page(&source, &mut target, ".p");

        assert_eq!(
            source.children.len(),
            source_children_len,
            "source must not be mutated"
        );
        assert_eq!(
            source.children.first().and_then(rect_id),
            Some(first_id.as_str()),
        );
    }

    #[test]
    fn empty_source_replaces_target_children() {
        const EMPTY_SOURCE: &str = r##"zenith version=1 {
  project id="proj.empty" name="E"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.empty" title="E" {
    page id="page.source" w=(px)100 h=(px)100 {}
  }
}
"##;
        let source = parse_first_page(EMPTY_SOURCE);
        let mut target = parse_first_page(TARGET_DOC);
        assert!(
            !target.children.is_empty(),
            "target must start with children"
        );

        merge_candidate_page(&source, &mut target, ".p");

        assert!(
            target.children.is_empty(),
            "empty source must produce empty target children"
        );
    }
}
