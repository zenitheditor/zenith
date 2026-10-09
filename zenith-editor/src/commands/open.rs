//! `doc.open {text}`: start a fresh session over `text`.

use serde::Deserialize;
use serde_json::{Value, json};

use super::common::params;
use crate::ctx::Ctx;
use crate::error::EditorError;
use crate::session::Session;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenParams {
    text: String,
}

/// Replace the session with a fresh one over `text`: empty history and
/// selection, page 1, default viewport, the history limit kept. The
/// version keeps counting up, so a reply to the old text is still stale.
/// The text is validated.
pub(crate) fn run(ctx: &mut Ctx<'_, '_>, raw: Value) -> Result<Value, EditorError> {
    let p: OpenParams = params(ctx, raw)?;
    let mut session = Session::new(p.text);
    session.version = ctx.session.version + 1;
    session.history.limit = ctx.session.history.limit;
    ctx.session = session;
    let text = ctx.session.text.clone();
    let checked = ctx.validate(&text);
    ctx.session.set_valid(checked.valid());
    let page_count = ctx.parse(&text).ok().map(|doc| doc.body.pages.len());
    Ok(json!({
        "version": ctx.session.version,
        "valid": ctx.session.valid,
        "stale": ctx.session.stale(),
        "page_count": page_count,
        "diagnostics": checked.diagnostics(&text),
    }))
}
