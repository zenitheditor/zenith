//! `select.hit` and `select.set`.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use zenith_core::Node;
use zenith_scene::{hit_test_within, selectable_id};

use super::common::{page_index, params};
use crate::ctx::Ctx;
use crate::doc::tree::{Root, exists, locate};
use crate::error::EditorError;
use crate::wire::to_json;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HitParams {
    x: f64,
    y: f64,
    #[serde(default)]
    page: Option<usize>,
    #[serde(default)]
    tolerance: f64,
    #[serde(default)]
    extend: bool,
    #[serde(default = "yes")]
    select: bool,
}

fn yes() -> bool {
    true
}

/// One selectable node under the point.
#[derive(Debug, Serialize)]
struct Hit {
    /// The authored node a click selects.
    id: String,
    /// The compiled id that was hit (`<page-id>/<id>` for master content,
    /// `<instance-id>/<id>` inside an instance, …).
    raw_id: String,
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    locked: bool,
    /// The master that holds the node: editing it changes every page that
    /// uses this master.
    #[serde(skip_serializing_if = "Option::is_none")]
    master: Option<String>,
    /// `instance` or `pattern` when the hit was expanded content that
    /// selects its instance or pattern node.
    #[serde(skip_serializing_if = "Option::is_none")]
    via: Option<&'static str>,
}

/// The selectable nodes under the page point `(x, y)` of 1-based `page`,
/// topmost first, and (with `select`, the default) the new selection.
///
/// Lines, polygons, polylines, paths, and connectors hit on their painted
/// fill and stroke, not their bounds. `tolerance` (page px) gives strokes
/// and thin boxes click slop (see [`hit_test_within`]). Guides never hit;
/// hidden nodes never hit. With `select`, the topmost hit becomes the
/// selection, or with `extend` it toggles in the selection; a miss clears
/// the selection unless `extend`. The page becomes the session page.
pub(crate) fn hit(ctx: &mut Ctx<'_, '_>, raw: Value) -> Result<Value, EditorError> {
    let p: HitParams = params(ctx, raw)?;
    if !(p.x.is_finite() && p.y.is_finite() && p.tolerance.is_finite() && p.tolerance >= 0.0) {
        return Err(EditorError::new(
            "editor.invalid_params",
            "x and y must be finite and tolerance finite and >= 0",
        ));
    }
    let display = ctx.display()?;
    let doc = &display.doc;
    let index = page_index(ctx, p.page, doc.body.pages.len())?;
    let view = ctx.view(doc, index, None, None)?;
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut hits: Vec<Hit> = Vec::new();
    for raw_id in hit_test_within(&view.boxes, p.x, p.y, p.tolerance) {
        let Some(id) = selectable_id(doc, index, raw_id) else {
            continue;
        };
        if seen.contains(id) {
            continue;
        }
        let Some(located) = locate(doc, id) else {
            continue;
        };
        seen.insert(id);
        let master = match located.root {
            Root::Master(m) => doc.masters.get(m).map(|m| m.id.clone()),
            Root::Page(_) => None,
        };
        let expanded = raw_id != id && !raw_id.ends_with(&format!("/{id}"));
        let via = if expanded {
            expansion_kind(located.node)
        } else {
            None
        };
        hits.push(Hit {
            id: id.to_owned(),
            raw_id: raw_id.to_owned(),
            kind: located.node.kind_str(),
            name: located.node.name().map(str::to_owned),
            locked: located.node.is_locked(),
            master,
            via,
        });
    }
    if p.select {
        match hits.first() {
            Some(top) if p.extend => {
                let selection = &mut ctx.session.selection;
                if let Some(at) = selection.iter().position(|s| *s == top.id) {
                    selection.remove(at);
                } else {
                    selection.push(top.id.clone());
                }
            }
            Some(top) => ctx.session.selection = vec![top.id.clone()],
            None if p.extend => {}
            None => ctx.session.selection.clear(),
        }
    }
    ctx.session.page = index + 1;
    Ok(json!({
        "page": index + 1,
        "stale": display.stale,
        "hits": to_json(&hits)?,
        "selection": ctx.session.selection,
    }))
}

/// `instance` or `pattern` for the kinds whose content compiles under
/// expanded ids; `None` for every other kind.
fn expansion_kind(node: &Node) -> Option<&'static str> {
    match node {
        Node::Instance(_) => Some("instance"),
        Node::Pattern(_) => Some("pattern"),
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Line(_)
        | Node::Text(_)
        | Node::Code(_)
        | Node::Frame(_)
        | Node::Group(_)
        | Node::Image(_)
        | Node::Polygon(_)
        | Node::Polyline(_)
        | Node::Path(_)
        | Node::Field(_)
        | Node::Toc(_)
        | Node::Footnote(_)
        | Node::Table(_)
        | Node::Shape(_)
        | Node::Connector(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Mesh(_)
        | Node::Unknown(_) => None,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetParams {
    ids: Vec<String>,
}

/// Set the selection to `ids` (authored node ids, in order; repeats
/// dropped). When the current text parses, every id must name a node.
pub(crate) fn set(ctx: &mut Ctx<'_, '_>, raw: Value) -> Result<Value, EditorError> {
    let p: SetParams = params(ctx, raw)?;
    let text = ctx.session.text.clone();
    if let Ok(doc) = ctx.parse(&text)
        && let Some(missing) = p.ids.iter().find(|id| !exists(&doc, id))
    {
        return Err(EditorError::unknown_node(missing));
    }
    let mut selection: Vec<String> = Vec::with_capacity(p.ids.len());
    for id in p.ids {
        if !selection.contains(&id) {
            selection.push(id);
        }
    }
    ctx.session.selection = selection;
    Ok(json!({ "selection": ctx.session.selection }))
}
