//! `gesture.preview` and `gesture.commit`.

use serde_json::{Value, json};
use zenith_tx::{Permissions, Transaction, TxStatus};

use super::common::params;
use super::render::{ViewportParams, insert_region, scale};
use crate::ctx::{Ctx, ImageRegion, RenderedImage};
use crate::edit::offers::for_rejection;
use crate::edit::ops::{OpsEdit, apply_ops, rejected};
use crate::error::EditorError;
use crate::gesture::multi::union_of_raw;
use crate::gesture::snap::Aabb;
use crate::gesture::{GestureParams, corners_of, plan};
use crate::wire::to_json;

/// Show where a gesture would put the node, without changing the text.
///
/// Maps the gesture to ops exactly as `gesture.commit` does, runs them on
/// a copy of the document, compiles the node's page with lint off, and
/// (with `render`, the default) rasterizes it at `scale` (default: the
/// viewport zoom), or only its `viewport` window (page px) as `doc.render`
/// does. The reply has the ops and the node's page corners after
/// the gesture; a rejection is the error `gesture.commit` would give, with
/// the same offers. The session is unchanged.
///
/// It costs a transaction run, two compiles, and a raster: keep one in
/// flight and send only the newest pointer position (the page does).
pub(crate) fn preview(ctx: &mut Ctx<'_, '_>, raw: Value) -> Result<Value, EditorError> {
    let p: GestureParams = params(ctx, raw.clone())?;
    let requested = p.scale.unwrap_or(ctx.session.viewport.zoom);
    let raster = match (p.render, p.viewport) {
        (false, _) => None,
        // A viewport render checks the scale against the region limits.
        (true, Some(_)) => Some(requested),
        (true, None) => Some(scale(requested)?),
    };
    let doc = ctx.editable()?;
    let planned = plan(ctx, &doc, &p, &raw)?;
    let ids = planned.ids();
    let raw_ids: Vec<String> = planned
        .targets
        .iter()
        .map(|t| t.page.raw_id.clone())
        .collect();
    let Some(page) = planned.targets.first().map(|t| t.page.clone()) else {
        return Err(EditorError::no_selection(ctx.command));
    };
    let tx = Transaction {
        ops: planned.ops,
        permissions: Permissions::default(),
    };
    let result = ctx.run_tx(&doc, &tx)?;
    if result.status == TxStatus::Rejected {
        let offers = for_rejection(ctx.command, &raw, &result.diagnostics, true);
        return Err(rejected(
            ctx.command,
            &result.diagnostics,
            &ctx.session.text,
            offers,
        ));
    }
    let viewport = p.viewport.map(ViewportParams::page_rect);
    let view = ctx.view(&result.document_after, page.index, raster, viewport)?;
    let mut reply = match (ids.as_slice(), raw_ids.as_slice()) {
        ([id], [raw_id]) => json!({
            "node": id,
            "page": page.index + 1,
            "ops": tx.ops,
            "corners": corners_of(&view.boxes, raw_id),
            "notes": planned.notes,
        }),
        _ => {
            let refs: Vec<&str> = raw_ids.iter().map(String::as_str).collect();
            let members: Vec<Value> = ids
                .iter()
                .zip(&raw_ids)
                .map(|(id, raw_id)| json!({"id": id, "corners": corners_of(&view.boxes, raw_id)}))
                .collect();
            json!({
                "nodes": ids,
                "page": page.index + 1,
                "ops": tx.ops,
                "corners": union_of_raw(&view.boxes, &refs).map(Aabb::corners),
                "members": members,
                "notes": planned.notes,
            })
        }
    };
    if let (Some(snap), Value::Object(map)) = (&planned.snap, &mut reply) {
        map.insert("snap".to_owned(), to_json(snap)?);
    }
    if let (Some(image), Value::Object(map)) = (view.image, &mut reply) {
        let region = match (raster, p.viewport) {
            (Some(scale), Some(_)) => Some(ImageRegion {
                x: image.rect.x,
                y: image.rect.y,
                scale,
                device_width: image.device_size.0,
                device_height: image.device_size.1,
                page_width: image.page_size.0,
                page_height: image.page_size.1,
            }),
            _ => None,
        };
        let image = RenderedImage {
            png: image.png,
            width: image.width,
            height: image.height,
            page: page.index + 1,
            region,
        };
        map.insert("width".to_owned(), json!(image.width));
        map.insert("height".to_owned(), json!(image.height));
        map.insert("sha256".to_owned(), json!(image.sha256()));
        if let Some(region) = image.region {
            insert_region(map, region, image.width, image.height);
        }
        ctx.image = Some(image);
    }
    Ok(reply)
}

/// Apply a gesture: map it to ops (see [`plan`](crate::gesture::plan)),
/// run them, and patch the text. The node stays selected. The request must
/// carry the version the gesture was made against.
pub(crate) fn commit(ctx: &mut Ctx<'_, '_>, raw: Value) -> Result<Value, EditorError> {
    let p: GestureParams = params(ctx, raw.clone())?;
    let doc = ctx.editable()?;
    let planned = plan(ctx, &doc, &p, &raw)?;
    let ids = planned.ids();
    let ops = planned.ops;
    let notes = planned.notes;
    apply_ops(
        ctx,
        &doc,
        OpsEdit {
            label: None,
            ops,
            permissions: Permissions::default(),
            selection: Some(ids),
            notes,
            gesture: true,
            raw,
        },
    )
}
