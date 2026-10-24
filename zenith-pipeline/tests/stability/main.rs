//! The format-stability promise, held by a frozen corpus of `.zen` documents.
//!
//! - `corpus`: every frozen document parses, validates without Error, formats
//!   idempotently, and renders to its pinned PNG hashes.
//! - `changelog`: the corpus freeze has a matching CHANGELOG.md section.
//!
//! README.md `## Stability` states the promise and the process for a break.

mod changelog;
mod corpus;
#[path = "../common/disk_fs.rs"]
mod disk_fs;
mod frozen;
