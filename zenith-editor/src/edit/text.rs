//! [`change_text`]: the one path every text change takes. It records the
//! history entry, bumps the version, validates the new text, and keeps the
//! selection on ids that still exist.

use serde::Serialize;
use serde_json::Value;
use zenith_tx::Op;

use super::comments::removed_comments;
use crate::ctx::Ctx;
use crate::doc::tree::exists;
use crate::error::EditorError;
use crate::session::{HistoryEntry, TextDelta};
use crate::wire::{DeltaOut, DiagnosticOut, to_json};

/// How a text change enters the history.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Record<'l> {
    /// A `buffer.set` typing burst; `coalesce` lets it extend the last one.
    Typing { coalesce: bool },
    /// An engine edit, labelled with its command.
    Edit { label: &'l str },
    /// Undo or redo: the history stacks move instead.
    Skip,
}

/// The result of [`change_text`].
pub(crate) struct Changed {
    /// The text before the change.
    pub(crate) before: String,
    /// The change, `None` when the text did not change.
    pub(crate) delta: Option<TextDelta>,
    /// The diagnostics of the new text (empty when unchanged).
    pub(crate) diagnostics: Vec<DiagnosticOut>,
}

/// Replace the session text with `text`.
///
/// Unchanged text records nothing and keeps the version; the selection
/// still becomes `selection`. Otherwise the change is recorded per
/// `record`, the version is bumped, the new text is validated (setting
/// `valid` and the last valid text), and the selection keeps the ids of
/// `selection` the new text still has.
///
/// # Errors
///
/// `editor.history_mismatch` when the history does not fit the text.
pub(crate) fn change_text(
    ctx: &mut Ctx<'_, '_>,
    text: String,
    record: Record<'_>,
    selection: Vec<String>,
) -> Result<Changed, EditorError> {
    let before = ctx.session.text.clone();
    let Some(delta) = TextDelta::between(&before, &text) else {
        ctx.session.selection = selection;
        return Ok(Changed {
            before,
            delta: None,
            diagnostics: Vec::new(),
        });
    };
    let (label, typing, coalesce) = match record {
        Record::Typing { coalesce } => (Some("buffer.set"), true, coalesce),
        Record::Edit { label } => (Some(label), false, false),
        Record::Skip => (None, false, false),
    };
    if let Some(label) = label {
        let entry = HistoryEntry {
            label: label.to_owned(),
            typing,
            forward: delta.clone(),
            inverse: delta.inverse(&before)?,
            selection_before: ctx.session.selection.clone(),
            selection_after: selection.clone(),
        };
        ctx.session
            .history
            .record(entry, &before, &text, coalesce)?;
    }
    ctx.session.replace_text(text);
    let current = ctx.session.text.clone();
    let checked = ctx.validate(&current);
    ctx.session.set_valid(checked.valid());
    ctx.session.selection = match ctx.parse(&current) {
        Ok(doc) => selection
            .into_iter()
            .filter(|id| exists(&doc, id))
            .collect(),
        Err(_) => selection,
    };
    Ok(Changed {
        before,
        delta: Some(delta),
        diagnostics: checked.diagnostics(&current),
    })
}

/// The result of every text-changing command except `buffer.set`.
#[derive(Debug, Serialize)]
pub(crate) struct EditReply {
    /// `true` when the text changed.
    pub(crate) changed: bool,
    /// The session version after the command.
    pub(crate) version: u64,
    /// The change for the page's editor; absent when unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) delta: Option<DeltaOut>,
    /// `true` when the patcher fell back to canonical text: the source's
    /// comments and layout are gone.
    pub(crate) reformatted: bool,
    /// `//` comment lines the change dropped (a removed node takes the
    /// comments directly above it).
    pub(crate) removed_comments: Vec<String>,
    /// The ops that ran.
    pub(crate) ops: Vec<Op>,
    /// The selection after the command.
    pub(crate) selection: Vec<String>,
    /// `true` when the new text has no Error diagnostic.
    pub(crate) valid: bool,
    /// `true` when render commands now use the last valid text.
    pub(crate) stale: bool,
    /// The diagnostics of the new text.
    pub(crate) diagnostics: Vec<DiagnosticOut>,
    /// Warning and advisory diagnostics from the transaction.
    pub(crate) tx_diagnostics: Vec<DiagnosticOut>,
    /// Notes on how a gesture was mapped, for example an axis the anchor
    /// keeps.
    pub(crate) notes: Vec<DiagnosticOut>,
}

/// The parts of an [`EditReply`] the caller supplies.
#[derive(Default)]
pub(crate) struct EditFacts {
    pub(crate) reformatted: bool,
    pub(crate) ops: Vec<Op>,
    pub(crate) tx_diagnostics: Vec<DiagnosticOut>,
    pub(crate) notes: Vec<DiagnosticOut>,
}

/// The [`EditReply`] of `changed` as JSON.
///
/// # Errors
///
/// `editor.encode_failed` when the reply does not encode.
pub(crate) fn edit_reply(
    ctx: &Ctx<'_, '_>,
    changed: Changed,
    facts: EditFacts,
) -> Result<Value, EditorError> {
    let reply = EditReply {
        changed: changed.delta.is_some(),
        version: ctx.session.version,
        delta: changed
            .delta
            .as_ref()
            .map(|d| DeltaOut::new(d, &changed.before)),
        reformatted: facts.reformatted && changed.delta.is_some(),
        removed_comments: changed
            .delta
            .as_ref()
            .map(|d| removed_comments(&changed.before, d))
            .unwrap_or_default(),
        ops: facts.ops,
        selection: ctx.session.selection.clone(),
        valid: ctx.session.valid,
        stale: ctx.session.stale(),
        diagnostics: changed.diagnostics,
        tx_diagnostics: facts.tx_diagnostics,
        notes: facts.notes,
    };
    to_json(&reply)
}
