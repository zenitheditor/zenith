//! Diagnostic-policy and brand-contract resolution.
//!
//! ## Diagnostic policy
//!
//! The effective policy concatenates four tiers, low → high precedence:
//!
//! 1. **global config** — the host's global config file.
//! 2. **local config** — the nearest `.zenith.kdl` walking up from the
//!    document's directory.
//! 3. **in-file policy** — the document's own `diagnostics { … }` block.
//! 4. **flags** — caller overrides ([`PolicyFlags`]).
//!
//! Resolution is last-wins (see [`zenith_core::DiagnosticPolicy::verb_for`]),
//! so concatenating once yields `flags > in-file > local > global`.
//!
//! ## Brand contract
//!
//! Merged per category (`colors`, `fonts`, `weights`), low → high: global
//! config, local config, in-file `brand { … }`. An absent category in a higher
//! tier does not erase it from a lower one. See
//! [`zenith_core::merge_brand_contract`].

mod flags;
mod layers;

pub use flags::{PolicyFlags, merge_policy};
pub use layers::{ConfigLayers, brand_of, load_policy_layers, policy_of};
