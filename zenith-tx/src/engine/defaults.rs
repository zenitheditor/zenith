//! `defaults` op application: [`apply_set_default`] and [`apply_remove_default`].
//!
//! Both ops edit one `defaults { }` block: the document block, or the block of
//! one page. Entries live in a `BTreeMap` keyed by kind, so the canonical
//! formatter writes them sorted by kind.

use zenith_core::{
    DEFAULTS_UNSUPPORTED_KINDS, DefaultsBlock, DefaultsEntry, DefaultsKind, Diagnostic, Document,
    find_suggestion, format_candidate_list,
};

use super::record_affected;

/// Scalars of a `set_default` op, bundled into one `Copy` borrow struct.
#[derive(Clone, Copy)]
pub(super) struct SetDefaultScalars<'a> {
    pub(super) page: Option<&'a str>,
    pub(super) kind: &'a str,
    pub(super) style: &'a str,
    pub(super) text_style: Option<Option<&'a str>>,
}

/// Emit a `tx.invalid_value` error for `op`.
fn invalid(op: &str, message: String, subject: &str, diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.push(Diagnostic::error(
        "tx.invalid_value",
        format!("{op}: {message}"),
        None,
        Some(subject.to_owned()),
    ));
}

/// Resolve `kind` to a [`DefaultsKind`], or push an error naming the valid kinds.
fn resolve_kind(op: &str, kind: &str, diagnostics: &mut Vec<Diagnostic>) -> Option<DefaultsKind> {
    if let Some(k) = DefaultsKind::from_name(kind) {
        return Some(k);
    }
    if DEFAULTS_UNSUPPORTED_KINDS.contains(&kind) {
        invalid(
            op,
            format!(
                "kind {kind:?} carries no style and takes no defaults entry; use one of: {}",
                DefaultsKind::names().join(", ")
            ),
            kind,
            diagnostics,
        );
        return None;
    }
    let names = DefaultsKind::names();
    let hint = match find_suggestion(kind, names.iter().copied(), 2) {
        Some(s) => format!("did you mean {s:?}?"),
        None => "use a valid kind".to_owned(),
    };
    invalid(
        op,
        format!("unknown kind {kind:?}; {hint} Kinds: {}", names.join(", ")),
        kind,
        diagnostics,
    );
    None
}

/// Push `tx.unknown_style` unless `style_id` is declared in `doc.styles`.
/// Returns `true` when the style exists.
fn check_style(
    op: &str,
    style_id: &str,
    doc: &Document,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if doc.styles.styles.iter().any(|s| s.id == style_id) {
        return true;
    }
    let ids = doc.styles.styles.iter().map(|s| s.id.as_str());
    let hint = match find_suggestion(style_id, ids.clone(), 2) {
        Some(s) => format!("did you mean {s:?}?"),
        None => format!(
            "create it with create_style first ({})",
            format_candidate_list("styles", ids)
        ),
    };
    diagnostics.push(Diagnostic::error(
        "tx.unknown_style",
        format!("{op}: style {style_id:?} not found in styles; {hint}"),
        None,
        Some(style_id.to_owned()),
    ));
    false
}

/// The `defaults` block for `page` (or the document block), or an error when
/// the page does not exist.
fn block_mut<'a>(
    op: &str,
    page: Option<&str>,
    doc: &'a mut Document,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<&'a mut DefaultsBlock> {
    let Some(page_id) = page else {
        return Some(&mut doc.defaults);
    };
    let found = doc
        .body
        .pages
        .iter_mut()
        .find(|p| p.id == page_id)
        .map(|p| &mut p.defaults);
    if found.is_none() {
        diagnostics.push(Diagnostic::error(
            "tx.unknown_node",
            format!("{op}: page {page_id:?} not found"),
            None,
            Some(page_id.to_owned()),
        ));
    }
    found
}

/// Upsert the `defaults` entry for `s.kind`.
///
/// `text_style`: `None` keeps the current value, `Some(None)` clears it,
/// `Some(Some(id))` sets it (`shape` and `connector` only). Every referenced
/// style must exist. On success records the page id (or the kind for document
/// scope) in `affected`.
pub(super) fn apply_set_default(
    s: SetDefaultScalars<'_>,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    const OP: &str = "set_default";
    let Some(kind) = resolve_kind(OP, s.kind, diagnostics) else {
        return;
    };
    if let Some(Some(_)) = s.text_style
        && !kind.accepts_text_style()
    {
        invalid(
            OP,
            format!(
                "kind {:?} takes no text_style; only shape and connector entries do. \
                 Omit text_style for this kind",
                s.kind
            ),
            s.kind,
            diagnostics,
        );
        return;
    }
    let style_ok = check_style(OP, s.style, doc, diagnostics);
    let text_style_ok = match s.text_style {
        Some(Some(id)) => check_style(OP, id, doc, diagnostics),
        Some(None) | None => true,
    };
    if !style_ok || !text_style_ok {
        return;
    }
    let Some(block) = block_mut(OP, s.page, doc, diagnostics) else {
        return;
    };
    let existing = block.entries.remove(&kind);
    let text_style = match s.text_style {
        None => existing.as_ref().and_then(|e| e.text_style.clone()),
        Some(None) => None,
        Some(Some(id)) => Some(id.to_owned()),
    };
    let (unknown_props, source_span) = match existing {
        Some(e) => (e.unknown_props, e.source_span),
        None => (Default::default(), None),
    };
    block.entries.insert(
        kind,
        DefaultsEntry {
            style: s.style.to_owned(),
            text_style,
            unknown_props,
            source_span,
        },
    );
    record_affected(s.page.unwrap_or(s.kind), affected);
}

/// Remove the `defaults` entry for `kind` from the document or page block.
///
/// Rejects with `tx.invalid_value` when the block has no entry for `kind`.
pub(super) fn apply_remove_default(
    page: Option<&str>,
    kind: &str,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    const OP: &str = "remove_default";
    let Some(parsed) = resolve_kind(OP, kind, diagnostics) else {
        return;
    };
    let Some(block) = block_mut(OP, page, doc, diagnostics) else {
        return;
    };
    if block.entries.remove(&parsed).is_none() {
        let scope = match page {
            Some(p) => format!("page {p:?}"),
            None => "the document".to_owned(),
        };
        invalid(
            OP,
            format!("{scope} has no defaults entry for kind {kind:?}; nothing to remove"),
            kind,
            diagnostics,
        );
        return;
    }
    record_affected(page.unwrap_or(kind), affected);
}
