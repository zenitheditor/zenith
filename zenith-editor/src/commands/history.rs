//! `history.undo`, `history.redo`, `history.list`, and `history.goto`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::commands::common::params;
use crate::ctx::Ctx;
use crate::edit::text::{EditFacts, Record, change_text, edit_reply};
use crate::error::EditorError;
use crate::session::HistoryEntry;
use crate::wire::to_json;

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

#[derive(Debug, Serialize)]
struct HistoryItemOut {
    index: usize,
    label: String,
    current: bool,
    ahead: bool,
    selection: Vec<String>,
}

#[derive(Debug, Serialize)]
struct HistoryListOut {
    index: usize,
    count: usize,
    can_undo: bool,
    can_redo: bool,
    entries: Vec<HistoryItemOut>,
}

/// List every step in the session's undo/redo timeline in chronological
/// order, starting with step `0` (the opened base state).
pub(crate) fn list(ctx: &mut Ctx<'_, '_>, _raw: Value) -> Result<Value, EditorError> {
    let undo = &ctx.session.history.undo;
    let redo = &ctx.session.history.redo;
    let current_index = undo.len();
    let count = undo.len() + redo.len();
    let mut entries = Vec::with_capacity(count + 1);
    entries.push(HistoryItemOut {
        index: 0,
        label: "doc.open".to_owned(),
        current: current_index == 0,
        ahead: false,
        selection: Vec::new(),
    });
    for (i, entry) in undo.iter().enumerate() {
        let index = i + 1;
        entries.push(HistoryItemOut {
            index,
            label: entry.label.clone(),
            current: index == current_index,
            ahead: false,
            selection: entry.selection_after.clone(),
        });
    }
    for (j, entry) in redo.iter().rev().enumerate() {
        let index = current_index + 1 + j;
        entries.push(HistoryItemOut {
            index,
            label: entry.label.clone(),
            current: false,
            ahead: true,
            selection: entry.selection_after.clone(),
        });
    }
    to_json(&HistoryListOut {
        index: current_index,
        count,
        can_undo: !undo.is_empty(),
        can_redo: !redo.is_empty(),
        entries,
    })
}

#[derive(Debug, Deserialize)]
struct GotoParams {
    index: usize,
}

/// Jump directly to `index` in the history timeline (`0..=count`),
/// applying the combined inverse or forward deltas as one session edit.
pub(crate) fn goto(ctx: &mut Ctx<'_, '_>, raw: Value) -> Result<Value, EditorError> {
    let p: GotoParams = params(ctx, raw)?;
    let current = ctx.session.history.undo.len();
    let total = current + ctx.session.history.redo.len();
    if p.index > total {
        return Err(EditorError::new(
            "editor.history_out_of_range",
            format!(
                "history index {} out of range; valid range is 0..={total}",
                p.index
            ),
        ));
    }
    if p.index == current {
        let changed = change_text(
            ctx,
            ctx.session.text.clone(),
            Record::Skip,
            ctx.session.selection.clone(),
        )?;
        return edit_reply(ctx, changed, EditFacts::default());
    }
    let mut text = ctx.session.text.clone();
    let mut selection = ctx.session.selection.clone();
    if p.index < current {
        let steps = current - p.index;
        for _ in 0..steps {
            let entry = ctx.session.history.undo.pop().ok_or_else(|| {
                EditorError::new("editor.history_mismatch", "undo stack underflow")
            })?;
            text = entry.inverse.apply(&text)?;
            selection = entry.selection_before.clone();
            ctx.session.history.redo.push(entry);
        }
    } else {
        let steps = p.index - current;
        for _ in 0..steps {
            let entry = ctx.session.history.redo.pop().ok_or_else(|| {
                EditorError::new("editor.history_mismatch", "redo stack underflow")
            })?;
            text = entry.forward.apply(&text)?;
            selection = entry.selection_after.clone();
            ctx.session.history.undo.push(entry);
        }
    }
    let changed = change_text(ctx, text, Record::Skip, selection)?;
    if let Some(last) = ctx.session.history.undo.last_mut() {
        last.typing = false;
    }
    ctx.session.history.enforce_limit();
    edit_reply(ctx, changed, EditFacts::default())
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
