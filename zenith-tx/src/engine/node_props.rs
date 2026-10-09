//! `set_node_token` and `set_span_text`: token-valued node properties
//! (corner radius and font) and the text of one span.

use zenith_core::{Diagnostic, Document, Node, PropertyValue, TextSpan};

use super::{find_node_any_mut, record_affected};

/// The token-valued node properties `set_node_token` writes, in their KDL
/// spelling.
pub const NODE_TOKEN_PROPERTIES: &[&str] = &["radius", "font-family", "font-size", "font-weight"];

/// The properties `set_node_token` writes on `node`, in
/// [`NODE_TOKEN_PROPERTIES`] order.
pub fn node_token_properties(node: &Node) -> Vec<&'static str> {
    let mut probe = node.clone();
    if let Some(children) = probe.children_mut() {
        children.clear();
    }
    NODE_TOKEN_PROPERTIES
        .iter()
        .copied()
        .filter(|p| slot(&mut probe, p).is_some())
        .collect()
}

/// The canonical spelling of `property`, or `None` when `set_node_token`
/// does not write it.
fn canonical(property: &str) -> Option<&'static str> {
    let hyphenated = property.replace('_', "-");
    NODE_TOKEN_PROPERTIES
        .iter()
        .copied()
        .find(|p| *p == hyphenated)
}

/// The slot of `property` (canonical) on `node`, or `None` for a kind
/// without it.
fn slot<'n>(node: &'n mut Node, property: &str) -> Option<&'n mut Option<PropertyValue>> {
    match property {
        "radius" => radius_mut(node),
        "font-family" => font_family_mut(node),
        "font-size" => font_size_mut(node),
        "font-weight" => font_weight_mut(node),
        _ => None,
    }
}

/// The corner radius of a node. A `light`'s `radius` is its reach, not a
/// corner, so it has none here.
fn radius_mut(node: &mut Node) -> Option<&mut Option<PropertyValue>> {
    match node {
        Node::Rect(n) => Some(&mut n.radius),
        Node::Frame(n) => Some(&mut n.radius),
        Node::Shape(n) => Some(&mut n.radius),
        Node::Pattern(n) => Some(&mut n.radius),
        Node::Chart(n) => Some(&mut n.radius),
        Node::Ellipse(_)
        | Node::Line(_)
        | Node::Text(_)
        | Node::Code(_)
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
        | Node::Light(_)
        | Node::Mesh(_)
        | Node::Unknown(_) => None,
    }
}

fn font_family_mut(node: &mut Node) -> Option<&mut Option<PropertyValue>> {
    match node {
        Node::Text(n) => Some(&mut n.font_family),
        Node::Code(n) => Some(&mut n.font_family),
        Node::Field(n) => Some(&mut n.font_family),
        Node::Footnote(n) => Some(&mut n.font_family),
        Node::Toc(n) => Some(&mut n.font_family),
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
        | Node::Table(_)
        | Node::Shape(_)
        | Node::Connector(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Mesh(_)
        | Node::Unknown(_) => None,
    }
}

fn font_size_mut(node: &mut Node) -> Option<&mut Option<PropertyValue>> {
    match node {
        Node::Text(n) => Some(&mut n.font_size),
        Node::Code(n) => Some(&mut n.font_size),
        Node::Field(n) => Some(&mut n.font_size),
        Node::Footnote(n) => Some(&mut n.font_size),
        Node::Toc(n) => Some(&mut n.font_size),
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
        | Node::Table(_)
        | Node::Shape(_)
        | Node::Connector(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Mesh(_)
        | Node::Unknown(_) => None,
    }
}

fn font_weight_mut(node: &mut Node) -> Option<&mut Option<PropertyValue>> {
    match node {
        Node::Text(n) => Some(&mut n.font_weight),
        Node::Code(n) => Some(&mut n.font_weight),
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
        | Node::Unknown(_) => None,
    }
}

fn unknown_node(node_id: &str) -> Diagnostic {
    Diagnostic::error(
        "tx.unknown_node",
        format!("node {node_id:?} not found in document"),
        None,
        Some(node_id.to_owned()),
    )
}

/// Bind `property` of `node_id` to `token`, or remove it when `token` is
/// `None`. The post-apply validation checks the token's type.
pub(super) fn apply_set_node_token(
    node_id: &str,
    property: &str,
    token: Option<&str>,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    let Some(property) = canonical(property) else {
        diagnostics.push(Diagnostic::error(
            "tx.unsupported_property",
            format!(
                "set_node_token does not write {property:?}; use one of: {}",
                NODE_TOKEN_PROPERTIES.join(", ")
            ),
            None,
            Some(node_id.to_owned()),
        ));
        return;
    };
    let Some(node) = find_node_any_mut(doc, node_id) else {
        diagnostics.push(unknown_node(node_id));
        return;
    };
    let kind = node.kind_str();
    match slot(node, property) {
        Some(slot) => {
            *slot = token.map(|t| PropertyValue::TokenRef(t.to_owned()));
            record_affected(node_id, affected);
        }
        None => diagnostics.push(Diagnostic::error(
            "tx.unsupported_property",
            format!("{property} is not supported on a {kind} node"),
            None,
            Some(node_id.to_owned()),
        )),
    }
}

/// Replace the text of span `index` of `node_id` (a `text` or `shape`).
pub(super) fn apply_set_span_text(
    node_id: &str,
    index: usize,
    text: &str,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    let Some(node) = find_node_any_mut(doc, node_id) else {
        diagnostics.push(unknown_node(node_id));
        return;
    };
    let kind = node.kind_str();
    let spans: &mut Vec<TextSpan> = match node {
        Node::Text(n) => &mut n.spans,
        Node::Shape(n) => &mut n.spans,
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
        | Node::Unknown(_) => {
            diagnostics.push(Diagnostic::error(
                "tx.unsupported_property",
                format!("set_span_text is not supported on a {kind} node"),
                None,
                Some(node_id.to_owned()),
            ));
            return;
        }
    };
    let count = spans.len();
    match spans.get_mut(index) {
        Some(span) => {
            text.clone_into(&mut span.text);
            record_affected(node_id, affected);
        }
        None => diagnostics.push(Diagnostic::error(
            "tx.invalid_value",
            format!("{kind} {node_id:?} has {count} span(s); span {index} does not exist"),
            None,
            Some(node_id.to_owned()),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_accepts_underscores_and_rejects_others() {
        assert_eq!(canonical("font_size"), Some("font-size"));
        assert_eq!(canonical("radius"), Some("radius"));
        assert_eq!(canonical("stroke-width"), None);
        assert_eq!(canonical("fill"), None);
    }
}
