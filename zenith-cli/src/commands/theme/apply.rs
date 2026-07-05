//! Pure logic for `zenith theme apply`.
//!
//! Re-skins a document's token values from a theme pack by building a
//! [`zenith_tx::Transaction`] and running it through the existing `tx`
//! pipeline ([`crate::commands::tx::run`]) as a black box — this module never
//! mutates a `Document` or formats KDL itself.
//!
//! For each token in the resolved theme pack:
//! - a same-id, same-[`TokenType`] token in the target doc gets an
//!   `Op::UpdateTokenValue`, with `set` stamped to the theme pack's resolved
//!   id so the re-skinned token's provenance reflects the applied theme;
//! - an id absent from the target doc gets an `Op::CreateToken`, stamped with
//!   `set` set to the theme pack's resolved id (e.g. `"@zenith/theme.cobalt"`)
//!   so the created token's provenance is recorded;
//! - a same-id token of a *different* type is left untouched and reported as
//!   a skip (`SkipReason::TypeMismatch`);
//! - a theme token whose value can't be expressed as an op literal string (a
//!   structured gradient/shadow/filter/mask, or an alias `(token)` reference)
//!   is left untouched and reported as a skip (`SkipReason::Unencodable`) —
//!   the tx engine rejects the whole transaction if any single op is invalid,
//!   so an unencodable token must never reach `ops` in the first place.
//!
//! Tokens that exist only in the target document (not in the theme) are never
//! touched.

use std::collections::BTreeMap;
use std::path::Path;

use zenith_core::{KdlAdapter, KdlSource as _, TokenLiteral, TokenType, TokenValue};
use zenith_tx::{Op, Permissions, Transaction};

use crate::commands::tx::run as tx_run;
use crate::library::resolve_theme_pack;

// ── Error / result types ──────────────────────────────────────────────────────

/// Error from `theme apply`: a message plus a process exit code.
#[derive(Debug)]
pub struct ThemeApplyErr {
    /// Human-readable message.
    pub message: String,
    /// Exit code (always 2: unknown pack, unreadable doc, or an internal
    /// pre-check failure).
    pub exit_code: u8,
}

/// Why a theme token was left untouched instead of becoming an op.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// The document already has a token with this id, but of a different
    /// declared [`TokenType`].
    TypeMismatch,
    /// The theme token's value can't be expressed as an op literal string
    /// (a structured gradient/shadow/filter/mask, or a `(token)` alias).
    Unencodable,
}

impl SkipReason {
    /// A short, stable label for human/JSON output.
    pub fn label(&self) -> &'static str {
        match self {
            SkipReason::TypeMismatch => "type_mismatch",
            SkipReason::Unencodable => "unencodable",
        }
    }
}

/// A theme token that was left untouched instead of becoming an op.
#[derive(Debug, Clone)]
pub struct SkippedToken {
    /// The token id.
    pub id: String,
    /// The target document's existing declared type for this id, as a label
    /// (`"color"`, `"dimension"`, …). `None` when the document has no token
    /// with this id at all (an unencodable theme-side value).
    pub doc_type: Option<String>,
    /// The theme's declared type for this id, as a label.
    pub theme_type: String,
    /// Why the token was skipped.
    pub reason: SkipReason,
}

/// The outcome of a successful `theme apply` run (even a rejected one — a
/// rejected underlying transaction is still a computed outcome, not an
/// [`ThemeApplyErr`]).
#[derive(Debug)]
pub struct ApplyOutcome {
    /// The underlying transaction result (source before/after, diagnostics,
    /// affected ids, status).
    pub result: zenith_tx::TxResult,
    /// Ids of tokens created (present in the theme, absent from the doc).
    pub added_tokens: Vec<String>,
    /// Theme tokens left untouched, with the reason.
    pub skipped: Vec<SkippedToken>,
    /// Human-readable summary (the tx summary plus theme-apply extras).
    pub human: String,
    /// JSON summary (the tx JSON schema plus theme-apply extras).
    pub json_str: String,
    /// Status-derived exit code, inherited from the underlying transaction.
    pub exit_code: u8,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Resolve `pack_ref` (a bare theme name or a full pack id) against
/// `project_dir`, compute the re-skin transaction against `doc_src`, run it
/// through [`crate::commands::tx::run`], and return an [`ApplyOutcome`].
///
/// This function never touches the filesystem itself (the caller reads
/// `doc_src` and, on `--apply`, persists `result.source_after`).
///
/// When the theme has no encodable changes to offer at all (every theme
/// token already matches, or every candidate op was skipped), the built
/// transaction simply has an empty `ops` list; running it through the tx
/// engine already reports that cleanly (`status: accepted`, `changed: false`,
/// no diagnostics) without any special-casing here.
pub fn run(
    project_dir: Option<&Path>,
    pack_ref: &str,
    doc_src: &str,
) -> Result<ApplyOutcome, ThemeApplyErr> {
    let theme_doc = resolve_theme_pack(project_dir, pack_ref).map_err(|message| ThemeApplyErr {
        message,
        exit_code: 2,
    })?;

    let doc = KdlAdapter
        .parse(doc_src.as_bytes())
        .map_err(|e| ThemeApplyErr {
            message: format!("error[parse.error]: {}", e.message),
            exit_code: 2,
        })?;

    // The resolved pack id (e.g. `@zenith/theme.cobalt`), not the possibly-bare
    // user arg (`pack_ref` may just be `"cobalt"`) — stamped onto every newly
    // created token as `set` provenance. Every theme pack declares its own
    // `project id="..."`; fall back to `pack_ref` only in the (unexpected)
    // case a resolved pack is missing a project block.
    let pack_id = theme_doc
        .project
        .as_ref()
        .map(|p| p.id.clone())
        .unwrap_or_else(|| pack_ref.to_owned());

    let (ops, added_tokens, skipped) =
        plan_ops(&doc.tokens.tokens, &theme_doc.tokens.tokens, &pack_id);

    let tx = Transaction {
        ops,
        permissions: Permissions::default(),
    };
    let tx_json = serde_json::to_string(&tx).map_err(|e| ThemeApplyErr {
        message: format!("internal: failed to serialize theme-apply transaction: {e}"),
        exit_code: 2,
    })?;

    let outcome = tx_run(doc_src, &tx_json).map_err(|e| ThemeApplyErr {
        message: e.message,
        exit_code: e.exit_code,
    })?;

    let human = format!(
        "{}\n{}",
        outcome.human,
        render_extra_human(&added_tokens, &skipped)
    );
    let json_str = augment_json(&outcome.json_str, &added_tokens, &skipped);

    Ok(ApplyOutcome {
        result: outcome.result,
        added_tokens,
        skipped,
        human,
        json_str,
        exit_code: outcome.exit_code,
    })
}

// ── Pre-filter: theme tokens → ops + skips ────────────────────────────────────

/// Build the op list (plus the `added`/`skipped` reports) for re-skinning
/// `doc_tokens` from `theme_tokens`.
///
/// Doc-only tokens (ids in `doc_tokens` absent from `theme_tokens`) are never
/// touched and never appear in either report.
fn plan_ops(
    doc_tokens: &[zenith_core::Token],
    theme_tokens: &[zenith_core::Token],
    pack_id: &str,
) -> (Vec<Op>, Vec<String>, Vec<SkippedToken>) {
    let mut ops = Vec::new();
    let mut added = Vec::new();
    let mut skipped = Vec::new();

    // Index the (small, but potentially large-doc) target once, rather than
    // rescanning `doc_tokens` per theme token.
    let doc_by_id: BTreeMap<&str, &zenith_core::Token> =
        doc_tokens.iter().map(|t| (t.id.as_str(), t)).collect();

    for theme_token in theme_tokens {
        let doc_match = doc_by_id.get(theme_token.id.as_str()).copied();

        // Encodability is checked first: a theme token whose type or value
        // can never become a valid op (regardless of what the doc side looks
        // like) is always a skip, never an op — the engine would reject the
        // whole transaction if we guessed wrong.
        let Some(type_str) = op_token_type_str(&theme_token.token_type) else {
            skipped.push(SkippedToken {
                id: theme_token.id.clone(),
                doc_type: doc_match.map(|t| token_type_label(&t.token_type)),
                theme_type: token_type_label(&theme_token.token_type),
                reason: SkipReason::Unencodable,
            });
            continue;
        };
        let Some(value_str) = encode_value(&theme_token.value) else {
            skipped.push(SkippedToken {
                id: theme_token.id.clone(),
                doc_type: doc_match.map(|t| token_type_label(&t.token_type)),
                theme_type: type_str.to_owned(),
                reason: SkipReason::Unencodable,
            });
            continue;
        };

        match doc_match {
            None => {
                ops.push(Op::CreateToken {
                    id: theme_token.id.clone(),
                    token_type: type_str.to_owned(),
                    value: value_str,
                    set: Some(pack_id.to_owned()),
                });
                added.push(theme_token.id.clone());
            }
            Some(existing) if existing.token_type == theme_token.token_type => {
                ops.push(Op::UpdateTokenValue {
                    id: theme_token.id.clone(),
                    value: value_str,
                    set: Some(pack_id.to_owned()),
                });
            }
            Some(existing) => {
                skipped.push(SkippedToken {
                    id: theme_token.id.clone(),
                    doc_type: Some(token_type_label(&existing.token_type)),
                    theme_type: type_str.to_owned(),
                    reason: SkipReason::TypeMismatch,
                });
            }
        }
    }

    (ops, added, skipped)
}

/// The op-literal `type` string for a [`TokenType`] the tx engine's
/// `create_token`/`update_token_value` ops actually accept — `None` for the
/// structured types (`gradient`, `shadow`, `filter`, `mask`) and for
/// `Unknown`, which those ops always reject (v0: scalar types only).
fn op_token_type_str(ty: &TokenType) -> Option<&'static str> {
    match ty {
        TokenType::Color => Some("color"),
        TokenType::Dimension => Some("dimension"),
        TokenType::Number => Some("number"),
        TokenType::FontFamily => Some("fontFamily"),
        TokenType::FontWeight => Some("fontWeight"),
        TokenType::Gradient
        | TokenType::Shadow
        | TokenType::Filter
        | TokenType::Mask
        | TokenType::Unknown(_) => None,
    }
}

/// A human-readable label for any [`TokenType`], including `Unknown`.
fn token_type_label(ty: &TokenType) -> String {
    match ty {
        TokenType::Color => "color".to_owned(),
        TokenType::Dimension => "dimension".to_owned(),
        TokenType::Number => "number".to_owned(),
        TokenType::FontFamily => "fontFamily".to_owned(),
        TokenType::FontWeight => "fontWeight".to_owned(),
        TokenType::Gradient => "gradient".to_owned(),
        TokenType::Shadow => "shadow".to_owned(),
        TokenType::Filter => "filter".to_owned(),
        TokenType::Mask => "mask".to_owned(),
        TokenType::Unknown(s) => s.clone(),
    }
}

/// Encode a token's declared value as an op literal string, or `None` when it
/// can't be expressed that way.
///
/// A `(token)` alias reference can't be encoded: passing its target id as a
/// plain literal string would silently store the raw reference text as the
/// value instead of actually resolving it, which is not what re-skinning a
/// document means.
fn encode_value(value: &TokenValue) -> Option<String> {
    match value {
        TokenValue::Literal(lit) => encode_literal(lit),
        TokenValue::Reference { token_id: _ } => None,
    }
}

/// Encode a [`TokenLiteral`] as an op literal string, or `None` for the
/// structured variants (gradient/shadow/filter/mask child-node tokens) that
/// have no scalar string form.
fn encode_literal(lit: &TokenLiteral) -> Option<String> {
    match lit {
        TokenLiteral::String(s) => Some(s.clone()),
        TokenLiteral::Dimension(d) => Some(d.to_kdl_string()),
        TokenLiteral::Number(n) => Some(super::support::format_scalar(*n)),
        TokenLiteral::Gradient(_)
        | TokenLiteral::Shadow(_)
        | TokenLiteral::Filter(_)
        | TokenLiteral::Mask(_) => None,
    }
}

// ── Output rendering (extras layered on top of the tx black box) ────────────

/// Render the theme-apply-specific extra lines appended after the tx human
/// summary: the added token ids and the skipped tokens with their reasons.
fn render_extra_human(added: &[String], skipped: &[SkippedToken]) -> String {
    let mut out = String::new();
    if added.is_empty() {
        out.push_str("added tokens: (none)\n");
    } else {
        out.push_str(&format!("added tokens: {}\n", added.join(", ")));
    }
    if skipped.is_empty() {
        out.push_str("skipped: (none)");
    } else {
        out.push_str("skipped:");
        for s in skipped {
            let doc_type = s.doc_type.as_deref().unwrap_or("(absent)");
            out.push_str(&format!(
                "\n  {} (doc: {}, theme: {}) [{}]",
                s.id,
                doc_type,
                s.theme_type,
                s.reason.label()
            ));
        }
    }
    out
}

/// Parse the tx JSON output back into a value and layer the theme-apply
/// extras (`added_tokens`, `skipped_token_mismatches`) onto it, keeping the
/// output a strict superset of the `zenith-tx-v1` schema.
fn augment_json(tx_json_str: &str, added: &[String], skipped: &[SkippedToken]) -> String {
    let mut value: serde_json::Value =
        serde_json::from_str(tx_json_str).unwrap_or_else(|_| serde_json::json!({}));
    if let serde_json::Value::Object(map) = &mut value {
        let added_json = serde_json::to_value(added).unwrap_or(serde_json::Value::Null);
        let skip_json: Vec<crate::json_types::ThemeApplySkipJson> = skipped
            .iter()
            .map(crate::json_types::ThemeApplySkipJson::from)
            .collect();
        let skip_json = serde_json::to_value(skip_json).unwrap_or(serde_json::Value::Null);
        map.insert("added_tokens".to_owned(), added_json);
        map.insert("skipped_token_mismatches".to_owned(), skip_json);
    }
    serde_json::to_string_pretty(&value).unwrap_or_else(|e| e.to_string())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{Dimension, ShadowLiteral, Token, Unit};

    fn token(id: &str, token_type: TokenType, lit: TokenLiteral) -> Token {
        Token {
            id: id.to_owned(),
            token_type,
            value: TokenValue::Literal(lit),
            set: None,
            source_span: None,
        }
    }

    fn ref_token(id: &str, token_type: TokenType, target: &str) -> Token {
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

    /// The pack id used across tests; matches the shape of a resolved embedded
    /// theme pack id.
    const TEST_PACK_ID: &str = "@zenith/theme.test";

    #[test]
    fn same_type_becomes_update() {
        let doc = vec![token(
            "color.primary",
            TokenType::Color,
            TokenLiteral::String("#111111".into()),
        )];
        let theme = vec![token(
            "color.primary",
            TokenType::Color,
            TokenLiteral::String("#222222".into()),
        )];
        let (ops, added, skipped) = plan_ops(&doc, &theme, TEST_PACK_ID);
        assert_eq!(ops.len(), 1);
        assert!(matches!(
            &ops[0],
            Op::UpdateTokenValue { id, value, set }
                if id == "color.primary" && value == "#222222"
                    && set.as_deref() == Some(TEST_PACK_ID)
        ));
        assert!(added.is_empty());
        assert!(skipped.is_empty());
    }

    #[test]
    fn absent_id_becomes_create() {
        let doc: Vec<Token> = vec![];
        let theme = vec![token(
            "color.new",
            TokenType::Color,
            TokenLiteral::String("#333333".into()),
        )];
        let (ops, added, skipped) = plan_ops(&doc, &theme, TEST_PACK_ID);
        assert_eq!(ops.len(), 1);
        assert!(matches!(
            &ops[0],
            Op::CreateToken { id, token_type, value, set }
                if id == "color.new" && token_type == "color" && value == "#333333"
                    && set.as_deref() == Some(TEST_PACK_ID)
        ));
        assert_eq!(added, vec!["color.new".to_string()]);
        assert!(skipped.is_empty());
    }

    #[test]
    fn type_mismatch_is_skipped_not_emitted() {
        let doc = vec![token(
            "space.unit",
            TokenType::Color,
            TokenLiteral::String("#000000".into()),
        )];
        let theme = vec![token(
            "space.unit",
            TokenType::Dimension,
            TokenLiteral::Dimension(Dimension {
                value: 4.0,
                unit: Unit::Px,
            }),
        )];
        let (ops, added, skipped) = plan_ops(&doc, &theme, TEST_PACK_ID);
        assert!(ops.is_empty());
        assert!(added.is_empty());
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].reason, SkipReason::TypeMismatch);
        assert_eq!(skipped[0].doc_type.as_deref(), Some("color"));
        assert_eq!(skipped[0].theme_type, "dimension");
    }

    #[test]
    fn structured_literal_is_unencodable() {
        let doc: Vec<Token> = vec![];
        let theme = vec![Token {
            id: "shadow.depth".to_owned(),
            token_type: TokenType::Shadow,
            value: TokenValue::Literal(TokenLiteral::Shadow(ShadowLiteral { layers: vec![] })),
            set: None,
            source_span: None,
        }];
        let (ops, added, skipped) = plan_ops(&doc, &theme, TEST_PACK_ID);
        assert!(ops.is_empty());
        assert!(added.is_empty());
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].reason, SkipReason::Unencodable);
    }

    #[test]
    fn alias_reference_is_unencodable() {
        let doc: Vec<Token> = vec![];
        let theme = vec![ref_token("color.alias", TokenType::Color, "color.primary")];
        let (ops, added, skipped) = plan_ops(&doc, &theme, TEST_PACK_ID);
        assert!(ops.is_empty());
        assert!(added.is_empty());
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].reason, SkipReason::Unencodable);
    }

    #[test]
    fn doc_only_token_is_left_alone() {
        let doc = vec![token(
            "color.extra",
            TokenType::Color,
            TokenLiteral::String("#abcabc".into()),
        )];
        let theme: Vec<Token> = vec![];
        let (ops, added, skipped) = plan_ops(&doc, &theme, TEST_PACK_ID);
        assert!(ops.is_empty());
        assert!(added.is_empty());
        assert!(skipped.is_empty());
    }

    #[test]
    fn dimension_encodes_without_trailing_zero() {
        let lit = TokenLiteral::Dimension(Dimension {
            value: 16.0,
            unit: Unit::Px,
        });
        assert_eq!(encode_literal(&lit), Some("(px)16".to_owned()));
    }
}
