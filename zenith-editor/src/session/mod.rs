//! The serializable session and its history. Wiring only.
//!
//! - `state` — [`Session`] and [`Viewport`].
//! - `delta` — [`TextDelta`], one contiguous text replacement.
//! - `history` — [`History`]: bounded undo / redo of deltas, typing bursts
//!   coalesced.

mod delta;
mod history;
mod state;

pub use delta::TextDelta;
pub use history::{
    DEFAULT_MAX_BYTES, DEFAULT_MAX_ENTRIES, History, HistoryEntry, HistoryLimit, MAX_BURST_BYTES,
};
pub use state::{Session, Viewport};
