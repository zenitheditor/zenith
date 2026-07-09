//! Capture record for child KDL nodes that a node kind does not consume.
//!
//! When a document authors a child node under a kind whose transform never
//! reads it (e.g. `ellipse { text … }`), the child is dropped at parse time.
//! Rather than add a field to every one of the ~20 node structs, the parser
//! records each dropped child in a single document-level side table
//! ([`crate::ast::Document::unsupported_children`]). Validation then reports one
//! `node.unsupported_child` Warning per entry, with the child's span, so the
//! silent data loss becomes visible.

use super::Span;

/// One authored child KDL node that its parent kind does not consume, captured
/// at parse time and reported from validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedChild {
    /// The parent node's `id`, when it declared one.
    pub parent_id: Option<String>,
    /// The parent node's KDL kind (e.g. `"ellipse"`).
    pub parent_kind: String,
    /// The dropped child node's KDL kind (e.g. `"text"`).
    pub child_kind: String,
    /// The dropped child's source span, for a precise diagnostic location.
    pub source_span: Option<Span>,
}
