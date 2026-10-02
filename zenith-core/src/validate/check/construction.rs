//! Validation for non-printing construction guide metadata.

use crate::ast::construction::ConstructionGuideDef;
use crate::ast::document::Page;
use crate::ast::value::{Dimension, dim_to_px};
use crate::diagnostics::Diagnostic;
use crate::suggest::invalid_value_message;

pub(super) fn check_construction(page: &Page, diagnostics: &mut Vec<Diagnostic>) {
    for guide in &page.construction.guides {
        match guide.guide_type.as_str() {
            "segment" => check_segment(guide, diagnostics),
            "circle" => check_circle(guide, diagnostics),
            other => diagnostics.push(Diagnostic::error(
                "construction.unknown_guide_type",
                invalid_value_message(
                    &format!("construction guide '{}'", guide.id),
                    "type",
                    other,
                    &["segment", "circle"],
                ),
                guide.source_span,
                Some(guide.id.clone()),
            )),
        }
    }
}

fn check_segment(guide: &ConstructionGuideDef, diagnostics: &mut Vec<Diagnostic>) {
    let Some(x1) = required_px(guide, "x1", guide.x1.as_ref(), diagnostics) else {
        return;
    };
    let Some(y1) = required_px(guide, "y1", guide.y1.as_ref(), diagnostics) else {
        return;
    };
    let Some(x2) = required_px(guide, "x2", guide.x2.as_ref(), diagnostics) else {
        return;
    };
    let Some(y2) = required_px(guide, "y2", guide.y2.as_ref(), diagnostics) else {
        return;
    };

    if x1 == x2 && y1 == y2 {
        diagnostics.push(Diagnostic::warning(
            "construction.degenerate_guide",
            format!(
                "construction guide '{}' segment endpoints must not be identical",
                guide.id
            ),
            guide.source_span,
            Some(guide.id.clone()),
        ));
    }
}

fn check_circle(guide: &ConstructionGuideDef, diagnostics: &mut Vec<Diagnostic>) {
    let _ = required_px(guide, "cx", guide.cx.as_ref(), diagnostics);
    let _ = required_px(guide, "cy", guide.cy.as_ref(), diagnostics);
    let Some(radius) = required_px(guide, "r", guide.r.as_ref(), diagnostics) else {
        return;
    };

    if radius <= 0.0 {
        diagnostics.push(Diagnostic::warning(
            "construction.invalid_radius",
            format!(
                "construction guide '{}' circle radius must be positive",
                guide.id
            ),
            guide.source_span,
            Some(guide.id.clone()),
        ));
    }
}

fn required_px(
    guide: &ConstructionGuideDef,
    field: &str,
    value: Option<&Dimension>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<f64> {
    let Some(value) = value else {
        diagnostics.push(Diagnostic::warning(
            "construction.missing_geometry",
            format!(
                "construction guide '{}' is missing required property '{}'",
                guide.id, field
            ),
            guide.source_span,
            Some(guide.id.clone()),
        ));
        return None;
    };

    let Some(px) = dim_to_px(value.value, &value.unit) else {
        diagnostics.push(Diagnostic::warning(
            "construction.invalid_geometry",
            format!(
                "construction guide '{}' property '{}' must use a px/pt dimension",
                guide.id, field
            ),
            guide.source_span,
            Some(guide.id.clone()),
        ));
        return None;
    };

    if !px.is_finite() {
        diagnostics.push(Diagnostic::warning(
            "construction.invalid_geometry",
            format!(
                "construction guide '{}' property '{}' must resolve to a finite value",
                guide.id, field
            ),
            guide.source_span,
            Some(guide.id.clone()),
        ));
        None
    } else {
        Some(px)
    }
}
