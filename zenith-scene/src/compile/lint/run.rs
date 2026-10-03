//! [`lint_page`]: the page lint the page compile runs when it reports
//! diagnostics.

use zenith_core::{Diagnostic, Node, Page};

use crate::ir::SceneCommand;

use super::super::boxes::{BoxRecorder, glyph_inks};
use super::super::text::ShapeEnv;
use super::contrast::content_contrast;
use super::label_overflow::label_overflow;
use super::ledger::{LedgerInput, PageLedger};
use super::overlap::{ink_overlap, occluded};
use super::paint::{PaintEnv, authored_facts};

/// What one compiled page hands the lint.
pub(in crate::compile) struct LintEnv<'a> {
    /// The compiled (lowered) page.
    pub(in crate::compile) page: &'a Page,
    /// The same page in the prepared, unlowered document.
    pub(in crate::compile) authored: Option<&'a Page>,
    /// The lowered master projection, ids prefixed with `<page-id>/`.
    pub(in crate::compile) master: &'a [Node],
    /// The finished command stream.
    pub(in crate::compile) commands: &'a [SceneCommand],
    /// Scene offset of the trim box.
    pub(in crate::compile) bleed: f64,
    pub(in crate::compile) paint: PaintEnv<'a>,
    pub(in crate::compile) shape: ShapeEnv<'a>,
    /// The page's diagnostics so far.
    pub(in crate::compile) compiled: &'a [Diagnostic],
}

/// The lint diagnostics of one page: `text.ink_overlap`, `text.occluded`,
/// `label.overflow`, label contrast, and the text contrast of expanded
/// content. `recorder` is the page compile's box recorder.
pub(in crate::compile) fn lint_page(env: &LintEnv<'_>, recorder: BoxRecorder) -> Vec<Diagnostic> {
    let (boxes, expansions) = recorder.into_parts();
    let authored = env
        .authored
        .map(|p| authored_facts(&p.children))
        .unwrap_or_default();
    let inks = glyph_inks(env.commands, (env.bleed, env.bleed), env.shape);
    let ledger = PageLedger::build(
        &LedgerInput {
            master: env.master,
            children: &env.page.children,
            boxes: &boxes,
            expansions: &expansions,
            authored: &authored,
            paint: env.paint,
        },
        inks,
    );
    let mut out = ink_overlap(&ledger, &authored);
    out.extend(occluded(&ledger));
    out.extend(label_overflow(&ledger, env.compiled));
    out.extend(content_contrast(env, &expansions));
    out
}
