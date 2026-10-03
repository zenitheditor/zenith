//! `zenith theme` — synthesize theme packs and re-skin documents from them.
//!
//! Wiring only: synthesis logic lives in the `new` submodule, re-skin logic
//! in the `apply` submodule, style and `defaults` planning in `blocks`.

mod apply;
mod blocks;
mod new;
mod support;

pub use apply::{ApplyOutcome, SkipReason, SkippedToken, ThemeApplyErr, run as apply_run};
pub use blocks::{BlockSkipReason, SkippedBlockItem};
pub use new::{Shape, ThemeErr, ThemeInput, new};
