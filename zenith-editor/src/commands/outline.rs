//! `doc.outline`: the pages and layers tree.

use serde::Serialize;
use serde_json::Value;
use zenith_core::{Node, dim_to_px};

use crate::ctx::Ctx;
use crate::doc::tree::child_lists;
use crate::error::EditorError;
use crate::wire::to_json;

/// One layer: a node and its children, in source (paint) order.
#[derive(Debug, Serialize)]
struct Layer {
    /// The node id; `None` for an unknown node without one.
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    /// The node kind (`rect`, `group`, …).
    kind: &'static str,
    /// The authored display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    /// `false` when the node has `visible=#false`.
    visible: bool,
    /// `true` when the node itself has `locked=#true`.
    locked: bool,
    /// `true` for a construction guide (`role="guide"`): never drawn.
    guide: bool,
    children: Vec<Layer>,
}

#[derive(Debug, Serialize)]
struct PageOutline {
    /// The 1-based page number.
    page: usize,
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    master: Option<String>,
    /// Width in px, when it resolves.
    #[serde(skip_serializing_if = "Option::is_none")]
    w: Option<f64>,
    /// Height in px, when it resolves.
    #[serde(skip_serializing_if = "Option::is_none")]
    h: Option<f64>,
    children: Vec<Layer>,
}

#[derive(Debug, Serialize)]
struct MasterOutline {
    id: String,
    children: Vec<Layer>,
}

#[derive(Debug, Serialize)]
struct Outline {
    stale: bool,
    pages: Vec<PageOutline>,
    masters: Vec<MasterOutline>,
}

fn layers(nodes: &[Node]) -> Vec<Layer> {
    nodes
        .iter()
        .map(|node| Layer {
            id: node.id().map(str::to_owned),
            kind: node.kind_str(),
            name: node.name().map(str::to_owned),
            visible: node.is_visible(),
            locked: node.is_locked(),
            guide: node.role() == Some("guide"),
            children: child_lists(node).into_iter().flat_map(layers).collect(),
        })
        .collect()
}

/// The layers of every page and master of the display text (the last
/// valid text while the current one has errors; then `stale: true`).
pub(crate) fn run(ctx: &mut Ctx<'_, '_>, _raw: Value) -> Result<Value, EditorError> {
    let display = ctx.display()?;
    let doc = &display.doc;
    let outline = Outline {
        stale: display.stale,
        pages: doc
            .body
            .pages
            .iter()
            .enumerate()
            .map(|(i, p)| PageOutline {
                page: i + 1,
                id: p.id.clone(),
                name: p.name.clone(),
                master: p.master.clone(),
                w: dim_to_px(p.width.value, &p.width.unit),
                h: dim_to_px(p.height.value, &p.height.unit),
                children: layers(&p.children),
            })
            .collect(),
        masters: doc
            .masters
            .iter()
            .map(|m| MasterOutline {
                id: m.id.clone(),
                children: layers(&m.children),
            })
            .collect(),
    };
    to_json(&outline)
}
