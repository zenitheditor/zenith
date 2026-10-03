//! `tx.layout_managed`: hand-placement ops on an in-flow child of a layout
//! frame are rejected, because the frame computes that child's `x` / `y`.
//!
//! The test is structural (no scene dependency) and mirrors the scene's
//! child roles: a child is in flow when its direct parent is a `row`,
//! `column`, or `grid` frame, it carries layout item attributes, its
//! `position` is not `absolute`, it is visible, and its role is not `guide`.

use zenith_core::{Diagnostic, Document, FrameNode, LayoutKind, LayoutPosition, Node};

/// Push one `tx.layout_managed` Error per in-flow id in `ids` and return
/// `true` when any was pushed. `op` names the rejected op in the message.
pub(in crate::engine) fn reject_layout_managed<'a>(
    doc: &Document,
    ids: impl IntoIterator<Item = &'a str>,
    op: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let mut rejected = false;
    for id in ids {
        if let Some((frame, mode)) = managing_frame(doc, id) {
            rejected = true;
            diagnostics.push(Diagnostic::error(
                "tx.layout_managed",
                format!(
                    "{op}: node {id:?} is in the flow of {mode} frame {frame:?}, which computes \
                     its x/y; to place it by hand, run set_layout with position=\"absolute\" \
                     (and x/y in the same transaction), or change its order with reparent, \
                     move_forward, or move_backward"
                ),
                None,
                Some(id.to_owned()),
            ));
        }
    }
    rejected
}

/// The id and layout mode of the frame that places node `id` in flow, or
/// `None` when no layout frame manages the node's position.
fn managing_frame<'d>(doc: &'d Document, id: &str) -> Option<(&'d str, &'static str)> {
    let roots = doc
        .body
        .pages
        .iter()
        .map(|p| p.children.as_slice())
        .chain(doc.masters.iter().map(|m| m.children.as_slice()));
    for children in roots {
        if let Some(found) = search(children, id) {
            return found;
        }
    }
    None
}

/// Find `id` below `nodes`. `Some(result)` once the node is found (the
/// result says whether a frame manages it); `None` when it is not in this
/// subtree.
fn search<'d>(nodes: &'d [Node], id: &str) -> Option<Option<(&'d str, &'static str)>> {
    for node in nodes {
        if let Node::Frame(f) = node
            && let Some(child) = f.children.iter().find(|c| c.id() == Some(id))
        {
            return Some(
                flow_mode(f)
                    .filter(|_| in_flow(child))
                    .map(|m| (f.id.as_str(), m)),
            );
        }
        if node.id() == Some(id) {
            // A top-level node of the scanned list: no frame parent.
            return Some(None);
        }
        let found = match node {
            Node::Frame(f) => search(&f.children, id),
            Node::Group(g) => search(&g.children, id),
            Node::Table(t) => t
                .rows
                .iter()
                .flat_map(|r| r.cells.iter())
                .find_map(|c| search(&c.children, id)),
            Node::Unknown(u) => search(&u.children, id),
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Text(_)
            | Node::Code(_)
            | Node::Image(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Instance(_)
            | Node::Field(_)
            | Node::Footnote(_)
            | Node::Toc(_)
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_) => None,
        };
        if found.is_some() {
            return found;
        }
    }
    None
}

/// `true` when `frame` places `child` in flow: the frame is a `row`,
/// `column`, or `grid` frame and `child` takes a flow slot.
pub(in crate::engine) fn places_in_flow(frame: &FrameNode, child: &Node) -> bool {
    flow_slot_mode(frame, child).is_some()
}

/// The layout mode name (`row` / `column` / `grid`) when `frame` places
/// `child` in flow, or `None` when it does not.
pub(in crate::engine) fn flow_slot_mode(frame: &FrameNode, child: &Node) -> Option<&'static str> {
    flow_mode(frame).filter(|_| in_flow(child))
}

/// The `tx.flow_placed` advisory: `op` puts node `id` into a flow slot of
/// layout frame `frame` with layout `mode`.
pub(in crate::engine) fn flow_placed(op: &str, id: &str, frame: &str, mode: &str) -> Diagnostic {
    Diagnostic::advisory(
        "tx.flow_placed",
        format!(
            "{op}: node {id:?} is placed by layout frame {frame:?} ({mode}); its x/y and size \
             are ignored and siblings reflow. To keep its page box, set position=\"absolute\" \
             or use set_geometry."
        ),
        None,
        Some(id.to_owned()),
    )
}

/// The layout mode name of a frame that positions its children.
fn flow_mode(f: &FrameNode) -> Option<&'static str> {
    match f.layout.as_ref()? {
        LayoutKind::Row => Some("row"),
        LayoutKind::Column => Some("column"),
        LayoutKind::Grid => Some("grid"),
        LayoutKind::Absolute | LayoutKind::Unknown(_) => None,
    }
}

/// `true` when a layout frame places `child` (the scene's flow role).
fn in_flow(child: &Node) -> bool {
    if child.role() == Some("guide") || child.visible() == Some(false) {
        return false;
    }
    child.layout_item().is_some_and(|item| match item.position {
        Some(LayoutPosition::Absolute) => false,
        Some(LayoutPosition::Auto | LayoutPosition::Unknown(_)) | None => true,
    })
}
