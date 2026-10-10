//! Changing the text. Wiring only.
//!
//! - `text` — [`change_text`](text::change_text), the one path every text
//!   change takes, and the edit reply.
//! - `ops` — run a transaction and patch its result into the text.
//! - `offers` — the follow-up commands a rejection offers.
//! - `comments` — the comments an edit dropped.

pub(crate) mod comments;
pub(crate) mod offers;
pub(crate) mod ops;
pub(crate) mod text;
