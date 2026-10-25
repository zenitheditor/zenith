//! [`print_preflight`]: the print checks of one compiled page.
//!
//! `font.not_embeddable` runs on every page, because any page can export
//! to PDF. The resolution and ink checks run only on a print page: a page
//! with a positive `bleed`, or any page of a `colorspace="cmyk"` document.
//! A screen design reports nothing new.

use std::collections::BTreeMap;

use zenith_core::Diagnostic;

use super::super::super::boxes::CompiledBox;
use super::super::ledger::PageLedger;
use super::super::run::LintEnv;
use super::font::not_embeddable;
use super::ink::ink;
use super::owner::Owners;
use super::ppi::effective_ppi;

/// What the print checks read from one page lint.
pub(in crate::compile::lint) struct PrintInput<'a, 'e> {
    pub(in crate::compile::lint) env: &'a LintEnv<'e>,
    pub(in crate::compile::lint) ledger: &'a PageLedger,
    /// The final box of every compiled node, by id.
    pub(in crate::compile::lint) boxes: &'a BTreeMap<String, CompiledBox>,
}

/// The print diagnostics of one page.
pub(in crate::compile::lint) fn print_preflight(input: &PrintInput<'_, '_>) -> Vec<Diagnostic> {
    let owners = Owners::build(input.ledger, input.boxes, input.env.commands.len());
    let mut out = not_embeddable(input, &owners);
    if input.env.print {
        out.extend(effective_ppi(input));
        out.extend(ink(input, &owners));
    }
    out
}
