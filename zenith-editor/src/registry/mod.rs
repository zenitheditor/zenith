//! The command registry. Wiring only.
//!
//! - `spec` — [`CommandSpec`] and the enabled rules.
//! - `table` — the table of every command.

mod spec;
mod table;

pub use spec::{CommandSpec, Disabled};
pub use table::{commands, find};
