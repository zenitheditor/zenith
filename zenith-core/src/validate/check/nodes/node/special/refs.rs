//! Per-kind checks for the reference-bearing leaf nodes: `toc` and `footnote`.

use std::collections::BTreeSet;

use crate::ast::node::{FootnoteNode, TocNode};
use crate::diagnostics::Diagnostic;

use crate::validate::check::nodes::WalkCtx;
use crate::validate::check::nodes::node::shared::{
    AnchorParentCtx, AnchorProps, check_anchor, check_spans, check_style_ref,
};
use crate::validate::check::nodes::node::suggest::check_unknown_props;
use crate::validate::check::register_id;
use crate::validate::check::visual::{VisualExpect, check_visual_prop};

/// Validate a [`TocNode`]: id uniqueness, style ref, visual properties, and
/// the `toc.no_selector` advisory when both `match_role` and `match_style` are
/// absent (the toc would collect no entries at compile time).
pub(in crate::validate::check) fn check_toc(
    toc: &TocNode,
    ctx: WalkCtx,
    seen_ids: &mut BTreeSet<String>,
    referenced_token_ids: &mut BTreeSet<String>,
    parent_ctx: AnchorParentCtx,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let WalkCtx {
        resolved_tokens,
        declared_style_ids,
        zone_ids,
        ..
    } = ctx;
    register_id(&toc.id, seen_ids, diagnostics);
    check_style_ref(
        &toc.id,
        toc.style.as_deref(),
        declared_style_ids,
        toc.source_span,
        diagnostics,
    );

    // Validate the anchor value (geometry is all-optional for toc anyway).
    check_anchor(
        &toc.id,
        AnchorProps {
            anchor: toc.anchor.as_deref(),
            anchor_zone: toc.anchor_zone.as_deref(),
            anchor_sibling: toc.anchor_sibling.as_deref(),
            anchor_parent: toc.anchor_parent == Some(true),
            anchor_edge: toc.anchor_edge.as_deref(),
            anchor_gap: toc.anchor_gap.as_ref(),
        },
        parent_ctx,
        zone_ids,
        toc.source_span,
        diagnostics,
    );

    // Warn when neither selector is set: the toc will collect no entries.
    if toc.match_role.is_none() && toc.match_style.is_none() {
        diagnostics.push(Diagnostic::warning(
            "toc.no_selector",
            format!(
                "toc '{}' has neither match-role nor match-style; it will collect no entries",
                toc.id
            ),
            toc.source_span,
            Some(toc.id.clone()),
        ));
    }

    // Visual properties (mirror the field-node checks).
    check_visual_prop(
        &toc.id,
        "fill",
        toc.fill.as_ref(),
        VisualExpect::Color,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &toc.id,
        "font-family",
        toc.font_family.as_ref(),
        VisualExpect::FontFamily,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &toc.id,
        "font-size",
        toc.font_size.as_ref(),
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );

    // Unknown properties on the toc node.
    check_unknown_props(
        "toc",
        &toc.id,
        &toc.unknown_props,
        toc.source_span,
        diagnostics,
    );
}

/// Validate a [`FootnoteNode`]: id uniqueness, style ref, the content-span and
/// node visual properties (fill/font-family/font-size, plus per-span fill/weight
/// so raw visual literals are surfaced like any text), and unknown properties.
///
/// The structural `footnote.unresolved_ref` check (a span `footnote-ref` that
/// names no footnote on the same page) is done at the PAGE level (it needs the
/// page's footnote-id set), not here. A footnote has no geometry, so there are
/// no geometry checks.
pub(in crate::validate::check) fn check_footnote(
    footnote: &FootnoteNode,
    ctx: WalkCtx,
    seen_ids: &mut BTreeSet<String>,
    referenced_token_ids: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let WalkCtx {
        resolved_tokens,
        declared_style_ids,
        ..
    } = ctx;
    register_id(&footnote.id, seen_ids, diagnostics);
    check_style_ref(
        &footnote.id,
        footnote.style.as_deref(),
        declared_style_ids,
        footnote.source_span,
        diagnostics,
    );

    check_visual_prop(
        &footnote.id,
        "fill",
        footnote.fill.as_ref(),
        VisualExpect::Color,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &footnote.id,
        "font-family",
        footnote.font_family.as_ref(),
        VisualExpect::FontFamily,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &footnote.id,
        "font-size",
        footnote.font_size.as_ref(),
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );

    // Per-span visual props (mirror the text-node span checks) so token refs are
    // registered and raw visual literals are flagged `token.raw_visual_literal`.
    check_spans(
        &footnote.id,
        &footnote.spans,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );

    check_unknown_props(
        "footnote",
        &footnote.id,
        &footnote.unknown_props,
        footnote.source_span,
        diagnostics,
    );
}
