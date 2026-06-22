//! KDL-node-tree → Zenith AST transform.
//!
//! All fallible helpers return `Result<_, ParseError>` so no `.unwrap()` or
//! `.expect()` appears anywhere in this module tree.
//!
//! Wiring only: submodules carry the logic, grouped by node cohesion.
//! - [`helpers`]: shared span/value-extraction helpers.
//! - [`document`]: the top-level [`transform`] entry plus the document-level
//!   structural blocks (project/assets/libraries/.../pages).
//! - [`tokens`]: the `tokens { … }` and `styles { … }` blocks.
//! - [`node`]: the per-node-kind dispatch edge ([`node::transform_node`]).
//! - [`leaf`]/[`container`]/[`special`]: the renderable node transforms.

mod container;
mod document;
mod helpers;
mod leaf;
mod node;
mod pattern;
mod special;
mod tokens;

pub use document::transform;
