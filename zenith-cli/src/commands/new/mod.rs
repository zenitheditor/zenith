//! `zenith new` — scaffold a fresh, valid `.zen` document.
//!
//! Wiring only: the scaffolding logic lives in the private `scaffold` submodule,
//! the page-size resolution (named formats, explicit dimensions, orientation,
//! page count) in [`page`], and the theme print scale in `print_scale`.

pub mod page;
mod print_scale;
mod scaffold;

pub use page::{DEFAULT_PAGE, PageSpec, PaperFormat, resolve_page};
pub use scaffold::{NewErr, NewResult, run, run_in};
