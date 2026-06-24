//! Static schema metadata for the transaction op set.
//!
//! Exposes the canonical list of op names (as their JSON `op` tag strings),
//! one-line summaries per op, field-level schema (name, type hint, required
//! flag) per op, a minimal JSON example per op, and compile-time drift guards
//! that force a compile error whenever a new `Op` variant is added without
//! updating this module.
//!
//! Submodules: `names`, `summaries`, `fields`, `examples`. Tests in `tests`.

mod examples;
mod fields;
mod names;
mod summaries;

pub use examples::op_example;
pub use fields::{OpFieldSchema, op_fields};
pub use names::op_names;
pub use summaries::op_summary;

#[cfg(test)]
mod tests;
