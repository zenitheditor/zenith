//! Print preflight over one compiled page. Wiring only.
//!
//! - `run` — the entry point and the print-page gate.
//! - `owner` — which node drew each scene command, as a message subject.
//! - `ppi` — `image.effective_ppi` and `image.effective_ppi_target`.
//! - `ink` — `ink.rich_black` and `ink.total_coverage`.
//! - `font` — `font.not_embeddable`.
//! - `fmt` — number formatting for the messages.

mod fmt;
mod font;
mod ink;
mod owner;
mod ppi;
mod run;

pub(in crate::compile::lint) use run::{PrintInput, print_preflight};
