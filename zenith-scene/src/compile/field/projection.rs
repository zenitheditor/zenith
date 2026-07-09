//! Master-page projection helpers: document/page-wide node indexing for
//! `page-ref` resolution, per-page node-box collection for text-runaround
//! exclusion, and the page live-area computation that mirrors the validator's
//! margin formula.
//!
//! Wiring only; the concerns live in submodules:
//! - `common` — shared node-id extraction and imported-component resolution.
//! - `page_index` — document-wide `node id → page index` map.
//! - `node_boxes` — per-page absolute node-box collection.
//! - `ports` — connector port-map builders.
//! - `connector_targets` — connector-target shape families and outline boxes.
//! - `live_area` — the page live-area (margin) computation.

mod common;
mod connector_targets;
mod live_area;
mod node_boxes;
mod page_index;
mod ports;

pub(in crate::compile) use connector_targets::{ConnectorTargetKind, build_connector_targets};
pub(in crate::compile) use live_area::compute_live_area;
pub(in crate::compile) use node_boxes::build_node_boxes;
pub(in crate::compile) use page_index::build_page_index_map;
pub(in crate::compile) use ports::{PortTarget, build_port_map};
