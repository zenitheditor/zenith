//! Static schema metadata for the authorable node kinds and non-node surfaces.
//!
//! Exposes the canonical list of node kinds, one-line summaries, and the
//! recognized attribute names for each kind. The attribute list is derived
//! directly from the parser's own `known_props_for_kind` table so the two
//! can never silently diverge.
//!
//! Also exposes `page_attributes`, `asset_attributes`, and
//! `document_attributes` for the three non-node authorable surfaces, derived
//! from the same parser-side `PAGE_KNOWN_PROPS`, `ASSET_KNOWN_PROPS`, and
//! `DOCUMENT_KNOWN_PROPS` constants.
//!
//! Token-type schema (`token_types`, `token_type_summary`, `token_type_descriptor`)
//! mirrors the node-kind surface and provides agent-readable value-form, child-node
//! structure, and minimal correct examples for every authorable token type.
//!
//! Submodules: `kinds` (node-kind list/summaries/examples), `content`
//! (child-content descriptors), `attributes` (attribute name lists + type
//! hints), `tokens` (token-type schema), `surfaces` (variant/diagnostics
//! surfaces).

mod attributes;
mod content;
pub(crate) mod enums;
mod kinds;
mod surfaces;
mod tokens;

pub use attributes::{
    asset_attributes, asset_summary, attribute_type, attribute_type_for_kind, document_attributes,
    document_summary, node_attributes, page_attributes, page_summary,
};
pub use content::{NodeContentDescriptor, node_content};
pub use kinds::{node_example, node_kinds, node_summary};
pub use surfaces::{
    PortsDescriptor, VariantDescriptor, diagnostic_codes, diagnostics_summary, diagnostics_verbs,
    ports_descriptor, variant_descriptor,
};
pub use tokens::{TokenTypeDescriptor, token_type_descriptor, token_type_summary, token_types};
