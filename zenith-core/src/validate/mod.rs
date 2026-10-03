//! Document-level semantic validation pass for Zenith.
//!
//! This module wires together the validation submodule and re-exports the
//! public API surface. All logic lives in `check`.
//!
//! # Public API
//!
//! ```rust
//! use zenith_core::{validate, KdlAdapter, KdlSource};
//!
//! let src = r##"zenith version=1 {
//!   project id="p" name="P"
//!   tokens format="zenith-token-v1" { }
//!   styles { }
//!   document id="d" title="D" {
//!     page id="pg" w=(px)100 h=(px)100 { }
//!   }
//! }"##;
//!
//! let doc = KdlAdapter.parse(src.as_bytes()).expect("parses");
//! let report = validate(&doc);
//! assert!(!report.has_errors());
//! ```

mod check;

pub use check::{
    ContentScope, ContentScopes, ContrastInks, GlyphInk, InkLine, LabelInk, ScopeFit, ScopeTokens,
    ValidationReport, apply_policy, layout_geometry_checks, page_contrast_checks, validate,
    validate_with_policy,
};
