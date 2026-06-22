//! Theme synthesis — derive a complete design-token palette from a few brand
//! colors, choosing readable foregrounds via APCA (WCAG 3) contrast.
//!
//! This is the engine side of `zenith theme`: it owns the colour math and the
//! token contract, so every front end (CLI, future GUI) produces identical,
//! deterministic palettes. Emitting `.zen` source is the caller's job.

pub mod synth;

pub use synth::{PALETTE_ORDER, PaletteSpec, Rgb, Scheme, synth_palette};
