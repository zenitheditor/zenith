//! One document's editor state, shared by the `zenith edit` server and the
//! MCP `zenith_editor_*` tools. Wiring only.
//!
//! - `target` — [`Target`]: the canonical document path and its root.
//! - `canonical` — `canonicalize`: canonical paths without `\\?\` on Windows.
//! - `confined` — `ConfinedFs`: project reads limited to the root.
//! - `disk` — file stamps and text reads.
//! - `event` — [`Event`]: one live notification.
//! - `image` — [`image_meta`]: the JSON that describes a render.
//! - `state` — [`DocState`]: session, saved text, conflict policy.
//! - `file_cmds` — `file.save`, `file.reload`, `file.state`.

mod canonical;
mod confined;
mod disk;
mod event;
mod file_cmds;
mod image;
mod state;
mod target;

pub(crate) use canonical::canonicalize;
pub(crate) use event::Event;
pub(crate) use image::image_meta;
pub(crate) use state::{DocState, Ran, TextChange};
pub(crate) use target::Target;
