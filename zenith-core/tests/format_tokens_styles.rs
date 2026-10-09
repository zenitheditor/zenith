//! Integration tests for the canonical writer: tokens_styles.
//!
//! Token literals (gradient, shadow, filter, mask, duotone), syntax themes, and
//! the `styles` block — parse, serialize, and round-trip.
//!
//! Moved verbatim from the former in-`src` `format/writer/tests.rs`; the body of
//! every test is unchanged — only import paths were rewritten to the public
//! `zenith_core` surface. Span-stripping uses `zenith_core::strip_spans`.

use zenith_core::format::format_document;
use zenith_core::{KdlAdapter, KdlSource, Node, strip_spans};

#[path = "format_tokens_styles/gradient_theme.rs"]
mod gradient_theme;
#[path = "format_tokens_styles/mask_duotone.rs"]
mod mask_duotone;
#[path = "format_tokens_styles/shadow_filter.rs"]
mod shadow_filter;
#[path = "format_tokens_styles/styles.rs"]
mod styles;
#[path = "format_tokens_styles/token_set.rs"]
mod token_set;
