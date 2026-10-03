//! Per-op JSON field schema (name, type hint, required flag).
//!
//! The [`OpFieldSchema`] type and the [`op_fields`] dispatch live here; the
//! per-op field tables are grouped into the `group_a`/`group_b`/`group_c`
//! submodules for file-size hygiene. Op names are unique across the groups, so the chained
//! lookup is order-independent.

mod group_a;
mod group_b;
mod group_c;

/// One JSON field belonging to a transaction op (excluding the `"op"` tag).
#[derive(Debug, Clone, PartialEq)]
pub struct OpFieldSchema {
    /// The JSON key name for this field.
    pub name: &'static str,
    /// Short human/agent-readable type hint, e.g. `"node id"`, `"token ref"`,
    /// `"string"`, `"f64"`, `"bool"`, `"enum: left|center|right"`.
    pub ty: &'static str,
    /// `true` when the field MUST be present; `false` when it may be omitted.
    pub required: bool,
}

/// Return the JSON fields for a named op (excluding the `"op"` tag itself).
///
/// Returns an empty slice for ops that have no fields (none exist in v0, but
/// the signature is consistent). Returns `None` if `name` is not a known op.
pub fn op_fields(name: &str) -> Option<&'static [OpFieldSchema]> {
    group_a::op_fields(name)
        .or_else(|| group_b::op_fields(name))
        .or_else(|| group_c::op_fields(name))
}
