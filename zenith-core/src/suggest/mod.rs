//! Shared "did you mean?" helpers for diagnostics and `zenith fix`.
//!
//! - `distance` — edit distance, nearest-name suggestion, token-id ranking.
//! - `hints` — structured `FixHint` builders.
//! - `messages` — hint message builders.
//! - `value` — value-based token matching for raw visual literals.
//!
//! Used by property-name checks (`node.unknown_property`), enum-value checks
//! (`node.invalid_value`), token-reference checks (`token.unknown_reference`,
//! `token.raw_visual_literal`), and the `fix` planner.

mod distance;
mod hints;
mod messages;
mod value;

pub use distance::find_suggestion;
pub use messages::format_candidate_list;

pub(crate) use distance::{common_dotted_prefix, find_token_suggestion};
pub(crate) use hints::{rename_property_fix, replace_token_ref_fix, replace_value_fix};
pub(crate) use messages::{
    invalid_value_message, raw_literal_message, unknown_child_message, unknown_property_message,
    unknown_reference_message, unsupported_child_message,
};
pub(crate) use value::{
    LiteralValue, best_token, dimension_role, exact_token, literal_text, unit_suffix,
};
