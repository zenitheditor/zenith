//! `buffer.set {text, coalesce?}`: the page's code editor changed the text.

use serde::Deserialize;
use serde_json::{Value, json};

use super::common::params;
use crate::ctx::Ctx;
use crate::edit::text::{Record, change_text};
use crate::error::EditorError;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BufferParams {
    text: String,
    #[serde(default = "yes")]
    coalesce: bool,
}

fn yes() -> bool {
    true
}

/// Take the page's text as the session text.
///
/// The request must carry the version the page's buffer started from, so
/// a buffer made before an engine edit is rejected with
/// `editor.stale_version` instead of overwriting it. With `coalesce`
/// (default) the change is typing: it extends the last typing entry when
/// contiguous. Without it (a reload from disk) the change is its own
/// history entry, a boundary that no later typing extends, so one undo
/// never reverts a reload and a keystroke together. The text is
/// validated: while it has errors, the session keeps the last valid text
/// for rendering.
///
/// The reply has no delta: the page already holds the text.
pub(crate) fn run(ctx: &mut Ctx<'_, '_>, raw: Value) -> Result<Value, EditorError> {
    let p: BufferParams = params(ctx, raw)?;
    let selection = ctx.session.selection.clone();
    let changed = change_text(
        ctx,
        p.text,
        Record::Typing {
            coalesce: p.coalesce,
        },
        selection,
    )?;
    Ok(json!({
        "changed": changed.delta.is_some(),
        "version": ctx.session.version,
        "valid": ctx.session.valid,
        "stale": ctx.session.stale(),
        "selection": ctx.session.selection,
        "diagnostics": changed.diagnostics,
    }))
}
