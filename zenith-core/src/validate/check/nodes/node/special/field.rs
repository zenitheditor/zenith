//! Per-kind check for the `field` node.

use std::collections::BTreeSet;

use crate::ast::node::FieldNode;
use crate::diagnostics::Diagnostic;

use crate::validate::check::nodes::WalkCtx;
use crate::validate::check::nodes::node::shared::{
    AnchorParentCtx, AnchorProps, check_anchor, check_style_ref,
};
use crate::validate::check::nodes::node::suggest::check_unknown_props;
use crate::validate::check::register_id;
use crate::validate::check::visual::{VisualExpect, check_visual_prop};

// ── field validation ──────────────────────────────────────────────────────────

/// The known v0 field types.
const KNOWN_FIELD_TYPES: &[&str] = &[
    "running-head",
    "page-number",
    "page-ref",
    "page-count",
    "section-page-number",
    "section-page-count",
    "section-name",
];

/// Validate a `field` node:
/// - its own `id` participates in GLOBAL uniqueness;
/// - `type` must be one of the known field types → else `field.unknown_type`
///   (Warning);
/// - a `page-ref` field whose `target` matches no node id anywhere in the
///   document → `field.unresolved_ref` (Warning);
/// - `style`/`fill`/`font-family`/`font-size` are validated like a text node's,
///   and any token refs are registered so they are not flagged unused.
///
/// A field is a leaf — it does not recurse. Geometry is optional (an absent
/// x/w defaults to the page live area at compile time), so no missing-geometry
/// error is raised here.
pub(in crate::validate::check) fn check_field(
    field: &FieldNode,
    ctx: WalkCtx,
    seen_ids: &mut BTreeSet<String>,
    referenced_token_ids: &mut BTreeSet<String>,
    parent_ctx: AnchorParentCtx,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let WalkCtx {
        resolved_tokens,
        declared_style_ids,
        all_node_ids,
        zone_ids,
        ..
    } = ctx;
    register_id(&field.id, seen_ids, diagnostics);
    check_style_ref(
        &field.id,
        field.style.as_deref(),
        declared_style_ids,
        field.source_span,
        diagnostics,
    );

    // Validate the anchor value (geometry is all-optional for fields anyway).
    check_anchor(
        &field.id,
        AnchorProps {
            anchor: field.anchor.as_deref(),
            anchor_zone: field.anchor_zone.as_deref(),
            anchor_sibling: field.anchor_sibling.as_deref(),
            anchor_parent: field.anchor_parent == Some(true),
            anchor_edge: field.anchor_edge.as_deref(),
            anchor_gap: field.anchor_gap.as_ref(),
        },
        parent_ctx,
        zone_ids,
        field.source_span,
        diagnostics,
    );

    // Unknown field type → Warning (never a hard error; the field simply renders
    // nothing at compile time).
    if !KNOWN_FIELD_TYPES.contains(&field.field_type.as_str()) {
        diagnostics.push(Diagnostic::warning(
            "field.unknown_type",
            format!(
                "field '{}': unknown type '{}'; expected one of {}",
                field.id,
                field.field_type,
                KNOWN_FIELD_TYPES.join(", ")
            ),
            field.source_span,
            Some(field.id.clone()),
        ));
    }

    // A page-ref field with an unresolvable target → Warning. A page-ref with no
    // target at all is also unresolved (nothing to point at).
    if field.field_type == "page-ref" {
        let resolved = field
            .target
            .as_ref()
            .map(|t| all_node_ids.contains(t))
            .unwrap_or(false);
        if !resolved {
            diagnostics.push(Diagnostic::warning(
                "field.unresolved_ref",
                format!(
                    "field '{}': page-ref target {} matches no node id in the document",
                    field.id,
                    field
                        .target
                        .as_deref()
                        .map(|t| format!("'{t}'"))
                        .unwrap_or_else(|| "(absent)".to_owned())
                ),
                field.source_span,
                Some(field.id.clone()),
            ));
        }
    }

    // Visual properties (mirror the text-node checks).
    check_visual_prop(
        &field.id,
        "fill",
        field.fill.as_ref(),
        VisualExpect::Color,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &field.id,
        "font-family",
        field.font_family.as_ref(),
        VisualExpect::FontFamily,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &field.id,
        "font-size",
        field.font_size.as_ref(),
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );

    // Unknown properties on the field node.
    check_unknown_props(
        "field",
        &field.id,
        &field.unknown_props,
        field.source_span,
        diagnostics,
    );
}
