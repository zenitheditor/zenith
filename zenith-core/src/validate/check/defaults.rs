//! `defaults { … }` block checks (document and page scope).
//!
//! - unknown kind → `defaults.unknown_kind` (Error, did-you-mean over kinds)
//! - `instance` / `light` / `mesh` → `defaults.unsupported_kind` (Error)
//! - repeated kind → `defaults.duplicate_kind` (Error)
//! - undeclared `style` / `text-style` → `defaults.unknown_style` (Error,
//!   did-you-mean over style ids)
//! - `text-style` on a kind without a label → `defaults.text_style_unsupported`
//!   (Error)
//! - attribute other than `style` / `text-style` → `defaults.unknown_property`
//!   (Error)
//!
//! Children of a row are reported by the `block.unknown_child` path.

use std::collections::BTreeSet;

use crate::ast::{
    DEFAULTS_ENTRY_PROPS, DEFAULTS_UNSUPPORTED_KINDS, DefaultsBlock, DefaultsEntry, DefaultsKind,
    DefaultsRejection,
};
use crate::diagnostics::{Diagnostic, FixHint};
use crate::suggest::{
    find_suggestion, format_candidate_list, rename_property_fix, replace_value_fix,
    unknown_property_message,
};

/// Check one `defaults` block. `scope` names it for messages (`document` or
/// `page 'p1'`); `subject_id` is the diagnostic subject (the page id, or
/// `None` at document scope).
pub(in crate::validate::check) fn check_defaults(
    block: &DefaultsBlock,
    scope: &str,
    subject_id: Option<&str>,
    declared_style_ids: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let cx = Cx {
        scope,
        subject_id,
        declared_style_ids,
    };
    for (kind, entry) in &block.entries {
        check_entry(&cx, kind.name(), Some(*kind), entry, diagnostics);
    }
    for rejected in &block.rejected {
        let name = rejected.name.as_str();
        let entry = &rejected.entry;
        match rejected.reason {
            DefaultsRejection::UnknownKind => {
                diagnostics.push(unknown_kind(&cx, name, entry));
                check_entry(&cx, name, None, entry, diagnostics);
            }
            DefaultsRejection::UnsupportedKind => {
                diagnostics.push(cx.error(
                    "defaults.unsupported_kind",
                    format!(
                        "{} defaults: kind '{name}' carries no style and takes no default; \
                         remove the row (kinds without a default: {})",
                        cx.scope,
                        DEFAULTS_UNSUPPORTED_KINDS.join(", ")
                    ),
                    entry,
                ));
                check_entry(&cx, name, None, entry, diagnostics);
            }
            DefaultsRejection::DuplicateKind(kind) => {
                diagnostics.push(cx.error(
                    "defaults.duplicate_kind",
                    format!(
                        "{} defaults: kind '{name}' is declared more than once; \
                         the first row wins — remove this row",
                        cx.scope
                    ),
                    entry,
                ));
                check_entry(&cx, name, Some(kind), entry, diagnostics);
            }
        }
    }
}

/// Shared message context for one block.
struct Cx<'a> {
    scope: &'a str,
    subject_id: Option<&'a str>,
    declared_style_ids: &'a BTreeSet<String>,
}

impl Cx<'_> {
    fn error(&self, code: &str, message: String, entry: &DefaultsEntry) -> Diagnostic {
        Diagnostic::error(
            code,
            message,
            entry.source_span,
            self.subject_id.map(str::to_owned),
        )
    }
}

/// `defaults.unknown_kind` with a did-you-mean over the kinds and a
/// `ReplaceValue` hint when the nearest kind is unique.
fn unknown_kind(cx: &Cx<'_>, name: &str, entry: &DefaultsEntry) -> Diagnostic {
    let kinds = DefaultsKind::names();
    let message = match find_suggestion(name, kinds.iter().copied(), 2) {
        Some(s) => format!(
            "{} defaults: unknown kind '{name}' — did you mean '{s}'? Kinds: {}",
            cx.scope,
            kinds.join(", ")
        ),
        None => format!(
            "{} defaults: unknown kind '{name}' — use one of: {}",
            cx.scope,
            kinds.join(", ")
        ),
    };
    cx.error("defaults.unknown_kind", message, entry)
        .with_fix(replace_value_fix("kind", name, &kinds))
}

/// Style references, `text-style` support, and unknown attributes of a row.
/// `kind` is `None` for a row whose kind was not accepted.
fn check_entry(
    cx: &Cx<'_>,
    name: &str,
    kind: Option<DefaultsKind>,
    entry: &DefaultsEntry,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_style(cx, name, "style", &entry.style, entry, diagnostics);
    if let Some(ts) = &entry.text_style {
        if let Some(k) = kind
            && !k.accepts_text_style()
        {
            diagnostics.push(
                cx.error(
                    "defaults.text_style_unsupported",
                    format!(
                        "{} defaults: kind '{name}' has no label, so text-style=\"{ts}\" has no \
                         effect; remove text-style (only connector and shape take it)",
                        cx.scope
                    ),
                    entry,
                )
                .with_fix(Some(FixHint::RemoveProperty {
                    property: "text-style".to_owned(),
                })),
            );
        }
        check_style(cx, name, "text-style", ts, entry, diagnostics);
    }
    for prop in entry.unknown_props.keys() {
        diagnostics.push(
            cx.error(
                "defaults.unknown_property",
                unknown_property_message(
                    &format!("{} defaults row '{name}'", cx.scope),
                    "defaults row",
                    prop,
                    DEFAULTS_ENTRY_PROPS,
                ),
                entry,
            )
            .with_fix(rename_property_fix(prop, DEFAULTS_ENTRY_PROPS)),
        );
    }
}

/// `defaults.unknown_style` when `style_id` is not declared.
fn check_style(
    cx: &Cx<'_>,
    name: &str,
    attr: &str,
    style_id: &str,
    entry: &DefaultsEntry,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if cx.declared_style_ids.contains(style_id) {
        return;
    }
    let ids = cx.declared_style_ids.iter().map(String::as_str);
    let hint = match find_suggestion(style_id, ids.clone(), 2) {
        Some(s) => format!("did you mean '{s}'?"),
        None => format!(
            "declare it in the styles block ({})",
            format_candidate_list("styles", ids)
        ),
    };
    let ids_vec: Vec<&str> = cx.declared_style_ids.iter().map(String::as_str).collect();
    diagnostics.push(
        cx.error(
            "defaults.unknown_style",
            format!(
                "{} defaults: row '{name}' {attr}=\"{style_id}\" names an undeclared style — {hint}",
                cx.scope
            ),
            entry,
        )
        .with_fix(replace_value_fix(attr, style_id, &ids_vec)),
    );
}
