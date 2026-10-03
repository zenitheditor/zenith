//! The fit search shared by `overflow="autofit"` and the overflow diagnostics,
//! plus `compile_text_sized`: the sized compile with its overflow diagnostic.
//!
//! One deterministic search answers both "which size does autofit render at"
//! and "which size would fit" in a diagnostic: a DOWNWARD linear scan over
//! integer px that lays the node out at each trial size into throwaway buffers
//! and stops at the first size whose content fits the box.

use zenith_core::{Diagnostic, Dimension, PropertyValue, TextNode, Unit};

use crate::compile::RenderCtx;
use crate::compile::text::ctx::TextCompileEnv;
use crate::compile::text::overflow_mode::TextOverflow;
use crate::ir::SceneCommand;

use super::overflow::{OverflowFacts, overflow_diagnostic};
use super::sized::compile_text_core;

/// Compile a `text` leaf node at its resolved font size, then report overflow.
///
/// The layout and command stream are exactly [`compile_text_core`]'s. When the
/// content overflows a complete box, the mode's diagnostic is pushed: a
/// `text.overflow` warning (`clip`, the default) or a `text.fit_failed` error
/// (`fit`, `autofit` at its floor). `visible` is silent. The message names the
/// box size that fits and the largest font size below the laid-out size that
/// fits, found by [`largest_fitting_px`].
///
/// Returns the laid-out content height in pixels.
pub(in crate::compile) fn compile_text_sized(
    text: &TextNode,
    env: TextCompileEnv,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: RenderCtx,
) -> f64 {
    let outcome = compile_text_core(text, env, commands, diagnostics, ctx);
    if let Some(facts) = outcome.overflow {
        let who = format!("text '{}'", text.id);
        let site = OverflowSite {
            who: &who,
            subject: &text.id,
        };
        report_overflow(text, &facts, site, env, ctx, diagnostics);
    }
    outcome.height
}

/// The shape that hosts a synthesized label: its id and the padding between
/// the shape box and the label box.
#[derive(Clone, Copy)]
pub(in crate::compile) struct LabelHost<'a> {
    pub(in crate::compile) id: &'a str,
    pub(in crate::compile) pad: f64,
}

/// Compile a shape's synthesized label, reporting overflow against the HOST
/// shape: the diagnostic names the shape id and the shape `w`/`h` (label box
/// plus `2 × pad`) that fits. Layout, clipping, and the fit search are exactly
/// [`compile_text_sized`]'s.
pub(in crate::compile) fn compile_label_text(
    text: &TextNode,
    host: LabelHost,
    env: TextCompileEnv,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: RenderCtx,
) -> f64 {
    let outcome = compile_text_core(text, env, commands, diagnostics, ctx);
    if let Some(facts) = outcome.overflow {
        let who = format!("label of shape '{}'", host.id);
        let facts = facts.padded(host.pad);
        let site = OverflowSite {
            who: &who,
            subject: host.id,
        };
        report_overflow(text, &facts, site, env, ctx, diagnostics);
    }
    outcome.height
}

/// Who an overflow diagnostic speaks for: the opening of the message and the
/// subject id.
#[derive(Clone, Copy)]
struct OverflowSite<'a> {
    who: &'a str,
    subject: &'a str,
}

/// Push the overflow diagnostic for `text` in its `overflow` mode: search the
/// largest fitting font size, then build the message for `site`.
fn report_overflow(
    text: &TextNode,
    facts: &OverflowFacts,
    site: OverflowSite,
    env: TextCompileEnv,
    ctx: RenderCtx,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mode = TextOverflow::from_attr(text.overflow.as_deref());
    let fitting_px = match mode {
        // Intentional overflow: no diagnostic, so no search.
        TextOverflow::Visible => return,
        TextOverflow::Clip | TextOverflow::Fit | TextOverflow::Autofit => {
            largest_fitting_px(text, env, ctx, below_px(facts.font_size), 1)
        }
    };
    if let Some(d) = overflow_diagnostic(
        site.who,
        site.subject,
        text.source_span,
        mode,
        facts,
        fitting_px,
    ) {
        diagnostics.push(d);
    }
}

/// The largest integer px strictly below `font_size`.
fn below_px(font_size: f64) -> i64 {
    font_size.ceil() as i64 - 1
}

/// The largest integer font size in `[to_px, from_px]` at which `text` fits its
/// box, scanning DOWNWARD and stopping at the first fit (deterministic: same
/// inputs → same size). `to_px` is clamped to at least 1. `None` when no size in
/// the range fits.
pub(super) fn largest_fitting_px(
    text: &TextNode,
    env: TextCompileEnv,
    ctx: RenderCtx,
    from_px: i64,
    to_px: i64,
) -> Option<i64> {
    let floor = to_px.max(1);
    let mut fs = from_px;
    while fs >= floor {
        if fits_at(text, env, ctx, fs as f64) {
            return Some(fs);
        }
        fs -= 1;
    }
    None
}

/// Does `text` fit its box at font size `fs`? Lays the node out into throwaway
/// command/diagnostic buffers and checks for measured overflow. A node without a
/// complete box, or one whose branch owns its overflow handling (chain member,
/// markdown, tab leader), always fits here.
fn fits_at(text: &TextNode, env: TextCompileEnv, ctx: RenderCtx, fs: f64) -> bool {
    let trial = with_font_size(text, fs);
    let mut throwaway_cmds: Vec<SceneCommand> = Vec::new();
    let mut throwaway_diags: Vec<Diagnostic> = Vec::new();
    compile_text_core(&trial, env, &mut throwaway_cmds, &mut throwaway_diags, ctx)
        .overflow
        .is_none()
}

/// A clone of `text` with its node font size set to `fs` px.
pub(super) fn with_font_size(text: &TextNode, fs: f64) -> TextNode {
    let mut t = text.clone();
    t.font_size = Some(PropertyValue::Dimension(Dimension {
        value: fs,
        unit: Unit::Px,
    }));
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn below_px_is_strictly_smaller() {
        assert_eq!(below_px(64.0), 63);
        assert_eq!(below_px(15.5), 15);
        assert_eq!(below_px(1.0), 0);
    }
}
