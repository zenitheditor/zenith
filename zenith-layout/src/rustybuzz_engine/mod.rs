//! `rustybuzz`-backed shaping engine for Zenith.
//!
//! This is the ONLY module in the crate that shapes with `rustybuzz`. No
//! third-party type escapes to a public signature.

mod face_cache;
mod fallback;
mod run;
mod shaper;

pub use face_cache::FontFaceStore;
pub use shaper::RustybuzzEngine;
