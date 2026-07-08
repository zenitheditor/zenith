//! Fill-rule op application for vector nodes with authored winding metadata.

use zenith_core::{Diagnostic, Document, Node};

use super::{find_node_any_mut, node_kind_str, record_affected};

const VALID_FILL_RULES_LABEL: &str = "nonzero|evenodd";

fn node_fill_rule_mut(node: &mut Node) -> Option<&mut Option<String>> {
    match node {
        Node::Polygon(n) => Some(&mut n.fill_rule),
        Node::Polyline(n) => Some(&mut n.fill_rule),
        Node::Path(n) => Some(&mut n.fill_rule),
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Line(_)
        | Node::Text(_)
        | Node::Code(_)
        | Node::Frame(_)
        | Node::Group(_)
        | Node::Image(_)
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

fn is_valid_fill_rule(fill_rule: &str) -> bool {
    matches!(fill_rule, "nonzero" | "evenodd")
}

pub(super) fn apply_set_fill_rule(
    node_id: &str,
    fill_rule: &str,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    if !is_valid_fill_rule(fill_rule) {
        diagnostics.push(Diagnostic::error(
            "tx.invalid_value",
            format!(
                "invalid fill-rule value {:?}; must be one of: {}",
                fill_rule, VALID_FILL_RULES_LABEL
            ),
            None,
            Some(node_id.to_owned()),
        ));
        return;
    }

    match find_node_any_mut(doc, node_id) {
        None => {
            diagnostics.push(Diagnostic::error(
                "tx.unknown_node",
                format!("node {:?} not found in document", node_id),
                None,
                Some(node_id.to_owned()),
            ));
        }
        Some(node) => {
            let kind = node_kind_str(node);
            match node_fill_rule_mut(node) {
                Some(slot) => {
                    *slot = Some(fill_rule.to_owned());
                    record_affected(node_id, affected);
                }
                None => {
                    diagnostics.push(Diagnostic::error(
                        "tx.unsupported_property",
                        format!("set_fill_rule is not supported on a {} node", kind),
                        None,
                        Some(node_id.to_owned()),
                    ));
                }
            }
        }
    }
}
