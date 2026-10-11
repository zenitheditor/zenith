//! `zenith new` — scaffold a fresh, valid `.zen` document.
//!
//! Wiring only: the scaffolding logic lives in the private `scaffold` submodule,
//! the page-size resolution (named formats, explicit dimensions, orientation,
//! page count) in [`page`], the theme print scale in `print_scale`, and the
//! finished `--starter` layouts in `starter`.

pub mod page;
mod print_scale;
mod scaffold;
mod starter;

pub use page::{DEFAULT_PAGE, PageSpec, PaperFormat, resolve_page};
pub use scaffold::{NewErr, NewResult, run, run_in};
pub use starter::{
    Slot, Starter, StarterSize, run_starter, run_starter_in, slot_listing, starter_document,
    starter_page, text_slots,
};
