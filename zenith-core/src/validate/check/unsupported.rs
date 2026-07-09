//! Report child nodes dropped at parse time because their parent kind does not
//! accept them (`node.unsupported_child`, Warning).
//!
//! The parse layer has no diagnostic sink, so it records each discarded child in
//! [`crate::ast::Document::unsupported_children`]; this pass turns those records
//! into one Warning apiece, carrying the child's span and the parent id so the
//! author can find and fix the silent data loss.

use crate::ast::Document;
use crate::diagnostics::Diagnostic;

/// Emit one `node.unsupported_child` Warning per parse-time discarded child.
///
/// A document with no such authoring mistakes has an empty side table, so this
/// pass is a no-op and adds no diagnostics.
pub(in crate::validate::check) fn check_unsupported_children(
    doc: &Document,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for entry in &doc.unsupported_children {
        let subject = match &entry.parent_id {
            Some(id) => format!("{} '{}'", entry.parent_kind, id),
            None => entry.parent_kind.clone(),
        };
        diagnostics.push(Diagnostic::warning(
            "node.unsupported_child",
            format!(
                "{subject}: child node '{child}' is not supported by '{parent}' and was \
                 discarded; place it as a sibling inside a group",
                child = entry.child_kind,
                parent = entry.parent_kind,
            ),
            entry.source_span,
            entry.parent_id.clone(),
        ));
    }
}
