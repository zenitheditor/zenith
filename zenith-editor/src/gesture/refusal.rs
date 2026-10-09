//! [`Refusal`]: why one node of a gesture cannot take it, and the merge of
//! several refusals into the one error a gesture replies with.

use serde_json::Value;
use zenith_core::Diagnostic;

use crate::ctx::Ctx;
use crate::edit::offers::for_rejection;
use crate::edit::ops::rejected;
use crate::error::{EditorError, Offer};

/// Why one node refuses a gesture.
#[derive(Debug)]
pub(crate) enum Refusal {
    /// The transaction policy rejects it: `tx.*` diagnostics, answered by
    /// the offers [`for_rejection`] builds (detach, set size, …).
    Rejected(Vec<Diagnostic>),
    /// Anything else (locked, hidden, unsupported, …), with its own offers.
    Error(EditorError),
}

impl From<EditorError> for Refusal {
    fn from(e: EditorError) -> Self {
        Refusal::Error(e)
    }
}

/// The error a gesture replies with when `refusals` (one per refusing
/// node, in node order) stop it. `raw` is the request params, for offers.
///
/// One refusal gives exactly the error a single-node gesture gives. Several
/// give one error: the first [`Refusal::Error`] sets the code and message
/// (else `editor.rejected`); the diagnostics of every rejected node are
/// listed; the offers are those of every refusal, each once, in order. An
/// offer resends the whole gesture with its flag, so it covers every node.
pub(crate) fn refuse(ctx: &Ctx<'_, '_>, raw: &Value, refusals: Vec<Refusal>) -> EditorError {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let mut errors: Vec<EditorError> = Vec::new();
    for refusal in refusals {
        match refusal {
            Refusal::Rejected(d) => diagnostics.extend(d),
            Refusal::Error(e) => errors.push(e),
        }
    }
    let flag_offers = for_rejection(ctx.command, raw, &diagnostics, true);
    let mut errors = errors.into_iter();
    let Some(first) = errors.next() else {
        return rejected(ctx.command, &diagnostics, &ctx.session.text, flag_offers);
    };
    let mut offers: Vec<Offer> = Vec::new();
    let mut push = |o: Offer| {
        if !offers.iter().any(|x| x.id == o.id && x.params == o.params) {
            offers.push(o);
        }
    };
    let rest: Vec<EditorError> = errors.collect();
    for o in first
        .offers
        .iter()
        .chain(rest.iter().flat_map(|e| e.offers.iter()))
        .chain(flag_offers.iter())
    {
        push(o.clone());
    }
    let mut out = first;
    if !diagnostics.is_empty() {
        let mut located = crate::wire::DiagnosticOut::all(&diagnostics, &ctx.session.text);
        out.diagnostics.append(&mut located);
    }
    out.offers = offers;
    out
}
