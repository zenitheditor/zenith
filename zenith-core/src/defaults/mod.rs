//! Lowering of `defaults { … }` blocks into explicit node values.
//!
//! [`lower`] turns the document and page defaults into per-node attributes
//! (and merged label styles) on a clone of the document, plus content
//! pairing of text fills against their structural backdrop. Scene
//! compilation and contrast validation both read the lowered clone.

mod alias;
mod apply;
mod label;
mod lower;
mod pair;
mod scope;

pub use alias::IdAliases;
pub use lower::{Lowered, lower};
