//! `zenith inspect` command — module wiring.
//!
//! - `document`    — error type, tree types, human renderers, and the public
//!   `run` / `summary` entry points.
//! - `tree`        — tree builders, node finder, authored-geometry helpers.
//! - `boxes`       — resolved page-absolute node boxes after auto-layout.
//! - [`path`]        — `zenith inspect path` topology / bounds / craft.
//! - [`recipes`]     — recipe-block JSON builder and human renderer.
//! - `sections`    — the masters, components, and styles outline sections.

mod boxes;
mod document;
pub mod path;
pub mod recipes;
mod sections;
mod tree;

pub use boxes::{BoxInfo, NodeBox, resolved_boxes};
pub use document::{
    InspectCmdErr, InspectNodeOutput, InspectOutput, NodeEntry, NodeGeometry, OverrideLine,
    PageEntry, clip_text, run, summary,
};
pub use tree::{build_doc_tree, find_node_tree};
