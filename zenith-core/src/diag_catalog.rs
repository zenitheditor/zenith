//! The single source of truth for every diagnostic code the engine can emit.
//!
//! This catalog is hand-maintained and drift-guarded (mirroring [`crate::schema`]'s
//! node-kind tables). It drives BOTH:
//! - the diagnostic-policy validation in [`crate::validate()`] (which codes are
//!   *governable* by a `diagnostics { … }` block, and which are always Errors), and
//! - the `zenith schema diagnostics` surface (re-exposed through [`crate::schema`]).
//!
//! ## Adding a new diagnostic code
//!
//! Any new diagnostic code emitted anywhere in the workspace **MUST** be added to
//! the catalog (one of the `codes_*` group modules), with its real
//! [`crate::diagnostics::Severity`] and a one-line summary. A code that is not in
//! this catalog is treated as *unknown* by the policy validator: a
//! `diagnostics { … }` entry naming it produces `policy.unknown_code`.
//!
//! ## Governable vs. always-Error
//!
//! A code is **governable** when its catalog severity is `Warning` or `Advisory`:
//! an `allow`/`deny`/`warn` entry can adjust how it is reported. A code whose
//! catalog severity is `Error` is **always-Error** and immutable — `allow`/`warn`
//! cannot weaken it (the validator emits `policy.ineffective_on_error`); a `deny`
//! on it is a silent no-op (it is already an Error).
//!
//! Submodules: `entry` (the [`DiagnosticCodeInfo`] type, verbs, and constructor),
//! `codes_a`/`codes_b`/`codes_c` (the catalog entries, grouped by namespace for
//! file-size hygiene), and `catalog` (assembly + `lookup`).

mod catalog;
mod codes_a;
mod codes_b;
mod codes_c;
mod entry;

pub use catalog::{DIAGNOSTIC_CODES, lookup};
pub use entry::{DIAGNOSTIC_VERBS, DiagnosticCodeInfo};
