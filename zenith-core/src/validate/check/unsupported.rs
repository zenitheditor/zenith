//! Report child nodes dropped at parse time because their parent does not
//! accept them.
//!
//! The parse layer has no diagnostic sink, so it records each discarded child in
//! [`crate::ast::Document::unsupported_children`]; this pass turns those records
//! into one diagnostic apiece, carrying the child's span and the parent id so the
//! author can find and fix the data loss:
//! - a child under a renderable node kind is a `node.unsupported_child` Error with a
//!   did-you-mean over the kind's accepted children;
//! - an extra shape child of a `mask` token is a `token.mask_extra_shape` Error;
//! - an unknown child inside a structural block is a `block.unknown_child` Error
//!   with a did-you-mean over the block's accepted children.

use crate::ast::{ChildSite, Document};
use crate::diagnostics::Diagnostic;
use crate::suggest::{unknown_child_message, unsupported_child_message};

/// Emit one diagnostic per parse-time discarded child.
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
        let (code, message) = match entry.site {
            ChildSite::Block => (
                "block.unknown_child",
                unknown_child_message(&subject, &entry.child_kind, entry.allowed),
            ),
            ChildSite::ExtraMaskShape => (
                "token.mask_extra_shape",
                format!(
                    "{subject}: extra mask shape '{child}' is ignored because only the first \
                     shape child is read; remove it or use a separate mask token",
                    child = entry.child_kind,
                ),
            ),
            ChildSite::Node => (
                "node.unsupported_child",
                unsupported_child_message(
                    &subject,
                    &entry.child_kind,
                    &entry.parent_kind,
                    entry.allowed,
                ),
            ),
        };
        diagnostics.push(Diagnostic::error(
            code,
            message,
            entry.source_span,
            entry.parent_id.clone(),
        ));
    }
}
