//! Editor engine tests through the public API: sessions and typing,
//! selection, gestures, structure edits, history, inspection, viewport
//! renders, batches, the showcase loop work counts, and the example sweeps
//! (property, determinism, work counts).

#[path = "../common/mod.rs"]
mod common;

mod batch;
mod gestures;
mod history;
mod inspect;
mod marquee;
mod props;
mod select;
mod session;
mod set;
mod showcase;
mod structure;
mod sweep;
mod viewport;
