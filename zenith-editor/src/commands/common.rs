//! Helpers every command shares: params decoding and target resolution.

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::ctx::Ctx;
use crate::error::EditorError;

/// Decode the params of the running command.
pub(crate) fn params<T: DeserializeOwned>(ctx: &Ctx<'_, '_>, raw: Value) -> Result<T, EditorError> {
    serde_json::from_value(raw).map_err(|e| EditorError::invalid_params(ctx.command, e))
}

/// The node a single-node command targets: `id`, else the one selected
/// node.
pub(crate) fn target(ctx: &Ctx<'_, '_>, id: Option<String>) -> Result<String, EditorError> {
    if let Some(id) = id {
        return Ok(id);
    }
    match ctx.session.selection.as_slice() {
        [one] => Ok(one.clone()),
        [] => Err(EditorError::no_selection(ctx.command)),
        [_, _, ..] => Err(EditorError::new(
            "editor.ambiguous_selection",
            format!(
                "'{}' acts on one node, and {} are selected; pass an id",
                ctx.command,
                ctx.session.selection.len()
            ),
        )),
    }
}

/// The nodes a multi-node command targets: `ids`, else the selection.
pub(crate) fn targets(
    ctx: &Ctx<'_, '_>,
    ids: Option<Vec<String>>,
) -> Result<Vec<String>, EditorError> {
    let ids = ids.unwrap_or_else(|| ctx.session.selection.clone());
    if ids.is_empty() {
        return Err(EditorError::no_selection(ctx.command));
    }
    Ok(ids)
}

/// The 0-based index of 1-based `page` (default: the session page) in a
/// document of `count` pages.
pub(crate) fn page_index(
    ctx: &Ctx<'_, '_>,
    page: Option<usize>,
    count: usize,
) -> Result<usize, EditorError> {
    let page = page.unwrap_or(ctx.session.page);
    if page == 0 || page > count {
        return Err(EditorError::new(
            "render.page_out_of_range",
            format!(
                "page {page} out of range; the document has {count} page(s); pass 1 to {count}"
            ),
        ));
    }
    Ok(page - 1)
}
