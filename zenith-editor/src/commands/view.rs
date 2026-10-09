//! `view.set {page?, zoom?, pan_x?, pan_y?}`: the page's view state.

use serde::Deserialize;
use serde_json::{Value, json};

use super::common::params;
use crate::ctx::Ctx;
use crate::error::EditorError;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ViewParams {
    #[serde(default)]
    page: Option<usize>,
    #[serde(default)]
    zoom: Option<f64>,
    #[serde(default)]
    pan_x: Option<f64>,
    #[serde(default)]
    pan_y: Option<f64>,
}

/// Set the current page and the viewport. A page must exist in the
/// display text; a zoom must be finite and `> 0`; a pan must be finite.
pub(crate) fn run(ctx: &mut Ctx<'_, '_>, raw: Value) -> Result<Value, EditorError> {
    let p: ViewParams = params(ctx, raw)?;
    if let Some(page) = p.page {
        let count = match ctx.session.display_text().map(str::to_owned) {
            Some(text) => ctx.parse(&text).map_or(0, |doc| doc.body.pages.len()),
            None => 0,
        };
        if page == 0 || page > count {
            return Err(EditorError::new(
                "render.page_out_of_range",
                format!(
                    "page {page} out of range; the document has {count} page(s); pass 1 to \
                     {count}"
                ),
            ));
        }
        ctx.session.page = page;
    }
    if let Some(zoom) = p.zoom {
        if !(zoom.is_finite() && zoom > 0.0) {
            return Err(EditorError::new(
                "editor.invalid_params",
                format!("zoom {zoom} is not a finite number greater than 0"),
            ));
        }
        ctx.session.viewport.zoom = zoom;
    }
    for (value, slot) in [
        (p.pan_x, &mut ctx.session.viewport.pan_x),
        (p.pan_y, &mut ctx.session.viewport.pan_y),
    ] {
        if let Some(v) = value {
            if !v.is_finite() {
                return Err(EditorError::new(
                    "editor.invalid_params",
                    format!("pan {v} is not a finite number"),
                ));
            }
            *slot = v;
        }
    }
    Ok(json!({ "page": ctx.session.page, "viewport": ctx.session.viewport }))
}
