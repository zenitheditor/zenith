//! How a node kind takes gestures.

use zenith_core::Node;

/// The gesture family of a node kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// An `x` / `y` / `w` / `h` box (and `instance`): `nudge_geometry`.
    Box,
    /// A `line`: `nudge_line_points`.
    Line,
    /// A `polygon` or `polyline`: `set_points`.
    Points,
    /// A `path`: `transform_path_anchors`, `move_path_anchor`,
    /// `move_path_handle`.
    Path,
    /// A `connector`: geometry derives from its targets.
    Derived,
    /// A `footnote`, `light`, or unknown node: no canvas geometry to edit.
    Fixed,
}

impl Kind {
    /// The family of `node`.
    pub(crate) fn of(node: &Node) -> Kind {
        match node {
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Text(_)
            | Node::Code(_)
            | Node::Frame(_)
            | Node::Group(_)
            | Node::Image(_)
            | Node::Instance(_)
            | Node::Field(_)
            | Node::Toc(_)
            | Node::Table(_)
            | Node::Shape(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Mesh(_) => Kind::Box,
            Node::Line(_) => Kind::Line,
            Node::Polygon(_) | Node::Polyline(_) => Kind::Points,
            Node::Path(_) => Kind::Path,
            Node::Connector(_) => Kind::Derived,
            Node::Footnote(_) | Node::Light(_) | Node::Unknown(_) => Kind::Fixed,
        }
    }
}

/// Why `node` takes no rotation on the canvas, or `None` when it does.
///
/// Kinds without `rotate` (line, instance, field, toc, footnote, light,
/// mesh, unknown) and connectors (derived geometry) take none. A `text` or
/// `code` without `h` takes none either: the scene draws its rotation only
/// with a full box.
pub(crate) fn rotate_blocked(node: &Node) -> Option<(&'static str, String)> {
    let kind = node.kind_str();
    if matches!(Kind::of(node), Kind::Derived) {
        return Some((
            "tx.derived_geometry",
            "a connector's geometry derives from its targets; rotate them instead".to_owned(),
        ));
    }
    if !node.takes_rotate() {
        return Some((
            "editor.unsupported",
            format!("{kind} has no rotate attribute"),
        ));
    }
    let needs_h = match node {
        Node::Text(t) => t.h.is_none(),
        Node::Code(c) => c.h.is_none(),
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Line(_)
        | Node::Frame(_)
        | Node::Group(_)
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
    };
    needs_h.then(|| {
        (
            "editor.rotate_needs_h",
            format!("a {kind} rotates only with an h; set h first"),
        )
    })
}
