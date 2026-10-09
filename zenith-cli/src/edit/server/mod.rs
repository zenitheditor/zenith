//! The `zenith edit` HTTP server. Wiring only.
//!
//! - `run` — [`EditServer`] and [`EditOptions`]: bind, threads, shutdown.
//! - `routes` — one connection: checks, routing.
//! - `api` — the `/api/*` handlers.
//! - `guard` — token, `Host`, and `Origin` checks.
//! - `events` — the Server-Sent Events fan-out.
//! - `images` — recent renders by SHA-256.
//! - `assets` — the embedded editor page.
//! - `shared` — the state every thread reaches.

mod api;
mod assets;
mod events;
mod guard;
mod images;
mod routes;
mod run;
mod shared;

pub use run::{EditOptions, EditServer};
