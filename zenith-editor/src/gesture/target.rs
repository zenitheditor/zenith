//! The node a gesture or a handle query acts on: where it sits, its
//! compiled box, and its lock and visibility state.

use std::rc::Rc;

use zenith_core::{Document, Node};
use zenith_pipeline::PageView;
use zenith_scene::CompiledBox;

use crate::ctx::Ctx;
use crate::doc::place::{NodePage, node_page};
use crate::doc::tree::{Located, locate};
use crate::edit::offers::{show, unlock};
use crate::error::EditorError;

/// A located node with its compiled box.
pub(crate) struct Target<'d> {
    pub(crate) id: String,
    pub(crate) located: Located<'d>,
    pub(crate) page: NodePage,
    /// The node's box on `page`.
    pub(crate) bx: CompiledBox,
    /// The px origin of the space that holds the node's `x` / `y`, when
    /// every container offset resolves.
    pub(crate) origin: Option<(f64, f64)>,
    /// The compiled page (boxes of every node, no raster), shared by the
    /// targets of one gesture.
    pub(crate) view: Rc<PageView>,
}

/// Locate node `id` in `doc` and compile the page it draws on.
///
/// # Errors
///
/// `editor.unknown_node`; `editor.not_drawn` for master content no page
/// uses; `editor.no_box` for a node the page draws no box for (a guide, a
/// node inside a hidden container, or a node that failed to compile).
pub(crate) fn resolve<'d>(
    ctx: &mut Ctx<'_, '_>,
    doc: &'d Document,
    id: &str,
) -> Result<Target<'d>, EditorError> {
    let (located, page) = place(ctx, doc, id)?;
    let view = Rc::new(ctx.view(doc, page.index, None, None)?);
    build(id, located, page, view, doc)
}

/// Locate every node of `ids` and compile their page once. The nodes must
/// draw on one page.
///
/// # Errors
///
/// The errors of [`resolve`], and `editor.mixed_pages` when the nodes draw
/// on different pages.
pub(crate) fn resolve_all<'d>(
    ctx: &mut Ctx<'_, '_>,
    doc: &'d Document,
    ids: &[String],
) -> Result<Vec<Target<'d>>, EditorError> {
    let mut placed = Vec::with_capacity(ids.len());
    for id in ids {
        placed.push((id, place(ctx, doc, id)?));
    }
    let Some((_, (_, first))) = placed.first() else {
        return Ok(Vec::new());
    };
    let index = first.index;
    if let Some((id, (_, other))) = placed.iter().find(|(_, (_, p))| p.index != index) {
        return Err(EditorError::new(
            "editor.mixed_pages",
            format!(
                "'{id}' draws on page {} and the other nodes on page {}; select nodes of one \
                 page",
                other.index + 1,
                index + 1
            ),
        ));
    }
    let view = Rc::new(ctx.view(doc, index, None, None)?);
    placed
        .into_iter()
        .map(|(id, (located, page))| build(id, located, page, Rc::clone(&view), doc))
        .collect()
}

/// Where node `id` sits and the page it draws on.
fn place<'d>(
    ctx: &Ctx<'_, '_>,
    doc: &'d Document,
    id: &str,
) -> Result<(Located<'d>, NodePage), EditorError> {
    let located = locate(doc, id).ok_or_else(|| EditorError::unknown_node(id))?;
    let page = node_page(doc, &located, ctx.session.page.saturating_sub(1))?;
    Ok((located, page))
}

fn build<'d>(
    id: &str,
    located: Located<'d>,
    page: NodePage,
    view: Rc<PageView>,
    doc: &'d Document,
) -> Result<Target<'d>, EditorError> {
    let bx = view.boxes.get(&page.raw_id).cloned().ok_or_else(|| {
        EditorError::new(
            "editor.no_box",
            format!(
                "page {} draws no box for '{id}' (a guide, a hidden container, or a node \
                 that does not compile); edit it in the code",
                page.index + 1
            ),
        )
    })?;
    let origin = zenith_tx::parent_space_origin(doc, id).and_then(Result::ok);
    Ok(Target {
        id: id.to_owned(),
        located,
        page,
        bx,
        origin,
        view,
    })
}

/// The id of the node that locks `located`: itself or the nearest locked
/// ancestor (a locked container locks its content in the editor).
pub(crate) fn locked_by<'d>(located: &Located<'d>) -> Option<&'d str> {
    located.self_or_ancestor(Node::is_locked).and_then(Node::id)
}

/// The id of the node that hides `located`: itself or the nearest hidden
/// ancestor.
pub(crate) fn hidden_by<'d>(located: &Located<'d>) -> Option<&'d str> {
    located
        .self_or_ancestor(|n| !n.is_visible())
        .and_then(Node::id)
}

/// Reject a gesture on a locked or hidden node, offering to unlock or
/// show the node that blocks it.
pub(crate) fn check_editable(located: &Located<'_>, id: &str) -> Result<(), EditorError> {
    if let Some(by) = locked_by(located) {
        let why = if by == id {
            format!("node '{id}' is locked")
        } else {
            format!("node '{id}' is inside locked '{by}'")
        };
        return Err(EditorError::new(
            "editor.locked",
            format!("{why}; unlock it (offer `unlock`) to edit it on the canvas"),
        )
        .with_offers(vec![unlock(by)]));
    }
    if let Some(by) = hidden_by(located) {
        let why = if by == id {
            format!("node '{id}' is hidden (visible=#false)")
        } else {
            format!("node '{id}' is inside hidden '{by}'")
        };
        return Err(EditorError::new(
            "editor.hidden",
            format!("{why}, so the canvas does not draw it; show it (offer `show`) to edit it"),
        )
        .with_offers(vec![show(by)]));
    }
    Ok(())
}
