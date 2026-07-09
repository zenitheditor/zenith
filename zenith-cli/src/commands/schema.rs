//! Pure logic for `zenith schema`.
//!
//! The public entry points operate entirely on static schema data — no
//! filesystem I/O.  The caller (dispatch) is responsible for printing the
//! returned string and mapping the exit code.
//!
//! Wiring only: the submodules carry the surface logic.
//! - `listings`: the node/op/token catalog listing surfaces.
//! - `surfaces`: the page/asset/document, variant, and diagnostics surfaces.
//! - `brand`: the brand-kit and markdown-block surfaces.
//! - `common`: the shared attribute-table formatter.

mod brand;
mod common;
mod listings;
mod surfaces;

#[cfg(test)]
mod tests;

pub use brand::{block, brand};
pub use listings::{node_detail, nodes, op_detail, ops, overview, token_detail, tokens};
pub use surfaces::{asset, diagnostics, document, page, ports, variant};
