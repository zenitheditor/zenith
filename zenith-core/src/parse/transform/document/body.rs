//! The `document { … }` body block, its pages, and the shared child-iteration
//! helper.

use kdl::KdlNode;

use crate::ast::UnsupportedChild;
use crate::ast::block_style::BlockStyle;
use crate::ast::document::{DocumentBody, Page};
use crate::ast::node::Node;
use crate::error::ParseError;
use crate::parse::transform::block_style::transform_block_style;
use crate::parse::transform::helpers::{optional_string_prop, required_string_prop};
use crate::parse::transform::node::transform_node;
use crate::parse::transform::page::transform_page;

pub(super) fn transform_document_body(
    node: &KdlNode,
    sink: &mut Vec<UnsupportedChild>,
) -> Result<DocumentBody, ParseError> {
    let id = required_string_prop(node, "id")?.to_owned();
    let title = optional_string_prop(node, "title").map(str::to_owned);

    let mut block_styles: Vec<BlockStyle> = Vec::new();
    let mut pages: Vec<Page> = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "block" => block_styles.push(transform_block_style(child)?),
                "page" => pages.push(transform_page(child, sink)?),
                _ => {}
            }
        }
    }

    Ok(DocumentBody {
        id,
        title,
        block_styles,
        pages,
    })
}

/// Iterate a KDL node's children block and transform each child into a
/// [`Node`].  Returns an empty `Vec` when the node has no children block.
///
/// Both `transform_page` and `transform_group` use this helper to avoid
/// duplicating the child-iteration logic.
///
/// # Known limitation
/// Groups nest recursively via `transform_node` → `transform_group` →
/// `transform_children` with no depth guard.  This is an accepted v0
/// limitation; stack overflow is only possible with pathologically deep trees.
pub(in crate::parse::transform) fn transform_children(
    node: &KdlNode,
    sink: &mut Vec<UnsupportedChild>,
) -> Result<Vec<Node>, ParseError> {
    let mut children: Vec<Node> = Vec::new();
    if let Some(doc) = node.children() {
        for child in doc.nodes() {
            children.push(transform_node(child, sink)?);
        }
    }
    Ok(children)
}
