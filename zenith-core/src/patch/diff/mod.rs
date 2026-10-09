//! Tree diff: walk the source KDL tree beside the canonical before/after
//! trees and record the minimal source edits.
//!
//! The canonical trees come from formatting the before and after documents.
//! A node whose canonical before and after texts are equal is never touched.
//! A changed node is matched to its source node by name, `id`, and position
//! among same-key siblings, so the source can order nodes differently from
//! the canonical form.
//!
//! Structural edits patch in place too:
//! - a removed node loses its lines, its trailing comment, and the comment
//!   lines attached above it ([`super::text::attach_start`]),
//! - a new node takes its canonical text at its sibling position,
//! - a moved node (reorder, reparent, group, ungroup) carries its own source
//!   text and attached comments to the new site, re-indented.

mod differ;
mod insert;
mod key;
mod nodes;
mod order;
mod remove;

pub(super) use differ::{Differ, Layout, Texts};
pub(super) use key::Index;
pub(super) use nodes::{entry_range, name_end};
