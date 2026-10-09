//! External-file text sources for `text` nodes with `src="path"`.
//!
//! File reads stay out of the pure scene compile, so every entry point calls
//! [`resolve_text_sources`] after validation and before compiling pages.

use std::path::Path;

use zenith_core::{Diagnostic, Document, Node, TextNode, TextSpan};

use crate::io::SourceFs;

/// For every `text` node with `src = Some(path)` on the pages of `doc`, read
/// the file (relative to `project_dir`) through `fs` and replace the node's
/// spans with one plain span holding the file's UTF-8 text.
///
/// The span has no style of its own: the node's fill and font apply at
/// compile time, and `format="markdown"` re-parses the content.
///
/// A `text.src_missing` Error diagnostic names the node and path when
/// `project_dir` is `None` or the read fails (absent, unreadable, or not
/// UTF-8). The node keeps its spans. The Error severity blocks render output
/// like `asset.missing`.
pub fn resolve_text_sources(
    fs: &dyn SourceFs,
    doc: &mut Document,
    project_dir: Option<&Path>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for page in &mut doc.body.pages {
        resolve_in_nodes(fs, &mut page.children, project_dir, diagnostics);
    }
}

/// Walk `nodes` recursively, resolving `src` on every `Node::Text`.
///
/// Every node variant is listed (exhaustive match), so a future container
/// kind forces a decision here.
fn resolve_in_nodes(
    fs: &dyn SourceFs,
    nodes: &mut [Node],
    project_dir: Option<&Path>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for node in nodes.iter_mut() {
        match node {
            Node::Text(text) => {
                // Clone the path so the borrow of `text.src` ends before
                // `resolve_one` takes `text` mutably.
                if let Some(rel_path) = text.src.clone() {
                    resolve_one(fs, text, &rel_path, project_dir, diagnostics);
                }
            }
            Node::Frame(f) => {
                resolve_in_nodes(fs, &mut f.children, project_dir, diagnostics);
            }
            Node::Group(g) => {
                resolve_in_nodes(fs, &mut g.children, project_dir, diagnostics);
            }
            Node::Table(t) => {
                for row in &mut t.rows {
                    for cell in &mut row.cells {
                        resolve_in_nodes(fs, &mut cell.children, project_dir, diagnostics);
                    }
                }
            }
            // Leaf nodes that cannot contain children — explicit for exhaustiveness:
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Code(_)
            | Node::Image(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Instance(_)
            | Node::Field(_)
            | Node::Toc(_)
            | Node::Footnote(_)
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Unknown(_) => {}
        }
    }
}

/// Load `rel_path` (relative to `project_dir`) and replace `text.spans` with
/// one plain span of its contents, or push `text.src_missing`.
fn resolve_one(
    fs: &dyn SourceFs,
    text: &mut TextNode,
    rel_path: &str,
    project_dir: Option<&Path>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(dir) = project_dir else {
        diagnostics.push(Diagnostic::error(
            "text.src_missing",
            format!(
                "text node '{}': src=\"{}\" cannot be resolved — no project directory available",
                text.id, rel_path
            ),
            text.source_span,
            Some(text.id.clone()),
        ));
        return;
    };

    let full_path = dir.join(rel_path);
    match fs.read_to_string(&full_path) {
        Ok(contents) => {
            text.spans = vec![TextSpan {
                text: contents,
                fill: None,
                font_weight: None,
                font_features: None,
                font_alternates: None,
                letter_spacing: None,
                italic: None,
                underline: None,
                strikethrough: None,
                vertical_align: None,
                footnote_ref: None,
                data_ref: None,
                data_format: None,
                highlight: None,
                code: None,
                link: None,
            }];
        }
        Err(e) => {
            diagnostics.push(Diagnostic::error(
                "text.src_missing",
                format!(
                    "text node '{}': src=\"{}\" could not be read from '{}': {}",
                    text.id,
                    rel_path,
                    full_path.display(),
                    e
                ),
                text.source_span,
                Some(text.id.clone()),
            ));
        }
    }
}
