//! Brand-contract AST type.
//!
//! A `brand { … }` block (sibling of `diagnostics { … }` inside `zenith { … }`)
//! declares the set of approved colors, font families, and font weights for the
//! document. Absent child nodes mean "unconstrained" for that category; an absent
//! `brand` block altogether is an identity pass (no brand checks).

use super::Span;

/// The brand contract parsed from a root `brand { … }` block.
///
/// Each field is an `Option<Vec<_>>`:
/// - `None` means the child node was absent → that category is unconstrained.
/// - `Some(vec)` means the child node was present → the vec lists the approved
///   values for that category (may be empty if the author wrote, e.g., `colors`
///   with no arguments, which means NO color is approved).
///
/// An absent `brand { … }` block is represented as the `Default` (all `None`),
/// which is identical to [`BrandContract::is_empty`] returning `true`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BrandContract {
    /// Approved color hex strings (case-normalised to lowercase at parse time),
    /// or `None` when the `colors` child was absent (unconstrained).
    pub allowed_colors: Option<Vec<String>>,
    /// Approved font-family names, or `None` when the `fonts` child was absent
    /// (unconstrained).
    pub allowed_fonts: Option<Vec<String>>,
    /// Approved font weights (integers in 100..=900), or `None` when the
    /// `weights` child was absent (unconstrained).
    pub allowed_weights: Option<Vec<u32>>,
    /// Source span of the `brand { … }` node, when available.
    pub source_span: Option<Span>,
}

impl BrandContract {
    /// True when no category is constrained — i.e. the `brand` block was absent
    /// or declared with no children that the engine recognises.
    ///
    /// The formatter uses this to decide whether to emit the block at all:
    /// when `is_empty()` returns `true`, nothing is emitted, preserving
    /// byte-identical output for documents that have no brand contract.
    pub fn is_empty(&self) -> bool {
        self.allowed_colors.is_none()
            && self.allowed_fonts.is_none()
            && self.allowed_weights.is_none()
    }
}
