//! `content.placeholder`: text nodes still holding the sample content a starter
//! or template left for the author. A text or shape node is a placeholder when
//! its `role` is `placeholder` or one of its spans is exactly the fill-command
//! text `<new text>`.
//!
//! One diagnostic per document names the first few, so an edit loop prints one
//! line, not one per node. It has no subject: a policy silences it with
//! `allow "content.placeholder"`, and a partial fill does not change its
//! identity across two validate runs.

use crate::ast::document::Document;
use crate::ast::node::{Node, PLACEHOLDER_TEXT, TextSpan};
use crate::diagnostics::Diagnostic;

/// Node ids the message names before it counts the rest.
const NAMED: usize = 6;

fn spans_hold_fill_text(spans: &[TextSpan]) -> bool {
    spans.iter().any(|s| s.text == PLACEHOLDER_TEXT)
}

/// Whether `node` is a text or shape node that still holds placeholder content.
fn is_placeholder_content(node: &Node) -> bool {
    match node {
        Node::Text(t) => node.is_placeholder() || spans_hold_fill_text(&t.spans),
        Node::Shape(s) => node.is_placeholder() || spans_hold_fill_text(&s.spans),
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
        | Node::Unknown(_) => false,
    }
}

/// Every placeholder node under `nodes`, depth first, table cells included.
fn collect<'a>(nodes: &'a [Node], out: &mut Vec<&'a Node>) {
    for node in nodes {
        if is_placeholder_content(node) {
            out.push(node);
        }
        if let Node::Table(table) = node {
            for cell in table.rows.iter().flat_map(|row| &row.cells) {
                collect(&cell.children, out);
            }
        }
        if let Some(children) = node.children() {
            collect(children, out);
        }
    }
}

/// Push one `content.placeholder` when any page or master node still holds
/// placeholder content. The span is the first node's.
pub(super) fn check_placeholders(doc: &Document, diagnostics: &mut Vec<Diagnostic>) {
    let mut found = Vec::new();
    for page in &doc.body.pages {
        collect(&page.children, &mut found);
    }
    for master in &doc.masters {
        collect(&master.children, &mut found);
    }
    let Some(first) = found.first() else {
        return;
    };
    let ids: Vec<&str> = found.iter().map(|n| n.id_and_span().0).collect();
    let named = ids
        .iter()
        .take(NAMED)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    let more = match ids.len().saturating_sub(NAMED) {
        0 => String::new(),
        n => format!(" and {n} more"),
    };
    let (_, span) = first.id_and_span();
    diagnostics.push(Diagnostic::warning(
        "content.placeholder",
        format!(
            "{} text node(s) still hold placeholder content: {named}{more}. Replace each with a `tx` text op (it clears role=\"placeholder\"), or remove the role.",
            ids.len()
        ),
        span,
        None,
    ));
}
