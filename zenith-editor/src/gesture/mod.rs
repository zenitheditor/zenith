//! Gestures to ops. Wiring only.
//!
//! - `flags` — the gesture params, confirmation flags, and handle ids.
//! - `kind` — which gesture family a node kind takes.
//! - `target` — the node, its compiled box, and its lock state.
//! - `facts` — what a box node's attributes allow.
//! - `boxplan` — box moves and resizes, axis by axis.
//! - `shapes` — line, polygon / polyline, and path gestures.
//! - `rotate` — rotation.
//! - `reorder` — flow-slot moves inside layout frames.
//! - `member` — one node's part of a gesture.
//! - `refusal` — why a node refuses, and the merged error.
//! - `snap` — snapping to other nodes and the page, with guides.
//! - `multi` — selection gestures (several nodes as one).
//! - `plan` — the dispatcher.

pub(crate) mod boxplan;
pub(crate) mod facts;
pub(crate) mod flags;
pub(crate) mod kind;
mod member;
pub(crate) mod multi;
mod plan;
mod refusal;
mod reorder;
pub(crate) mod rotate;
pub(crate) mod shapes;
pub(crate) mod snap;
pub(crate) mod target;

pub(crate) use flags::GestureParams;
pub(crate) use plan::{corners_of, current_axes, plan};
