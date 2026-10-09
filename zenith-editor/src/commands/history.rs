//! `history.undo` and `history.redo`.

use serde_json::Value;

use crate::ctx::Ctx;
use crate::edit::text::{EditFacts, Record, change_text, edit_reply};
use crate::error::EditorError;
use crate::session::HistoryEntry;

/// Revert the last undo entry: apply its inverse delta, restore the
/// selection before it, and move it to the redo stack. The reply's delta
/// is the exact inverse, so undo then redo gives the original text back
/// byte for byte.
pub(crate) fn undo(ctx: &mut Ctx<'_, '_>, _raw: Value) -> Result<Value, EditorError> {
    let entry =
        ctx.session.history.undo.last().cloned().ok_or_else(|| {
            EditorError::new("editor.nothing_to_undo", "the undo history is empty")
        })?;
    let text = entry.inverse.apply(&ctx.session.text)?;
    ctx.session.history.undo.pop();
    let selection = entry.selection_before.clone();
    step(ctx, entry, text, selection, Direction::Undo)
}

/// Re-apply the last undone entry and restore the selection after it.
pub(crate) fn redo(ctx: &mut Ctx<'_, '_>, _raw: Value) -> Result<Value, EditorError> {
    let entry =
        ctx.session.history.redo.last().cloned().ok_or_else(|| {
            EditorError::new("editor.nothing_to_redo", "the redo history is empty")
        })?;
    let text = entry.forward.apply(&ctx.session.text)?;
    ctx.session.history.redo.pop();
    let selection = entry.selection_after.clone();
    step(ctx, entry, text, selection, Direction::Redo)
}

enum Direction {
    Undo,
    Redo,
}

fn step(
    ctx: &mut Ctx<'_, '_>,
    entry: HistoryEntry,
    text: String,
    selection: Vec<String>,
    direction: Direction,
) -> Result<Value, EditorError> {
    let changed = change_text(ctx, text, Record::Skip, selection)?;
    match direction {
        Direction::Undo => ctx.session.history.redo.push(entry),
        Direction::Redo => ctx.session.history.undo.push(entry),
    }
    // Typing after an undo or redo starts a new entry.
    if let Some(last) = ctx.session.history.undo.last_mut() {
        last.typing = false;
    }
    ctx.session.history.enforce_limit();
    edit_reply(ctx, changed, EditFacts::default())
}
