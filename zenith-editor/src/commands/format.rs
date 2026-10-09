//! `doc.format`: rewrite the text in canonical form.

use serde_json::Value;
use zenith_core::{KdlAdapter, KdlSource};

use crate::ctx::Ctx;
use crate::edit::text::{EditFacts, Record, change_text, edit_reply};
use crate::error::EditorError;

/// Replace the text with the canonical formatting of its parse, as
/// `zenith fmt` writes it. Comments are not kept (`reformatted: true`), so
/// `removed_comments` lists them. Needs a text that parses; errors from
/// validation do not block formatting.
pub(crate) fn run(ctx: &mut Ctx<'_, '_>, _raw: Value) -> Result<Value, EditorError> {
    let text = ctx.session.text.clone();
    let doc = ctx.parse(&text).map_err(|d| {
        EditorError::new(
            "editor.buffer_invalid",
            format!(
                "the text does not parse ({}); fix it before formatting",
                d.message
            ),
        )
    })?;
    let bytes = KdlAdapter.format(&doc).map_err(|e| {
        EditorError::new(
            "editor.format_failed",
            format!("{e}; the text is unchanged"),
        )
    })?;
    let canonical = String::from_utf8(bytes).map_err(|e| {
        EditorError::new(
            "editor.format_failed",
            format!("{e}; the text is unchanged"),
        )
    })?;
    let selection = ctx.session.selection.clone();
    let label = ctx.command;
    let changed = change_text(ctx, canonical, Record::Edit { label }, selection)?;
    edit_reply(
        ctx,
        changed,
        EditFacts {
            reformatted: true,
            ..EditFacts::default()
        },
    )
}
