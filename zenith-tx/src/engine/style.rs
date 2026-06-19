//! Style and text op application: fill, stroke, stroke-width, opacity,
//! text-align, and text-replacement setters, plus the property accessors
//! they use.

use zenith_core::{Diagnostic, Document, Node, PropertyValue, TextSpan};

use crate::op::OpSpan;

use super::{find_node_any_mut, node_kind_str, record_affected};

// ── Valid align values ────────────────────────────────────────────────────────

const VALID_ALIGNS: &[&str] = &["start", "center", "end", "justify"];

// ── Field accessor helpers ────────────────────────────────────────────────────

/// Return a mutable reference to the `fill` field of a node, or `None` for
/// node variants that do not have a `fill` property
/// (`Line`, `Frame`, `Group`, `Image`, `Unknown`).
fn node_fill_mut(node: &mut Node) -> Option<&mut Option<PropertyValue>> {
    match node {
        Node::Rect(n) => Some(&mut n.fill),
        Node::Ellipse(n) => Some(&mut n.fill),
        Node::Text(n) => Some(&mut n.fill),
        Node::Code(n) => Some(&mut n.fill),
        Node::Polygon(n) => Some(&mut n.fill),
        Node::Polyline(n) => Some(&mut n.fill),
        Node::Line(_) | Node::Frame(_) | Node::Group(_) | Node::Image(_) | Node::Unknown(_) => None,
    }
}

/// Return a mutable reference to the `stroke` field of a node, or `None` for
/// variants that do not have a `stroke` property
/// (`Text`, `Code`, `Frame`, `Group`, `Image`, `Unknown`).
fn node_stroke_mut(node: &mut Node) -> Option<&mut Option<PropertyValue>> {
    match node {
        Node::Rect(n) => Some(&mut n.stroke),
        Node::Line(n) => Some(&mut n.stroke),
        Node::Polygon(n) => Some(&mut n.stroke),
        Node::Polyline(n) => Some(&mut n.stroke),
        Node::Ellipse(n) => Some(&mut n.stroke),
        Node::Text(_)
        | Node::Code(_)
        | Node::Frame(_)
        | Node::Group(_)
        | Node::Image(_)
        | Node::Unknown(_) => None,
    }
}

/// Return a mutable reference to the `stroke_width` field of a node, or `None`
/// for variants that do not have a `stroke_width` property
/// (`Text`, `Code`, `Frame`, `Group`, `Image`, `Unknown`).
fn node_stroke_width_mut(node: &mut Node) -> Option<&mut Option<PropertyValue>> {
    match node {
        Node::Rect(n) => Some(&mut n.stroke_width),
        Node::Line(n) => Some(&mut n.stroke_width),
        Node::Polygon(n) => Some(&mut n.stroke_width),
        Node::Polyline(n) => Some(&mut n.stroke_width),
        Node::Ellipse(n) => Some(&mut n.stroke_width),
        Node::Text(_)
        | Node::Code(_)
        | Node::Frame(_)
        | Node::Group(_)
        | Node::Image(_)
        | Node::Unknown(_) => None,
    }
}

/// Return a mutable reference to the `opacity` field of a node, or `None`
/// for `Node::Unknown` which carries no `opacity` field.
fn node_opacity_mut(node: &mut Node) -> Option<&mut Option<f64>> {
    match node {
        Node::Rect(n) => Some(&mut n.opacity),
        Node::Ellipse(n) => Some(&mut n.opacity),
        Node::Line(n) => Some(&mut n.opacity),
        Node::Text(n) => Some(&mut n.opacity),
        Node::Code(n) => Some(&mut n.opacity),
        Node::Frame(n) => Some(&mut n.opacity),
        Node::Group(n) => Some(&mut n.opacity),
        Node::Image(n) => Some(&mut n.opacity),
        Node::Polygon(n) => Some(&mut n.opacity),
        Node::Polyline(n) => Some(&mut n.opacity),
        Node::Unknown(_) => None,
    }
}

// ── SetTextAlign ──────────────────────────────────────────────────────────────

pub(super) fn apply_set_text_align(
    node_id: &str,
    align: &str,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    // Validate align value before touching the tree.
    if !VALID_ALIGNS.contains(&align) {
        diagnostics.push(Diagnostic::error(
            "tx.invalid_value",
            format!(
                "invalid align value {:?}; must be one of: {}",
                align,
                VALID_ALIGNS.join(", ")
            ),
            None,
            Some(node_id.to_owned()),
        ));
        return;
    }

    // Walk the tree looking for `node_id`.
    match find_node_any_mut(doc, node_id) {
        None => {
            diagnostics.push(Diagnostic::error(
                "tx.unknown_node",
                format!("node {:?} not found in document", node_id),
                None,
                Some(node_id.to_owned()),
            ));
        }
        Some(Node::Text(text_node)) => {
            text_node.align = Some(align.to_owned());
            record_affected(node_id, affected);
        }
        Some(other) => {
            let kind = node_kind_str(other);
            diagnostics.push(Diagnostic::error(
                "tx.wrong_node_type",
                format!(
                    "set_text_align requires a text node but {:?} is a {}",
                    node_id, kind
                ),
                None,
                Some(node_id.to_owned()),
            ));
        }
    }
}

// ── SetFill / SetStroke / SetStrokeWidth ──────────────────────────────────────

/// Shared driver for token-valued property setters (`set_fill`, `set_stroke`,
/// `set_stroke_width`). Finds the node, fetches the `Option<PropertyValue>`
/// slot via `accessor`, and sets it to `TokenRef(token)`. Emits `tx.unknown_node`
/// if the node is missing, or `tx.unsupported_property` (naming `op_label`) if the
/// node kind lacks the property.
fn apply_set_property_token(
    node_id: &str,
    token: &str,
    op_label: &str,
    accessor: fn(&mut Node) -> Option<&mut Option<PropertyValue>>,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
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
            // node_kind_str returns &'static str, so there is no live borrow
            // of `node` after this let binding — the mutable borrow below is fine.
            let kind = node_kind_str(node);
            match accessor(node) {
                Some(slot) => {
                    *slot = Some(PropertyValue::TokenRef(token.to_owned()));
                    record_affected(node_id, affected);
                }
                None => {
                    diagnostics.push(Diagnostic::error(
                        "tx.unsupported_property",
                        format!("{} is not supported on a {} node", op_label, kind),
                        None,
                        Some(node_id.to_owned()),
                    ));
                }
            }
        }
    }
}

pub(super) fn apply_set_fill(
    node_id: &str,
    fill_token: &str,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    apply_set_property_token(
        node_id,
        fill_token,
        "set_fill",
        node_fill_mut,
        doc,
        diagnostics,
        affected,
    );
}

pub(super) fn apply_set_stroke(
    node_id: &str,
    stroke_token: &str,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    apply_set_property_token(
        node_id,
        stroke_token,
        "set_stroke",
        node_stroke_mut,
        doc,
        diagnostics,
        affected,
    );
}

pub(super) fn apply_set_stroke_width(
    node_id: &str,
    stroke_width_token: &str,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    apply_set_property_token(
        node_id,
        stroke_width_token,
        "set_stroke_width",
        node_stroke_width_mut,
        doc,
        diagnostics,
        affected,
    );
}

// ── SetOpacity ────────────────────────────────────────────────────────────────

pub(super) fn apply_set_opacity(
    node_id: &str,
    opacity: f64,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
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
            match node_opacity_mut(node) {
                Some(slot) => {
                    *slot = Some(opacity.clamp(0.0, 1.0));
                    record_affected(node_id, affected);
                }
                None => {
                    diagnostics.push(Diagnostic::error(
                        "tx.unsupported_property",
                        format!("set_opacity is not supported on a {} node", kind),
                        None,
                        Some(node_id.to_owned()),
                    ));
                }
            }
        }
    }
}

// ── ReplaceText ───────────────────────────────────────────────────────────────

pub(super) fn apply_replace_text(
    node_id: &str,
    spans: &[OpSpan],
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    match find_node_any_mut(doc, node_id) {
        None => {
            diagnostics.push(Diagnostic::error(
                "tx.unknown_node",
                format!("node {:?} not found in document", node_id),
                None,
                Some(node_id.to_owned()),
            ));
        }
        Some(Node::Text(text_node)) => {
            text_node.spans = spans
                .iter()
                .map(|s| TextSpan {
                    text: s.text.clone(),
                    fill: s
                        .fill
                        .as_ref()
                        .map(|id| PropertyValue::TokenRef(id.clone())),
                    font_weight: s
                        .font_weight
                        .as_ref()
                        .map(|id| PropertyValue::TokenRef(id.clone())),
                    italic: s.italic,
                    underline: s.underline,
                    strikethrough: s.strikethrough,
                })
                .collect();
            record_affected(node_id, affected);
        }
        Some(other) => {
            let kind = node_kind_str(other);
            diagnostics.push(Diagnostic::error(
                "tx.unsupported_property",
                format!("replace_text is not supported on a {} node", kind),
                None,
                Some(node_id.to_owned()),
            ));
        }
    }
}
