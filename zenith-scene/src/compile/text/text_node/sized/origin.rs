//! Resolve the box origin of a sized node: the authored `x` / `y`, or the
//! anchor-derived value when the axis is absent.

use std::collections::BTreeMap;

use zenith_core::{Diagnostic, PropertyValue, ResolvedToken, TextNode};

use crate::compile::util::{resolve_geometry_px, unsupported_unit_diag};

/// The `(x, y)` origin in px before the container translation, or `None` after
/// pushing a diagnostic (unsupported unit, or no geometry and no anchor).
pub(super) fn resolve_origin(
    text: &TextNode,
    anchor_xy: Option<(f64, f64)>,
    resolved: &BTreeMap<String, ResolvedToken>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(f64, f64)> {
    let x = resolve_axis(
        text,
        "x",
        text.x.as_ref(),
        anchor_xy.map(|(ax, _)| ax),
        resolved,
        diagnostics,
    )?;
    let y = resolve_axis(
        text,
        "y",
        text.y.as_ref(),
        anchor_xy.map(|(_, ay)| ay),
        resolved,
        diagnostics,
    )?;
    Some((x, y))
}

/// One axis: the authored dimension when present, else the anchor value.
fn resolve_axis(
    text: &TextNode,
    name: &str,
    authored: Option<&PropertyValue>,
    anchor: Option<f64>,
    resolved: &BTreeMap<String, ResolvedToken>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<f64> {
    if let Some(dim) = authored {
        let value = resolve_geometry_px(Some(dim), resolved);
        if value.is_none() {
            diagnostics.push(unsupported_unit_diag(
                "text node",
                &text.id,
                name,
                text.source_span,
            ));
        }
        return value;
    }
    if anchor.is_none() {
        diagnostics.push(Diagnostic::advisory(
            "scene.missing_geometry",
            format!(
                "text node '{}' is missing x or y geometry; skipped",
                text.id
            ),
            text.source_span,
            Some(text.id.clone()),
        ));
    }
    anchor
}
