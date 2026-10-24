//! Fallback fonts for glyphs the Zenith bundled faces lack.
//!
//! The chain covers Arabic, Hebrew, Chinese, Japanese, Korean, and emoji.
//! [`fallback_faces`] lists the faces in chain order. A host registers
//! them in that order, after every other face. The crate depends on no
//! other Zenith crate.
//!
//! The `embed` feature (on by default) embeds the font bytes. Without it,
//! each [`FallbackFace::bytes`] is `None` and the host supplies the file.
//! `ABOUT.txt` records the upstream files and the exact subset commands.
//!
//! Every build holds each face's code-point coverage. [`fallback_face_for`]
//! names the face a glyph needs, so a host without the bytes fetches only
//! that file.

mod coverage;
#[cfg(feature = "embed")]
mod embedded;
mod faces;

pub use faces::{FallbackFace, fallback_face_by_file, fallback_face_for, fallback_faces};
