//! The `zenith edit` HTTP server. Wiring only.
//!
//! - `run` — [`EditServer`], [`EditOptions`], [`StopHandle`]: bind,
//!   threads, shutdown, stop signals.
//! - `routes` — one connection: checks, routing.
//! - `api` — the `/api/*` handlers.
//! - `guard` — token, `Host`, and `Origin` checks.
//! - `events` — the Server-Sent Events fan-out.
//! - `images` — recent renders by SHA-256.
//! - `assets` — the embedded editor page.
//! - `shared` — the state every thread reaches.
//! - `slots` — the cap on open connections.

mod api;
mod assets;
mod events;
mod guard;
mod images;
mod routes;
mod run;
mod shared;
mod slots;

#[cfg(feature = "http")]
pub(crate) use guard::{Token, split_host};
pub use run::{EditOptions, EditServer, Interrupt, StopHandle};
