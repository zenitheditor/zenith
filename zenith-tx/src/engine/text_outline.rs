use zenith_core::{
    Diagnostic, Document, FontProvider, Node, PathNode, PropertyValue,
};
use zenith_scene::{compile_page, outline_source_glyph_run_commands};

use super::{
    finish_candidate, find_node_shared, format_source, node_id_of, node_kind_str, record_affected,
};
use crate::result::{TxError, TxResult};

/// Request for font-aware text-to-outline materialization.
#[derive(Debug, Clone, PartialEq)]
pub struct TextOutlineRequest {
    /// Authored `text` or `code` node id to materialize.
    pub node: String,
    /// Prefix for generated path ids. The compiled glyph-run index is appended.
    pub id_prefix: String,
}

/// Compile `doc` with `fonts`, materialize the target text/code node's glyph
/// runs as editable `path` siblings, and return the standard transaction result.
///
/// This entrypoint is intentionally separate from `run_transaction`: ordinary
/// transactions stay pure and font-provider-free, while text outlines use the
/// scene compiler as the single source of truth for shaping, fallback, wrapping,
/// OpenType features, and glyph positions.
pub fn materialize_text_outlines(
    doc: &Document,
    fonts: &dyn FontProvider,
    request: &TextOutlineRequest,
) -> Result<TxResult, TxError> {
    let source_before = format_source(doc, "source_before")?;
    let mut diagnostics = Vec::new();
    let mut affected = Vec::new();
    let mut candidate = doc.clone();

    let Some(outline_paint) = source_outline_paint(&candidate, &request.node, &mut diagnostics)
    else {
        return finish_candidate(source_before, candidate, diagnostics, affected);
    };

    let mut paths = Vec::new();
    for page_index in 0..doc.body.pages.len() {
        let result = compile_page(doc, fonts, page_index, None);
        diagnostics.extend(result.diagnostics);
        match outline_source_glyph_run_commands(
            &request.node,
            &request.id_prefix,
            &result.scene.commands,
            fonts,
        ) {
            Ok(mut page_paths) => paths.append(&mut page_paths),
            Err(error) => {
                diagnostics.push(Diagnostic::error(
                    "tx.text_outline_failed",
                    format!(
                        "text outline materialization for node '{}' failed: {}",
                        request.node, error.message
                    ),
                    None,
                    Some(request.node.clone()),
                ));
                return finish_candidate(source_before, candidate, diagnostics, affected);
            }
        }
    }

    if paths.is_empty() {
        diagnostics.push(Diagnostic::error(
            "tx.no_text_outlines",
            format!(
                "text outline materialization for node '{}' produced no path geometry",
                request.node
            ),
            None,
            Some(request.node.clone()),
        ));
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
            if let Some(id) = node_id_of(node) {
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
                | Node::Footnote(_)
                | Node::Toc(_)
                | Node::Table(_)
                | Node::Shape(_)
                | Node::Connector(_)
                | Node::Unknown(_)
                | Node::Pattern(_)
                | Node::Chart(_)
                | Node::Light(_)
                | Node::Mesh(_) => {
                    diagnostics.push(Diagnostic::error(
                        "tx.unsupported_property",
                        format!(
                            "materialize_text_outlines is not supported on a {} node",
                            node_kind_str(node)
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
    if let Some(index) = children
        .iter()
        .position(|node| node_id_of(node) == Some(node_id))
    {
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
