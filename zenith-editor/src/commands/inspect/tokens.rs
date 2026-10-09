//! `doc.tokens {type?}`: the document's resolved tokens, for pickers.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use zenith_core::resolve_tokens;

use super::attrs::{resolved_json, type_name};
use crate::commands::common::params;
use crate::ctx::Ctx;
use crate::error::EditorError;
use crate::wire::to_json;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TokensParams {
    /// Only tokens of this type (`color`, `dimension`, …).
    #[serde(default, rename = "type")]
    ty: Option<String>,
}

#[derive(Debug, Serialize)]
struct TokenOut {
    id: String,
    r#type: String,
    value: Value,
}

#[derive(Debug, Serialize)]
struct Tokens {
    stale: bool,
    tokens: Vec<TokenOut>,
}

/// Every token that resolves, sorted by id, with its type and resolved
/// value (`type` keeps one type). Read from the display text: while the
/// text has errors, from the last valid text (`stale: true`).
pub(crate) fn run(ctx: &mut Ctx<'_, '_>, raw: Value) -> Result<Value, EditorError> {
    let p: TokensParams = params(ctx, raw)?;
    let display = ctx.display()?;
    let tokens = resolve_tokens(&display.doc.tokens)
        .resolved
        .into_iter()
        .map(|(id, t)| TokenOut {
            id,
            r#type: type_name(&t.token_type),
            value: resolved_json(&t.value),
        })
        .filter(|t| p.ty.as_deref().is_none_or(|ty| ty == t.r#type))
        .collect();
    to_json(&Tokens {
        stale: display.stale,
        tokens,
    })
}
