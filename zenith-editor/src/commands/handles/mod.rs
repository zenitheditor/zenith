//! `node.handles {id?, ids?, rotate_offset?}`. Wiring and the dispatcher
//! edge only.
//!
//! - `node` — one node's handles (`id`, default the one selected node).
//! - `selection` — a selection's box and grips (`ids`, two or more).
//! - `blocks` — what a plain drag cannot do, and the handle records.

mod blocks;
mod node;
mod selection;

use serde::Deserialize;
use serde_json::Value;

use super::common::params;
use crate::ctx::Ctx;
use crate::error::EditorError;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HandlesParams {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    ids: Option<Vec<String>>,
    #[serde(default = "default_offset")]
    rotate_offset: f64,
}

fn default_offset() -> f64 {
    24.0
}

/// `node.handles`: `ids` with two or more distinct nodes gives the
/// selection's handles; otherwise one node's (`id`, the one id of `ids`,
/// or the one selected node).
pub(crate) fn run(ctx: &mut Ctx<'_, '_>, raw: Value) -> Result<Value, EditorError> {
    let p: HandlesParams = params(ctx, raw)?;
    match (p.id, p.ids) {
        (Some(_), Some(_)) => Err(EditorError::new(
            "editor.invalid_params",
            "'node.handles' takes id or ids, not both",
        )),
        (None, Some(ids)) => selection::run(ctx, ids, p.rotate_offset),
        (id, None) => node::run(ctx, id, p.rotate_offset),
    }
}
