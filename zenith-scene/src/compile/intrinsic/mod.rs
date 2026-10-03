//! Intrinsic-size measurement for the auto-layout engine ([`crate::layout`]).
//!
//! The layout engine sizes `hug` children from their content. This module
//! measures that content through the production compile paths (text wrap,
//! code, table sizing, shape labels), so a measured size matches the render.
//! Wiring only.
//!
//! - `env` — [`IntrinsicEnv`], the borrowed measure environment.
//! - `measure` — natural width and height-at-width per node kind.
//! - `lower` — lowering of master projections and instance expansions.

mod env;
mod lower;
mod measure;

pub(crate) use env::IntrinsicEnv;
pub(in crate::compile) use lower::lower_expanded;
