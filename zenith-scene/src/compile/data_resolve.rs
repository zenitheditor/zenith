//! Data-binding resolution: substitute `PropertyValue::DataRef` with the
//! concrete field value from a [`DataContext`], emitting advisories when the
//! context is absent or the field is missing.

use std::borrow::Cow;

use zenith_core::{DataContext, Diagnostic, PropertyValue};

/// Attempt to resolve a `PropertyValue::DataRef` field path against `data`.
///
/// - If `pv` is NOT a `DataRef`, returns `Cow::Borrowed(pv)` (zero-copy, no
///   diagnostics — byte-identical for non-data docs).
/// - If `pv` is a `DataRef` and `data` is `None`, pushes `data.no_context` and
///   returns `Cow::Borrowed(pv)` (the unresolved ref; callers skip it).
/// - If `pv` is a `DataRef`, `data` is `Some`, but the path is absent, pushes
///   `data.missing_field` and returns `Cow::Borrowed(pv)`.
/// - If `pv` is a `DataRef` and the field resolves, returns
///   `Cow::Owned(PropertyValue::Literal(value))` — the caller treats it as a
///   raw literal for the rest of the paint pipeline.
pub(super) fn resolve_data_prop<'a>(
    pv: &'a PropertyValue,
    data: Option<&'a DataContext>,
    prop_name: &str,
    subject_id: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Cow<'a, PropertyValue> {
    let PropertyValue::DataRef(path) = pv else {
        // Fast path — not a data reference; return unchanged.
        return Cow::Borrowed(pv);
    };

    let Some(ctx) = data else {
        diagnostics.push(Diagnostic::advisory(
            "data.no_context",
            format!(
                "node '{subject_id}': property '{prop_name}' is a data reference \
                 '(data)\"{path}\"' but no data context was provided at compile time; skipped"
            ),
            None,
            Some(subject_id.to_owned()),
        ));
        return Cow::Borrowed(pv);
    };

    match ctx.get(path) {
        Some(value) => Cow::Owned(PropertyValue::Literal(value.to_owned())),
        None => {
            diagnostics.push(Diagnostic::advisory(
                "data.missing_field",
                format!(
                    "node '{subject_id}': property '{prop_name}' references data field \
                     '{path}' which is not present in the data context; skipped"
                ),
                None,
                Some(subject_id.to_owned()),
            ));
            Cow::Borrowed(pv)
        }
    }
}
