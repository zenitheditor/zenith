//! Capture record for child KDL nodes that a node kind does not consume.
//!
//! When a document authors a child node under a kind whose transform never
//! reads it (e.g. `ellipse { text … }`), the child is dropped at parse time.
//! Rather than add a field to every one of the ~20 node structs, the parser
//! records each dropped child in a single document-level side table
//! ([`crate::ast::Document::unsupported_children`]). Validation then reports one
//! `node.unsupported_child` Error per entry, with the child's span, so the
//! silent data loss becomes visible.
//!
//! The same table also records unknown child nodes inside structural blocks
//! (`brand`, `assets`, `table` rows, and similar). Those entries carry the
//! block's allowed child names in [`UnsupportedChild::allowed`] and validation
//! reports them as `block.unknown_child` Errors. A `mask` token's extra shape
//! children are recorded too and reported as `token.mask_extra_shape` Errors.

use super::Span;

/// Where an [`UnsupportedChild`] was found, which selects its diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildSite {
    /// Child of a renderable node kind: `node.unsupported_child`.
    Node,
    /// Unknown child of a structural block: `block.unknown_child`.
    Block,
    /// Second or later recognized shape child of a `mask` token:
    /// `token.mask_extra_shape`.
    ExtraMaskShape,
}

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
    /// Which kind of parent dropped the child; selects the diagnostic code.
    pub site: ChildSite,
    /// Every child name the parent accepts, used for the did-you-mean. Empty
    /// when the parent takes no children or has no name list.
    pub allowed: &'static [&'static str],
}
