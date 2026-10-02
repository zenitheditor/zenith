//! Document-level compile session: prepare a document once, compile any page.
//!
//! [`DocumentPrep`] runs data binding, markdown resolution, import scopes, and
//! token resolution once. [`PageCompiler`] borrows it and builds the
//! document-wide lookups and pre-passes once. Wiring only.

mod compile;
mod fonts;
mod page;
mod prep;

pub use page::PageCompiler;
pub use prep::DocumentPrep;
