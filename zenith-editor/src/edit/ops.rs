//! [`apply_ops`]: run a transaction on the current text and write the
//! result back with the source patcher.

use serde_json::Value;
use zenith_core::{Diagnostic, Document, patch_source};
use zenith_tx::{Op, Permissions, Transaction, TxStatus};

use super::offers::for_rejection;
use super::text::{EditFacts, Record, change_text, edit_reply};
use crate::ctx::Ctx;
use crate::error::{EditorError, Offer};
use crate::wire::DiagnosticOut;

/// One transaction to run on the current text.
pub(crate) struct OpsEdit {
    /// The history label. `None` uses the command id.
    pub(crate) label: Option<String>,
    /// The ops, in order.
    pub(crate) ops: Vec<Op>,
    /// Guard relaxations.
    pub(crate) permissions: Permissions,
    /// The selection after the edit. `None` keeps the current one.
    pub(crate) selection: Option<Vec<String>>,
    /// Notes for the reply.
    pub(crate) notes: Vec<DiagnosticOut>,
    /// `true` when a rejection may offer gesture flags.
    pub(crate) gesture: bool,
    /// The raw params, for offers.
    pub(crate) raw: Value,
}

/// Run `edit` on `doc`, the parse of the session text, and patch the
/// result into the text.
///
/// The patcher keeps comments and layout; when it cannot, the text is the
/// canonical form and the reply says `reformatted: true`.
///
/// # Errors
///
/// `editor.rejected` with the transaction's diagnostics and offers when
/// any op fails; `editor.patch_failed` when the result has no text.
pub(crate) fn apply_ops(
    ctx: &mut Ctx<'_, '_>,
    doc: &Document,
    edit: OpsEdit,
) -> Result<Value, EditorError> {
    let tx = Transaction {
        ops: edit.ops,
        permissions: edit.permissions,
    };
    let result = ctx.run_tx(doc, &tx)?;
    let text = ctx.session.text.clone();
    if result.status == TxStatus::Rejected {
        let offers = for_rejection(ctx.command, &edit.raw, &result.diagnostics, edit.gesture);
        return Err(rejected(ctx.command, &result.diagnostics, &text, offers));
    }
    ctx.work.patches += 1;
    let patched = patch_source(&text, doc, &result.document_after).map_err(|e| {
        EditorError::new(
            "editor.patch_failed",
            format!("{e}; the edit produced a document with no text, so nothing changed"),
        )
    })?;
    let selection = edit
        .selection
        .unwrap_or_else(|| ctx.session.selection.clone());
    let label = edit.label.unwrap_or_else(|| ctx.command.to_owned());
    let changed = change_text(ctx, patched.text, Record::Edit { label: &label }, selection)?;
    let tx_diagnostics: Vec<Diagnostic> = result
        .diagnostics
        .into_iter()
        .filter(|d| !d.is_error())
        .collect();
    edit_reply(
        ctx,
        changed,
        EditFacts {
            reformatted: patched.reformatted,
            ops: tx.ops,
            tx_diagnostics: DiagnosticOut::all(&tx_diagnostics, &text),
            notes: edit.notes,
        },
    )
}

/// `editor.rejected` for `command`, carrying `diagnostics` and `offers`.
pub(crate) fn rejected(
    command: &str,
    diagnostics: &[Diagnostic],
    src: &str,
    offers: Vec<Offer>,
) -> EditorError {
    let errors: Vec<&Diagnostic> = diagnostics.iter().filter(|d| d.is_error()).collect();
    let codes: Vec<&str> = errors.iter().map(|d| d.code.as_str()).collect();
    let first = errors.first().map_or("", |d| d.message.as_str());
    let next = if offers.is_empty() {
        "change the request and retry"
    } else {
        "send one of the offers, or change the request"
    };
    EditorError::new(
        "editor.rejected",
        format!(
            "'{command}' was rejected ({}): {first}; {next}",
            codes.join(", ")
        ),
    )
    .with_diagnostics(diagnostics, src)
    .with_offers(offers)
}
