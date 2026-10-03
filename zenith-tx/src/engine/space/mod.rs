//! Coordinate spaces for tx ops: the translating containers between a page
//! and a child space, and the px shift between two child spaces. Wiring only.
//!
//! Every `frame` and `group` translates its children
//! ([`zenith_core::Node::child_space`]). Ops that move a node between
//! containers, or compare geometry across containers, read the shift here.
//! The test is structural: tx reads authored values the way the scene reads
//! them, with no scene dependency.
//!
//! - `chain` — container chains, shared prefixes, and shifts.
//! - `offset` — the offset one container adds, including anchored layout
//!   frames.

mod chain;
mod offset;

pub(in crate::engine) use chain::{
    Chain, Link, chain_origin, common_prefix, container_chain, parent_chain, parent_frame,
    resolved_tokens, shift_between,
};
