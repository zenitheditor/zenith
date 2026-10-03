//! [`lint_page`]: the page lint the page compile runs when it reports
//! diagnostics.

use zenith_core::{Diagnostic, Node, Page, dim_to_px};

use crate::ir::SceneCommand;
use crate::layout::LayoutBox;

use super::super::boxes::{BoxRecorder, Recorded, glyph_inks};
use super::super::imports::ImportScopes;
use super::super::text::ShapeEnv;
use super::arrange::arrangement;
use super::blocks::block_overlap;
use super::chart_overflow::chart_overflow;
use super::contrast::{content_contrast, text_inks};
use super::label_overflow::label_overflow;
use super::ledger::{LedgerInput, PageLedger};
use super::legibility::legibility;
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
    /// The import scopes, for expanded imported components.
    pub(in crate::compile) imports: &'a ImportScopes<'a>,
    pub(in crate::compile) shape: ShapeEnv<'a>,
    /// The page's diagnostics so far.
    pub(in crate::compile) compiled: &'a [Diagnostic],
    /// The page or document declares book margins (a live area).
    pub(in crate::compile) margins_declared: bool,
}

/// The lint diagnostics of one page: `text.ink_overlap`, `text.occluded`,
/// `label.overflow`, the contrast of every text and label, legibility,
/// arrangement, `layout.block_overlap`, and `chart.overflow`. `recorder` is
/// the page compile's box recorder.
pub(in crate::compile) fn lint_page(env: &LintEnv<'_>, recorder: BoxRecorder) -> Vec<Diagnostic> {
    let Recorded {
        boxes,
        expansions,
        routes,
    } = recorder.into_parts();
    let authored = env
        .authored
        .map(|p| authored_facts(&p.children))
        .unwrap_or_default();
    let inks = glyph_inks(env.commands, (env.bleed, env.bleed), env.shape);
    let texts = text_inks(&inks);
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
    out.extend(content_contrast(env, &expansions, &texts));
    out.extend(legibility(env, &ledger, &authored));
    out.extend(arrangement(&ledger, &authored, &routes));
    let trim = match (
        dim_to_px(env.page.width.value, &env.page.width.unit),
        dim_to_px(env.page.height.value, &env.page.height.unit),
    ) {
        (Some(w), Some(h)) => Some(LayoutBox {
            x: 0.0,
            y: 0.0,
            w,
            h,
        }),
        _ => None,
    };
    out.extend(block_overlap(&ledger, trim));
    out.extend(chart_overflow(&ledger, &authored));
    out
}
