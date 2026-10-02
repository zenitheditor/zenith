//! "Did you mean?" diagnostics for unknown property names and bad enum values.
//!
//! [`check_unknown_props`] is the single shared helper for the per-kind
//! `check_*` files. It emits one `node.unknown_property` Error per unknown
//! property name. [`push_invalid_value`] emits one `node.invalid_value` Error
//! for a known property whose enum value is not allowed. Both use the
//! crate-level [`crate::suggest`] helpers for the nearest-name suggestion.

use std::collections::BTreeMap;

use crate::ast::Span;
use crate::ast::node::UnknownProperty;
use crate::diagnostics::Diagnostic;
use crate::parse::transform::known_props_for_kind;
use crate::suggest::{invalid_value_message, unknown_property_message};

/// Emit one `node.unknown_property` Error for every entry in `unknown`.
///
/// The closest known property name for `kind` within edit distance ≤ 2 is
/// suggested (see `crate::suggest::find_suggestion`; ties go to the lexicographically
/// smallest name). Without a near miss, the message points at
/// `zenith schema`.
///
/// Code is always `"node.unknown_property"`, severity always Error.
pub(super) fn check_unknown_props(
    kind: &str,
    id: &str,
    unknown: &BTreeMap<String, UnknownProperty>,
    span: Option<Span>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let known = known_props_for_kind(kind);

    for prop_name in unknown.keys() {
        let message = unknown_property_message(&format!("{kind} '{id}'"), kind, prop_name, known);

        diagnostics.push(Diagnostic::error(
            "node.unknown_property",
            message,
            span,
            Some(id.to_owned()),
        ));
    }
}

/// Emit one `node.invalid_value` Error: `prop` is a known property but
/// `value` is not one of `allowed`.
///
/// `subject` names the owner (for example `polygon 'p1'`). The message names
/// the property, the bad value, and every allowed value, and adds a
/// did-you-mean when `value` is within edit distance ≤ 2 of an allowed value.
pub(in crate::validate::check) fn push_invalid_value(
    subject: &str,
    node_id: Option<String>,
    prop: &str,
    value: &str,
    allowed: &[&str],
    span: Option<Span>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let message = invalid_value_message(subject, prop, value, allowed);
    diagnostics.push(Diagnostic::error(
        "node.invalid_value",
        message,
        span,
        node_id,
    ));
}

/// Allowed `blend-mode` values, in canonical order.
pub(in crate::validate::check) fn blend_mode_names() -> Vec<&'static str> {
    crate::color::BlendMode::ALL
        .iter()
        .map(crate::color::BlendMode::as_kebab)
        .collect()
}
