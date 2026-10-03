//! Intrinsic-size measurement for the auto-layout engine ([`crate::layout`]).
//!
//! The layout engine sizes `hug` children from their content. This module
//! measures that content through the production compile paths (text wrap,
//! code, table sizing, shape labels, image asset sizes, instance expansion),
//! so a measured size matches the render. Wiring only.
//!
//! - `env` — [`IntrinsicEnv`], the borrowed measure environment, and the
//!   probe positions of position-dependent text.
//! - `measure` — natural width and height-at-width per node kind.
//! - `instance` — content bounds of an expanded `instance`.
//! - `lower` — lowering of master projections and instance expansions.

mod env;
mod instance;
mod lower;
mod measure;

pub(crate) use env::{ImageSizes, IntrinsicEnv, ProbeAt, ProbeHints};
pub(in crate::compile) use lower::lower_expanded;
