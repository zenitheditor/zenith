//! `zenith new --starter <kind>`: a finished, themeable, resizable layout.
//!
//! Wiring only: the kinds and their natural page specs live in `kind`, the
//! page resolution, theme splice, and size fitting in `build`.

mod build;
mod kind;
mod slots;

pub use build::{StarterSize, run_starter, run_starter_in, starter_document, starter_page};
pub use kind::Starter;
pub use slots::{Slot, slot_listing, text_slots};
