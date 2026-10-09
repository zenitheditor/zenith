//! `node.set` text fields: the one plain span (`text`) and per-span text
//! (`spans`).

use zenith_core::{Node, TextSpan};

/// The text of a `text` node with exactly one span that carries no
/// attribute of its own; `None` for anything else.
pub(crate) fn plain_text(node: &Node) -> Option<&str> {
    let Node::Text(t) = node else {
        return None;
    };
    match t.spans.as_slice() {
        [only] if is_plain(only) => Some(only.text.as_str()),
        _ => None,
    }
}

fn is_plain(span: &TextSpan) -> bool {
    let TextSpan {
        text: _,
        fill,
        font_weight,
        font_features,
        font_alternates,
        letter_spacing,
        italic,
        underline,
        strikethrough,
        vertical_align,
        footnote_ref,
        data_ref,
        data_format,
        highlight,
        code,
        link,
    } = span;
    fill.is_none()
        && font_weight.is_none()
        && font_features.is_none()
        && font_alternates.is_none()
        && letter_spacing.is_none()
        && italic.is_none()
        && underline.is_none()
        && strikethrough.is_none()
        && vertical_align.is_none()
        && footnote_ref.is_none()
        && data_ref.is_none()
        && data_format.is_none()
        && highlight.is_none()
        && code.is_none()
        && link.is_none()
}

/// One span of a `text` or `shape` node, for the inspector.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct SpanOut {
    pub(crate) text: String,
    /// The span carries attributes of its own (fill, weight, link, …).
    pub(crate) styled: bool,
}

/// The spans of a `text` or `shape` node; `None` for any other kind.
pub(crate) fn spans_of(node: &Node) -> Option<Vec<SpanOut>> {
    let spans = match node {
        Node::Text(t) => &t.spans,
        Node::Shape(s) => &s.spans,
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Line(_)
        | Node::Code(_)
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
        | Node::Connector(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Mesh(_)
        | Node::Unknown(_) => return None,
    };
    Some(
        spans
            .iter()
            .map(|s| SpanOut {
                text: s.text.clone(),
                styled: !is_plain(s),
            })
            .collect(),
    )
}
