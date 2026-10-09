//! Where a node draws: the page to compile for it, and the compiled box id
//! it records under.

use zenith_core::{Document, Node};

use super::tree::{Located, Root};
use crate::error::EditorError;

/// The page a node draws on, for compile and hit data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NodePage {
    /// The 0-based page index.
    pub(crate) index: usize,
    /// The id the node's box records under on that page: the node id, or
    /// `<page-id>/<id>` for master content.
    pub(crate) raw_id: String,
    /// The master that holds the node, when it is master content: an edit
    /// changes every page that uses the master.
    pub(crate) master: Option<String>,
}

/// The page to compile for the node at `located`.
///
/// A page node draws on its own page. A master node draws on every page
/// that uses its master: the current page (0-based `current`) when it uses
/// the master, else the first page that does.
///
/// # Errors
///
/// `editor.not_drawn` when no page uses the node's master.
pub(crate) fn node_page(
    doc: &Document,
    located: &Located<'_>,
    current: usize,
) -> Result<NodePage, EditorError> {
    let id = located.node.id().unwrap_or_default();
    match located.root {
        Root::Page(index) => Ok(NodePage {
            index,
            raw_id: id.to_owned(),
            master: None,
        }),
        Root::Master(m) => {
            let master_id = doc
                .masters
                .get(m)
                .map(|m| m.id.as_str())
                .unwrap_or_default();
            let uses = |i: &usize| {
                doc.body
                    .pages
                    .get(*i)
                    .is_some_and(|p| p.master.as_deref() == Some(master_id))
            };
            let index = Some(current)
                .filter(uses)
                .or_else(|| (0..doc.body.pages.len()).find(uses))
                .ok_or_else(|| {
                    EditorError::new(
                        "editor.not_drawn",
                        format!(
                            "node '{id}' is in master '{master_id}', which no page uses; set a \
                             page's master to '{master_id}' to see and edit it on the canvas"
                        ),
                    )
                })?;
            let page_id = doc
                .body
                .pages
                .get(index)
                .map(|p| p.id.as_str())
                .unwrap_or_default();
            Ok(NodePage {
                index,
                raw_id: format!("{page_id}/{id}"),
                master: Some(master_id.to_owned()),
            })
        }
    }
}

/// `true` when an ancestor group turns about the bounds of its content (a
/// rotated group without both `w` and `h`). Then the pivot moves when the
/// node changes size or place, so a gesture's page position is exact only
/// up to that pivot shift.
pub(crate) fn pivot_follows_content(located: &Located<'_>) -> bool {
    located.ancestors.iter().any(|a| match a {
        Node::Group(g) => {
            g.rotate.as_ref().is_some_and(|r| r.value != 0.0) && (g.w.is_none() || g.h.is_none())
        }
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Line(_)
        | Node::Text(_)
        | Node::Code(_)
        | Node::Frame(_)
        | Node::Image(_)
        | Node::Polygon(_)
        | Node::Polyline(_)
        | Node::Path(_)
        | Node::Instance(_)
        | Node::Field(_)
        | Node::Toc(_)
        | Node::Footnote(_)
        | Node::Table(_)
        | Node::Shape(_)
        | Node::Connector(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Mesh(_)
        | Node::Unknown(_) => false,
    })
}
