//! Per-kind check for the `instance` node.

use std::collections::BTreeSet;

use crate::ast::node::InstanceNode;
use crate::diagnostics::Diagnostic;

use crate::validate::check::nodes::WalkCtx;
use crate::validate::check::nodes::node::suggest::check_unknown_props;
use crate::validate::check::register_id;
use crate::validate::check::visual::{VisualExpect, check_visual_prop};

// ── instance validation ───────────────────────────────────────────────────────

/// Validate an `instance` node:
/// - its own `id` participates in GLOBAL uniqueness;
/// - `component` must reference a declared component → else
///   `component.unknown_reference` (Error);
/// - each override `ref` must match a LOCAL descendant id of the referenced
///   component → else `component.unknown_override_target` (Warning).
///
/// The instance is a container-ish node but it does NOT recurse here: its
/// expanded subtree (and the component definition's own ids) are validated at
/// the component definition site, not per-instance, so token/asset refs are
/// checked once.
pub(in crate::validate::check) fn check_instance(
    inst: &InstanceNode,
    ctx: WalkCtx,
    seen_ids: &mut BTreeSet<String>,
    referenced_token_ids: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let WalkCtx {
        resolved_tokens,
        declared_component_ids,
        component_local_ids,
        ..
    } = ctx;
    register_id(&inst.id, seen_ids, diagnostics);

    let has_component = inst.component.is_some();
    let has_source = inst.source.is_some();
    if !has_component && !has_source {
        diagnostics.push(Diagnostic::error(
            "instance.missing_reference",
            format!(
                "instance '{}': exactly one of component or source is required",
                inst.id
            ),
            inst.source_span,
            Some(inst.id.clone()),
        ));
    }
    if has_component && has_source {
        diagnostics.push(Diagnostic::error(
            "instance.multiple_references",
            format!(
                "instance '{}': component and source are mutually exclusive",
                inst.id
            ),
            inst.source_span,
            Some(inst.id.clone()),
        ));
    }

    let component_known = match &inst.component {
        Some(component) => declared_component_ids.contains(component),
        None => false,
    };
    if let Some(component) = &inst.component
        && !component_known
    {
        diagnostics.push(Diagnostic::error(
            "component.unknown_reference",
            format!(
                "instance '{}': references component '{}' which is not declared in the \
                 components block",
                inst.id, component
            ),
            inst.source_span,
            Some(inst.id.clone()),
        ));
    }

    // Override targets are only checkable when the component is known. Look up the
    // referenced component's local-id set; an override `ref` that matches no local
    // descendant id → warning.
    let local_ids = inst
        .component
        .as_ref()
        .and_then(|component| component_local_ids.get(component));
    for ov in &inst.overrides {
        // Validate (and register as referenced) any token refs the override
        // carries, so an override-only token is not falsely flagged unused and
        // a bad override fill/span fill is type-checked like a node fill.
        check_visual_prop(
            &inst.id,
            "fill",
            ov.fill.as_ref(),
            VisualExpect::Color,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
        check_visual_prop(
            &inst.id,
            "stroke",
            ov.stroke.as_ref(),
            VisualExpect::Color,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
        check_visual_prop(
            &inst.id,
            "stroke-width",
            ov.stroke_width.as_ref(),
            VisualExpect::Dimension,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
        check_visual_prop(
            &inst.id,
            "svg-stroke",
            ov.svg_stroke.as_ref(),
            VisualExpect::Color,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
        check_visual_prop(
            &inst.id,
            "svg-fill",
            ov.svg_fill.as_ref(),
            VisualExpect::Color,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
        check_visual_prop(
            &inst.id,
            "svg-stroke-width",
            ov.svg_stroke_width.as_ref(),
            VisualExpect::Dimension,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
        if let Some(spans) = &ov.spans {
            for span in spans {
                check_visual_prop(
                    &inst.id,
                    "fill",
                    span.fill.as_ref(),
                    VisualExpect::Color,
                    referenced_token_ids,
                    resolved_tokens,
                    diagnostics,
                );
            }
        }

        let target_known = local_ids
            .map(|ids| ids.contains(&ov.ref_id))
            .unwrap_or(false);
        if component_known
            && !target_known
            && let Some(component) = &inst.component
        {
            diagnostics.push(Diagnostic::warning(
                "component.unknown_override_target",
                format!(
                    "instance '{}': override ref '{}' matches no descendant id in component '{}'",
                    inst.id, ov.ref_id, component
                ),
                ov.source_span.or(inst.source_span),
                Some(inst.id.clone()),
            ));
        }
    }

    // Unknown properties on the instance node.
    check_unknown_props(
        "instance",
        &inst.id,
        &inst.unknown_props,
        inst.source_span,
        diagnostics,
    );
}
