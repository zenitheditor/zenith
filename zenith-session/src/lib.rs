//! zenith-session: local-machine history/session state for `.zen` documents.
//!
//! Pure crate with injected fs/clock/rng adapters; never depended on by the
//! deterministic render pipeline.
//!
//! # Module layout
//!
//! - [`adapter`] — injectable trait boundaries (filesystem, clock, RNG)
//! - [`datadir`] — platform data-directory resolution
//! - [`docid`] — ULID document-identity minting
//! - [`error`] — [`SessionError`] (the single error type for this crate)
//! - [`layout`] — [`StorePaths`] pure path builders

pub mod adapter;
pub mod datadir;
pub mod docid;
pub mod error;
pub mod layout;

pub use datadir::{resolve_data_dir, resolve_data_dir_with};
pub use docid::mint_ulid;
pub use error::SessionError;
pub use layout::StorePaths;
