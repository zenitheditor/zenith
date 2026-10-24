//! Shared chain data types: the per-member assignment result and the collected
//! member descriptor.

use std::collections::BTreeMap;

use zenith_layout::TextDirection;

use crate::compile::text::{Line, WordMetrics};

/// The lines a single chain member must render, already shaped + packed to that
/// member's box width, plus the shared font metrics for baseline stacking.
pub(crate) struct ChainAssignment {
    pub(in crate::compile) lines: Vec<Line>,
    pub(in crate::compile) metrics: WordMetrics,
    /// `true` only for the LAST member of the chain (document-wide). Drives the
    /// justify last-line policy: the final member leaves its last line ragged;
    /// a continuation member justifies its last line (the paragraph flows on).
    pub(in crate::compile) is_last_member: bool,
    /// The chain source's base direction. A member without its own
    /// `direction` aligns its lines with it.
    pub(in crate::compile) direction: TextDirection,
}

/// Map from node id → its assigned chain lines. Empty when the page has no
/// chains. A node whose id is absent is NOT a chain member.
pub(crate) type ChainAssignments = BTreeMap<String, ChainAssignment>;

/// A collected chain member: its node id and the box width/height (px) used to
/// distribute lines. The member's actual draw geometry (x/y/align) is resolved
/// independently inside `compile_text` from the node's own AST, so only the box
/// extents needed for distribution are carried here.
pub(super) struct Member {
    pub(super) id: String,
    pub(super) w: f64,
    pub(super) h: f64,
}
