//! `connector` leaf-node compilation — a semantic arrow whose endpoints are
//! derived from the resolved boxes of its `from`/`to` targets.
//!
//! Wiring only: the per-concern logic lives in the submodules.
//! - `anchor`: anchor-point geometry (grid / `auto` / divided `i/N` resolution).
//! - `route`: orthogonal + self-loop path geometry, midpoint/point sampling, arrowheads.
//! - `label`: the connector's optional owned label (synthesized text node).
//! - `compile`: the top-level compiler and its `connector.*` diagnostics.

mod anchor;
mod compile;
mod label;
mod route;

pub(in crate::compile) use compile::{ConnectorEnv, compile_connector};
