//! Pure logic for `zenith workspace scratch` and `zenith workspace candidate`.
//!
//! Submodules:
//! - [`scratch`] — `zenith workspace scratch new/list/show`
//! - [`candidate`] — `zenith workspace candidate` (set lifecycle status)

mod candidate;
mod scratch;

pub use candidate::{candidate_set_status, candidate_set_status_in};
pub use scratch::{
    scratch_list, scratch_list_in, scratch_new, scratch_new_in, scratch_show, scratch_show_in,
};
