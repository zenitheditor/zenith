//! `select.marquee`: select the nodes a dragged page rectangle meets.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use zenith_core::Node;
use zenith_scene::{hit_region, selectable_id};

use super::common::{page_index, params};
use crate::ctx::Ctx;
use crate::doc::tree::{Root, child_lists, locate};
use crate::error::EditorError;
use crate::gesture::target::{hidden_by, locked_by};
use crate::wire::to_json;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MarqueeParams {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    #[serde(default)]
    page: Option<usize>,
    #[serde(default)]
    contain: bool,
    #[serde(default)]
    extend: bool,
    #[serde(default = "yes")]
    select: bool,
}

fn yes() -> bool {
    true
}

/// One node the rectangle selects.
#[derive(Debug, Serialize)]
struct Picked {
    id: String,
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    /// The master that holds the node: an edit changes every page that
    /// uses it.
    #[serde(skip_serializing_if = "Option::is_none")]
    master: Option<String>,
}

/// The nodes the page rectangle `{x, y, w, h}` (page px; a negative `w` or
/// `h` extends left or up from `x` / `y`) meets on 1-based `page`, in paint
/// order (back to front), and (with `select`, the default) the new
/// selection.
///
/// Rules:
/// - A node counts when its painted shape (line, polygon, polyline, path,
///   connector) or its drawn box meets the rectangle inside its clips (see
///   `CompiledBox::touches_region`); with `contain`, only when its whole
///   drawn box lies inside the rectangle.
/// - Content maps to the node a click selects: master content to its
///   master node, instance and pattern content to the instance or pattern.
/// - Locked nodes, and nodes inside a locked container, never count; hidden
///   nodes draw no box and never count. Guides have no box.
/// - A container (frame, group, table) whose drawn box holds the whole
///   rectangle is entered: it does not count, its content does.
/// - Only the outermost counted node of a subtree is selected: a node
///   inside another counted node rides along with it.
/// - With `extend` the nodes are added to the selection (kept in its order,
///   new ones after); else they replace it. A rectangle that meets nothing
///   clears the selection unless `extend`. The page becomes the session
///   page.
pub(crate) fn run(ctx: &mut Ctx<'_, '_>, raw: Value) -> Result<Value, EditorError> {
    let p: MarqueeParams = params(ctx, raw)?;
    if ![p.x, p.y, p.w, p.h].iter().all(|v| v.is_finite()) {
        return Err(EditorError::new(
            "editor.invalid_params",
            "x, y, w, and h must be finite page px",
        ));
    }
    let (x0, x1) = (p.x.min(p.x + p.w), p.x.max(p.x + p.w));
    let (y0, y1) = (p.y.min(p.y + p.h), p.y.max(p.y + p.h));
    let region = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
    let display = ctx.display()?;
    let doc = &display.doc;
    let index = page_index(ctx, p.page, doc.body.pages.len())?;
    let view = ctx.view(doc, index, None, None)?;
    let raw_hits: Vec<(&str, usize)> = if p.contain {
        let mut inside: Vec<(&str, usize)> = view
            .boxes
            .iter()
            .filter(|(_, b)| !b.hidden && b.within_region(&region))
            .map(|(id, b)| (id.as_str(), b.paint_order))
            .collect();
        inside.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        inside
    } else {
        hit_region(&view.boxes, &region)
            .into_iter()
            .filter_map(|id| view.boxes.get(id).map(|b| (id, b.paint_order)))
            .collect()
    };
    // Topmost first; the first raw id of a node sets its rank.
    let mut counted: Vec<(&str, usize)> = Vec::new();
    for (raw_id, order) in raw_hits {
        let Some(id) = selectable_id(doc, index, raw_id) else {
            continue;
        };
        if counted.iter().any(|(c, _)| *c == id) {
            continue;
        }
        let Some(located) = locate(doc, id) else {
            continue;
        };
        if locked_by(&located).is_some() || hidden_by(&located).is_some() {
            continue;
        }
        let entered = !p.contain
            && !child_lists(located.node).iter().all(|l| l.is_empty())
            && !matches!(located.node, Node::Instance(_) | Node::Pattern(_))
            && view
                .boxes
                .get(raw_id)
                .is_some_and(|b| b.holds_region(&region));
        if entered {
            continue;
        }
        counted.push((id, order));
    }
    let ids: Vec<&str> = counted.iter().map(|(id, _)| *id).collect();
    let mut picked: Vec<(usize, Picked)> = Vec::new();
    for (id, order) in &counted {
        let Some(located) = locate(doc, id) else {
            continue;
        };
        let inside_counted = located
            .ancestors
            .iter()
            .filter_map(|a| a.id())
            .any(|a| ids.contains(&a));
        if inside_counted {
            continue;
        }
        let master = match located.root {
            Root::Master(m) => doc.masters.get(m).map(|m| m.id.clone()),
            Root::Page(_) => None,
        };
        picked.push((
            *order,
            Picked {
                id: (*id).to_owned(),
                kind: located.node.kind_str(),
                name: located.node.name().map(str::to_owned),
                master,
            },
        ));
    }
    picked.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.id.cmp(&b.1.id)));
    let picked: Vec<Picked> = picked.into_iter().map(|(_, p)| p).collect();
    if p.select {
        if p.extend {
            for n in &picked {
                if !ctx.session.selection.contains(&n.id) {
                    ctx.session.selection.push(n.id.clone());
                }
            }
        } else {
            ctx.session.selection = picked.iter().map(|n| n.id.clone()).collect();
        }
    }
    ctx.session.page = index + 1;
    Ok(json!({
        "page": index + 1,
        "stale": display.stale,
        "hits": to_json(&picked)?,
        "selection": ctx.session.selection,
    }))
}
