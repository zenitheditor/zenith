//! Geometry-dimension property validators (raw dimension and token-ref forms)
//! and the narrowing helper `pv_to_dim`.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::value::{Dimension, PropertyValue, Unit};
use crate::diagnostics::Diagnostic;
use crate::tokens::ResolvedToken;
use crate::validate::check::visual::{VisualExpect, check_visual_prop};

/// Borrowed token-validation context passed to geometry helpers.
///
/// Bundles the two token-related arguments that `check_optional_dim` and its
/// callees need so the function stays within the 7-argument clippy limit without
/// an `#[allow]`.
pub(in crate::validate::check) struct TokenEnv<'a> {
    pub(in crate::validate::check) referenced: &'a mut BTreeSet<String>,
    pub(in crate::validate::check) resolved: &'a BTreeMap<String, ResolvedToken>,
}

/// - absent AND `required` (e.g. a non-flow-positioned leaf) → `node.missing_geometry` (Error).
/// - absent AND NOT `required` (e.g. a direct child of a `layout="flow"`
///   frame, whose position/size is supplied by the flow algorithm) → no
///   diagnostic.
/// - present but `Unit::Unknown` → `node.invalid_geometry` (Error) regardless
///   of `required`.
///
/// A geometry property accepts EITHER a raw dimension literal (`(px)N`) OR a
/// `(token)"id"` dimension token ref (exactly like `font-size`). The dispatch:
/// - absent + required → `node.missing_geometry`.
/// - `Dimension` with `Unit::Unknown` → `node.invalid_geometry`; a known unit is ok.
/// - `TokenRef(id)` → PRESENT and geometrically valid; existence + dimension-type
///   validation and reference registration are delegated to [`check_visual_prop`]
///   with [`VisualExpect::Dimension`] (which, on a token ref, never emits
///   `token.raw_visual_literal` — raw px geometry is intentionally allowed).
/// - `Literal` / `DataRef` → `node.invalid_geometry` (geometry can't be a bare
///   string or data ref).
pub(in crate::validate::check) fn check_optional_dim(
    node_id: &str,
    prop: &str,
    value: Option<&PropertyValue>,
    required: bool,
    span: Option<crate::ast::Span>,
    tokens: &mut TokenEnv<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match value {
        None if required => {
            diagnostics.push(Diagnostic::error(
                "node.missing_geometry",
                format!(
                    "node '{}': required geometry property '{}' is missing",
                    node_id, prop
                ),
                span,
                Some(node_id.to_owned()),
            ));
        }
        None => {
            // Flow-positioned child: geometry is supplied by the parent.
        }
        Some(PropertyValue::Dimension(d)) if matches!(d.unit, Unit::Unknown(_)) => {
            diagnostics.push(Diagnostic::error(
                "node.invalid_geometry",
                format!(
                    "node '{}': geometry property '{}' has an unrecognized unit; \
                     allowed units are px, pt, pct, deg",
                    node_id, prop
                ),
                span,
                Some(node_id.to_owned()),
            ));
        }
        Some(PropertyValue::Dimension(_)) => {
            // valid raw dimension literal.
        }
        Some(pv @ PropertyValue::TokenRef(_)) => {
            // Present + valid for the geometry check. Existence, dimension-type
            // compatibility, and reference registration are handled by the shared
            // visual-prop machinery (a token ref never trips raw_visual_literal).
            check_visual_prop(
                node_id,
                prop,
                Some(pv),
                VisualExpect::Dimension,
                &mut *tokens.referenced,
                tokens.resolved,
                diagnostics,
            );
        }
        Some(PropertyValue::Literal(_) | PropertyValue::DataRef(_)) => {
            diagnostics.push(Diagnostic::error(
                "node.invalid_geometry",
                format!(
                    "node '{}': geometry property '{}' must be a dimension literal \
                     (e.g. (px)100) or a dimension token ref; a bare string or \
                     data ref is not allowed",
                    node_id, prop
                ),
                span,
                Some(node_id.to_owned()),
            ));
        }
    }
}

/// Narrow an optional geometry [`PropertyValue`] to a raw [`Dimension`].
///
/// Returns `Some(&Dimension)` only for a `PropertyValue::Dimension`; a token ref
/// (or any non-dimension variant) yields `None`, so geometry expressed as a
/// `(token)` ref is treated as "not resolvable at validate time" by the
/// off-canvas / bbox checks (tokens resolve in a later pass).
pub(in crate::validate::check) fn pv_to_dim(pv: Option<&PropertyValue>) -> Option<&Dimension> {
    match pv? {
        PropertyValue::Dimension(d) => Some(d),
        PropertyValue::TokenRef(_) | PropertyValue::Literal(_) | PropertyValue::DataRef(_) => None,
    }
}

/// Validate a RAW [`Dimension`] geometry property (no token-ref support).
///
/// Used for geometry axes that are still typed `Option<Dimension>` and do NOT
/// accept a `(token)` ref — e.g. the `line` endpoints `x1`/`y1`/`x2`/`y2`. Same
/// missing/invalid-unit diagnostics as the dimension arm of [`check_optional_dim`].
pub(in crate::validate::check) fn check_dimension_geom(
    node_id: &str,
    prop: &str,
    dim: Option<&Dimension>,
    required: bool,
    span: Option<crate::ast::Span>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match dim {
        None if required => {
            diagnostics.push(Diagnostic::error(
                "node.missing_geometry",
                format!(
                    "node '{}': required geometry property '{}' is missing",
                    node_id, prop
                ),
                span,
                Some(node_id.to_owned()),
            ));
        }
        None => {}
        Some(d) if matches!(d.unit, Unit::Unknown(_)) => {
            diagnostics.push(Diagnostic::error(
                "node.invalid_geometry",
                format!(
                    "node '{}': geometry property '{}' has an unrecognized unit; \
                     allowed units are px, pt, pct, deg",
                    node_id, prop
                ),
                span,
                Some(node_id.to_owned()),
            ));
        }
        Some(_) => {}
    }
}
