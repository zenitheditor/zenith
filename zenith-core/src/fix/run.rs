//! The `zenith fix` driver: validate, plan, apply, format, repeat.

use std::collections::BTreeSet;

use kdl::KdlDocument;

use crate::ast::Document;
use crate::diagnostics::Diagnostic;
use crate::error::ParseError;
use crate::parse::{KdlAdapter, KdlSource};
use crate::validate::validate;

use super::apply::apply;
use super::mint::MintedToken;
use super::plan::{AppliedFix, plan};

/// Passes before the driver stops. A rename can expose a raw literal, so one
/// pass is not always enough.
const MAX_PASSES: usize = 3;

/// The result of [`fix_source`].
#[derive(Debug, Clone, PartialEq)]
pub struct FixOutcome {
    /// The input source.
    pub source_before: String,
    /// The fixed source in canonical (`zenith fmt`) form. Equals
    /// `source_before` when no fix applied.
    pub source_after: String,
    /// Every applied fix, in pass order.
    pub applied: Vec<AppliedFix>,
    /// Every token the fixes created, in pass order.
    pub minted: Vec<MintedToken>,
    /// Core validation diagnostics of `source_after` (no compile-stage
    /// checks; the CLI adds those).
    pub remaining: Vec<Diagnostic>,
}

impl FixOutcome {
    /// `true` when the fixes changed the source.
    pub fn changed(&self) -> bool {
        self.source_before != self.source_after
    }
}

/// Apply every machine-fixable diagnostic in `src`, up to a fixpoint.
///
/// Pure and deterministic: the same `src` gives the same bytes. Each pass
/// validates, plans fixes, edits the KDL, and formats the result
/// canonically. The driver stops when a pass applies nothing, after
/// three passes, or when a pass result fails to parse (that pass
/// is dropped).
///
/// # Errors
///
/// Returns the [`ParseError`] when `src` does not parse.
pub fn fix_source(src: &str) -> Result<FixOutcome, ParseError> {
    let mut doc = KdlAdapter.parse(src.as_bytes())?;
    let mut current = src.to_owned();
    let mut applied: Vec<AppliedFix> = Vec::new();
    let mut minted: Vec<MintedToken> = Vec::new();

    for _ in 0..MAX_PASSES {
        let Some(pass) = run_pass(&doc, &current) else {
            break;
        };
        doc = pass.doc;
        current = pass.source;
        applied.extend(pass.applied);
        minted.extend(pass.minted);
    }

    let source_after = if applied.is_empty() {
        src.to_owned()
    } else {
        current
    };
    Ok(FixOutcome {
        source_before: src.to_owned(),
        source_after,
        applied,
        minted,
        remaining: validate(&doc).diagnostics,
    })
}

struct Pass {
    doc: Document,
    source: String,
    applied: Vec<AppliedFix>,
    minted: Vec<MintedToken>,
}

/// One pass over `doc` (parsed from `source`). `None` when nothing applies
/// or the result does not parse.
fn run_pass(doc: &Document, source: &str) -> Option<Pass> {
    let diagnostics = validate(doc).diagnostics;
    let declared: BTreeSet<String> = doc.tokens.tokens.iter().map(|t| t.id.clone()).collect();

    let mut kdl: KdlDocument = source.parse().ok()?;
    let plan = plan(&kdl, &diagnostics, declared);
    let minted = plan.minted.clone();
    let applied = apply(&mut kdl, plan);
    if applied.is_empty() {
        return None;
    }
    kdl.autoformat();
    let edited = kdl.to_string();
    let next = KdlAdapter.parse(edited.as_bytes()).ok()?;
    let formatted = String::from_utf8(KdlAdapter.format(&next).ok()?).ok()?;
    if formatted == source {
        return None;
    }
    let doc = KdlAdapter.parse(formatted.as_bytes()).ok()?;
    Some(Pass {
        doc,
        source: formatted,
        applied,
        minted,
    })
}
