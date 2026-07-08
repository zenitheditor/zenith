//! Top-level `transform` entry point and the document-level structural blocks
//! (project, assets, libraries, actions, masters, sections, provenance,
//! components, document body, pages, folds, safe-zones).
//!
//! Wiring only: the submodules carry the logic, grouped by block cohesion.
//! - `entry`: the top-level `transform` entry and `DOCUMENT_KNOWN_PROPS`.
//! - `imports`: the `imports { … }` block and token maps.
//! - `policy`: the `diagnostics { … }` lint-policy block.
//! - `brand`: the `brand { … }` contract block.
//! - `structure`: masters, sections, libraries, actions, provenance.
//! - `variants`: the `variants { … }` and `recipes { … }` blocks.
//! - `components`: the `components { … }` block and `project` metadata.
//! - `assets`: the `assets { … }` block and `ASSET_KNOWN_PROPS`.
//! - `body`: the `document { … }` body, pages, and the `transform_children` helper.

mod assets;
mod body;
mod brand;
mod components;
mod entry;
mod imports;
mod policy;
mod structure;
mod variants;

pub(crate) use assets::ASSET_KNOWN_PROPS;
pub(in crate::parse::transform) use body::transform_children;
pub(crate) use brand::transform_brand_contract;
pub(crate) use entry::DOCUMENT_KNOWN_PROPS;
pub use entry::transform;
pub(crate) use policy::transform_diagnostic_policy;
