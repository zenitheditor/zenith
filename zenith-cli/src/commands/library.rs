//! Pure logic for `zenith library list`, `zenith library show`, and `zenith library add`.
//!
//! The registry/resolver lives in [`crate::library`]; this module turns a
//! resolved set of packs into stdout text ([`list`]), inspects individual items
//! ([`show`]), and materializes library items into target documents ([`add`]).
//! None of these functions touch the filesystem — the dispatcher reads/writes
//! files and calls [`crate::library::resolve_packs`].
//!
//! Submodules:
//! - [`list`] — `zenith library list`
//! - [`show`] — `zenith library show`
//! - [`add`] — `zenith library add`

mod add;
mod list;
mod search;
mod show;

pub use add::{AddCmdErr, AddResult, add};
pub use list::list;
pub use search::{
    DEFAULT_LIMIT as DEFAULT_SEARCH_LIMIT, Filter as SearchFilter, SearchOptions, search,
};
pub use show::{ShowCmdErr, show};
