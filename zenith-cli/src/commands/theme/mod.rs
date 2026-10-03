//! `zenith theme` — synthesize theme packs and re-skin documents from them.
//!
//! Wiring only: synthesis logic lives in the `new` submodule, re-skin logic
//! in the `apply` submodule, style and `defaults` planning in `blocks`.

mod apply;
mod blocks;
mod kit;
mod new;
mod support;

pub use apply::{ApplyOutcome, SkipReason, SkippedToken, ThemeApplyErr, run as apply_run};
pub use blocks::{BlockSkipReason, SkippedBlockItem};
pub use kit::{
    DISPLAY_SIZE_TOKEN_ID, DISPLAY_SIZE_TOKEN_PX, DISPLAY_SIZE_TOKEN_TYPE, HEADING_WEIGHT_TOKEN_ID,
    HEADING_WEIGHT_TOKEN_TYPE, HEADING_WEIGHT_TOKEN_VALUE, THEME_DEFAULTS, THEME_STYLE_IDS,
    THEME_STYLES, ThemeDefault, ThemeStyle, kit_document_source,
};
pub use new::{Shape, ThemeErr, ThemeInput, new};
