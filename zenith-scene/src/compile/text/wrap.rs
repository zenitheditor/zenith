//! The single-box WRAP path: greedy cross-span word packing with the optional
//! drop-cap, text-runaround, hyphenation/break-word, and bullet/hanging-indent
//! behaviours. Lifted verbatim out of `compile_text_sized` so each concern lives
//! in a focused unit; the emitted command stream is byte-identical to before.
//!
//! Wiring only: `types` holds the borrowed [`WrapEnv`]/[`WrapGeom`] bundles,
//! `paths` holds the dispatcher and the drop-cap/runaround/plain emit sub-paths.

mod paths;
mod types;

#[cfg(test)]
mod tests;

pub(in crate::compile) use paths::emit_wrap_path;
pub(in crate::compile) use types::{WrapEnv, WrapGeom, WrapOutcome};
