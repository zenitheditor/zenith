//! Pure AST application of text-outline path nodes.
//!
//! Outline geometry is produced by [`zenith_scene`] (compile + glyph outline).
//! This module only validates the source node, paints the paths, and inserts
//! them as siblings — no scene/layout dependency.
//!
//! Callers should [`check_text_outline_source`] **before** compiling pages so
//! wrong-kind / missing-id targets short-circuit without paying compile cost.

use zenith_core::{Diagnostic, Document, Node, PathNode, PropertyValue, Severity};

use super::{find_node_shared, finish_candidate, format_source, record_affected};
use crate::result::{TxError, TxResult};

/// Request for text-to-outline path insertion.
///
/// Path ids are already assigned by the scene outline collector (`id_prefix` +
/// glyph-run index); this request only names the source node to paint from and
/// insert after.
#[derive(Debug, Clone, PartialEq)]
pub struct TextOutlineRequest {
    /// Authored `text` or `code` node id that was outlined.
    pub node: String,
}

/// Diagnostic code emitted by scene outline conversion failure (stable string
/// used by [`apply_text_outline_paths`] to avoid stacking `tx.no_text_outlines`).
pub const SCENE_TEXT_OUTLINE_FAILED: &str = "scene.text_outline_failed";

/// Validate that `node_id` names a text/code node eligible for outlining.
///
/// Returns `Ok(())` when eligible. Returns `Err(diagnostics)` with
/// `tx.unknown_node` or `tx.unsupported_property` when not — without compiling
/// the document. Call this **before**
/// [`zenith_scene::collect_text_outline_paths`].
pub fn check_text_outline_source(doc: &Document, node_id: &str) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    if source_outline_paint(doc, node_id, &mut diagnostics).is_none() {
        return Err(diagnostics);
    }
    Ok(())
}

/// Build a rejected [`TxResult`] from pre-validation diagnostics (no AST
/// mutation). Used when [`check_text_outline_source`] fails so callers do not
/// re-run validation inside apply.
pub fn reject_text_outline(
    doc: &Document,
    diagnostics: Vec<Diagnostic>,
) -> Result<TxResult, TxError> {
    let source_before = format_source(doc, "source_before")?;
    finish_candidate(source_before, doc.clone(), diagnostics, Vec::new())
}

/// Insert precomputed outline [`PathNode`]s after the source text/code node.
///
/// Callers obtain `paths` from scene compilation + outline conversion so this
/// crate stays free of the scene/layout dependency.
///
/// Precondition: the source node should already have passed
/// [`check_text_outline_source`]. Apply still re-validates defensively.
///
/// `extra_diagnostics` are compile/outline advisories folded into the result.
/// Empty `paths` after a conversion failure (`scene.text_outline_failed`) does
/// **not** also emit `tx.no_text_outlines`.
pub fn apply_text_outline_paths(
    doc: &Document,
    request: &TextOutlineRequest,
    paths: Vec<PathNode>,
    extra_diagnostics: Vec<Diagnostic>,
) -> Result<TxResult, TxError> {
    let source_before = format_source(doc, "source_before")?;
    let mut diagnostics = extra_diagnostics;
    let mut affected = Vec::new();
    let mut candidate = doc.clone();

    let Some(outline_paint) = source_outline_paint(&candidate, &request.node, &mut diagnostics)
    else {
        return finish_candidate(source_before, candidate, diagnostics, affected);
    };

    if paths.is_empty() {
        let conversion_failed = diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error && d.code == SCENE_TEXT_OUTLINE_FAILED);
        if !conversion_failed {
            diagnostics.push(Diagnostic::error(
                "tx.no_text_outlines",
                format!(
                    "text outline materialization for node '{}' produced no path geometry",
                    request.node
                ),
                None,
                Some(request.node.clone()),
            ));
        }
        return finish_candidate(source_before, candidate, diagnostics, affected);
    }

    let nodes: Vec<Node> = paths
        .into_iter()
        .map(|mut path| {
            outline_paint.apply_to(&mut path);
            Node::Path(path)
        })
        .collect();
    if insert_after_node(&mut candidate.body.pages, &request.node, &nodes) {
        for node in &nodes {
            if let Some(id) = node.id() {
                record_affected(id, &mut affected);
            }
        }
    } else {
        diagnostics.push(Diagnostic::error(
            "tx.unknown_node",
            format!("no node with id {:?}", request.node),
            None,
            Some(request.node.clone()),
        ));
    }

    finish_candidate(source_before, candidate, diagnostics, affected)
}

#[derive(Debug, Clone, PartialEq)]
struct OutlinePaint {
    fill: Option<PropertyValue>,
    stroke: Option<PropertyValue>,
    stroke_width: Option<PropertyValue>,
}

impl OutlinePaint {
    fn apply_to(&self, path: &mut PathNode) {
        path.fill = self.fill.clone();
        path.stroke = self.stroke.clone();
        path.stroke_width = self.stroke_width.clone();
    }
}

fn source_outline_paint(
    doc: &Document,
    node_id: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<OutlinePaint> {
    for page in &doc.body.pages {
        if let Some(node) = find_node_shared(&page.children, node_id) {
            return match node {
                Node::Text(text) => Some(OutlinePaint {
                    fill: text.fill.clone(),
                    stroke: text.stroke.clone(),
                    stroke_width: text.stroke_width.clone(),
                }),
                Node::Code(code) => Some(OutlinePaint {
                    fill: code.fill.clone(),
                    stroke: None,
                    stroke_width: None,
                }),
                other => {
                    diagnostics.push(Diagnostic::error(
                        "tx.unsupported_property",
                        format!(
                            "text outline materialization is not supported on a {} node",
                            other.kind_str()
                        ),
                        None,
                        Some(node_id.to_owned()),
                    ));
                    None
                }
            };
        }
    }

    diagnostics.push(Diagnostic::error(
        "tx.unknown_node",
        format!("no node with id {:?}", node_id),
        None,
        Some(node_id.to_owned()),
    ));
    None
}

fn insert_after_node(pages: &mut [zenith_core::Page], node_id: &str, nodes: &[Node]) -> bool {
    for page in pages {
        if insert_after_node_in_children(&mut page.children, node_id, nodes) {
            return true;
        }
    }
    false
}

fn insert_after_node_in_children(children: &mut Vec<Node>, node_id: &str, nodes: &[Node]) -> bool {
    if let Some(index) = children.iter().position(|node| node.id() == Some(node_id)) {
        children.splice(index + 1..index + 1, nodes.iter().cloned());
        return true;
    }

    for child in children.iter_mut() {
        match child {
            Node::Frame(frame) => {
                if insert_after_node_in_children(&mut frame.children, node_id, nodes) {
                    return true;
                }
            }
            Node::Group(group) => {
                if insert_after_node_in_children(&mut group.children, node_id, nodes) {
                    return true;
                }
            }
            Node::Table(table) => {
                for row in &mut table.rows {
                    for cell in &mut row.cells {
                        if insert_after_node_in_children(&mut cell.children, node_id, nodes) {
                            return true;
                        }
                    }
                }
            }
            Node::Unknown(unknown) => {
                if insert_after_node_in_children(&mut unknown.children, node_id, nodes) {
                    return true;
                }
            }
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
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Shape(_) => {}
        }
    }

    false
}
