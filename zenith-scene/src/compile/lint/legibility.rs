//! The legibility checks of one page: `text.edge_crowding`,
//! `text.too_small`, and `type.near_duplicate_size`.

use std::collections::BTreeMap;

use zenith_core::{Diagnostic, dim_to_px};

use super::edges::{Trim, edge_crowding};
use super::ledger::PageLedger;
use super::paint::Authored;
use super::run::LintEnv;
use super::small_text::too_small;
use super::text_facts::collect;
use super::type_scale::near_duplicate_size;

/// The legibility diagnostics of one page.
pub(super) fn legibility(
    env: &LintEnv<'_>,
    ledger: &PageLedger,
    authored: &BTreeMap<String, Authored>,
) -> Vec<Diagnostic> {
    let (Some(w), Some(h)) = (
        dim_to_px(env.page.width.value, &env.page.width.unit),
        dim_to_px(env.page.height.value, &env.page.height.unit),
    ) else {
        return Vec::new();
    };
    let source = env.authored.unwrap_or(env.page);
    let text = collect(&source.children, env.paint);
    let live_area =
        env.margins_declared || !env.page.safe_zones.is_empty() || !source.safe_zones.is_empty();
    let trim = Trim {
        origin: env.bleed,
        w,
        h,
    };
    let mut out = edge_crowding(ledger, authored, &text, trim, live_area);
    out.extend(too_small(ledger, &text, (w, h)));
    out.extend(near_duplicate_size(&text));
    out
}
