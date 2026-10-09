//! `node.set`: the inspector's write. Wiring only.
//!
//! - `run` — the command.
//! - `params` — its params.
//! - `geometry` — `x` / `y` / `w` / `h`, mapped as a gesture is.
//! - `tokens` — raw values bound through existing or new tokens.
//! - `text` — the plain span and per-span text.

mod geometry;
mod params;
mod run;
mod text;
mod tokens;

pub(crate) use run::run;
pub(crate) use text::{SpanOut, plain_text, spans_of};
