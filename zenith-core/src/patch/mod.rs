//! Format-preserving source patcher.
//!
//! An edit changes the AST. [`patch_source`] writes that change back into the
//! user's source text and keeps comments, blank lines, and layout. Only the
//! changed entries and child lines move. Added, removed, moved, and
//! reparented nodes patch in place, and a moved node keeps its comments.
//! Layouts with no exact in-place edit fall back to canonical text, flagged
//! [`Patched::reformatted`].

mod diff;
mod entries;
mod error;
mod run;
mod text;

pub use error::{PatchError, PatchErrorCode};
pub use run::{Patched, patch_source, try_patch_source};
