//! Document-level compile session: prepare a document once, compile any page.
//!
//! [`DocumentPrep`] runs data binding, markdown resolution, import scopes, and
//! token resolution once. [`PageCompiler`] borrows it and builds the
//! document-wide lookups and pre-passes once (auto-layout lowering first).
//! Wiring only.

mod compile;
mod fonts;
mod label_contrast;
mod layout;
mod page;
mod prep;

pub use layout::LayoutStats;
pub use page::PageCompiler;
pub use prep::DocumentPrep;
