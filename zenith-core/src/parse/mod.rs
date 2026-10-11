//! Parse layer re-exports and the `KdlSource` trait.

pub mod kdl_adapter;
pub mod policy;
pub mod transform;
mod unclosed;

pub use kdl_adapter::KdlAdapter;
pub use policy::{parse_brand_contract, parse_diagnostic_policy};

use crate::ast::Document;
use crate::error::{FormatError, ParseError};

/// Contract for a type that can parse and format `.zen` source bytes.
pub trait KdlSource {
    /// Parse raw `.zen` bytes into a `Document` AST.
    ///
    /// # Errors
    ///
    /// Returns a `ParseError` if the bytes are not valid UTF-8, not valid KDL,
    /// or do not conform to the minimal Zenith schema.
    fn parse(&self, source: &[u8]) -> Result<Document, ParseError>;

    /// Serialize a `Document` AST back to canonical `.zen` bytes.
    ///
    /// The output is idempotent: `format(format(doc)) == format(doc)` for all
    /// valid documents. Returns `Err` only for genuinely un-serializable states
    /// (unreachable for valid v0 input).
    fn format(&self, doc: &Document) -> Result<Vec<u8>, FormatError>;
}
