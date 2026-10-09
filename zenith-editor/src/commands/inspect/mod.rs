//! `node.inspect` and `doc.tokens`. Wiring only.
//!
//! - `node` — the `node.inspect` command.
//! - `attrs` — authored attributes from the node's own source.
//! - `edit` — the `node.set` fields a node takes.
//! - `style` — the style the node draws with.
//! - `tokens` — the `doc.tokens` command.

mod attrs;
mod edit;
mod node;
mod style;
mod tokens;

pub(crate) use node::run;
pub(crate) use tokens::run as tokens;
