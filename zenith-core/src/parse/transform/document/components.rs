//! The document-level `components { … }` block and the `project` metadata block.

use kdl::{KdlNode, KdlValue};

use crate::ast::document::{ComponentDef, Project};
use crate::error::ParseError;
use crate::parse::transform::helpers::{node_span, required_string_prop};
use crate::parse::transform::node::transform_node;
use crate::parse::transform::page::transform_ports;

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Transform the document-level `components { … }` block into a list of
/// [`ComponentDef`]. Each `component id="..." { <child nodes> }` becomes one
/// definition whose children are parsed exactly like page/group children (via
/// [`crate::parse::transform::node::transform_node`]). Non-`component` children
/// inside the block are silently ignored (forward-compat).
pub(super) fn transform_components(node: &KdlNode) -> Result<Vec<ComponentDef>, ParseError> {
    let mut defs: Vec<ComponentDef> = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() == "component" {
                defs.push(transform_component_def(child)?);
            }
        }
    }
    Ok(defs)
}

fn transform_component_def(node: &KdlNode) -> Result<ComponentDef, ParseError> {
    let id = required_string_prop(node, "id")?.to_owned();
    let mut ports = Vec::new();
    let mut children = Vec::new();
    if let Some(doc) = node.children() {
        for child in doc.nodes() {
            match child.name().value() {
                "ports" => ports.extend(transform_ports(child)?),
                _ => children.push(transform_node(child)?),
            }
        }
    }
    Ok(ComponentDef {
        id,
        ports,
        children,
        source_span: node_span(node),
    })
}

// ---------------------------------------------------------------------------
// Project
// ---------------------------------------------------------------------------

pub(super) fn transform_project(node: &KdlNode) -> Result<Project, ParseError> {
    let id = required_string_prop(node, "id")?.to_owned();
    let name = required_string_prop(node, "name")?.to_owned();
    let author = node.children().and_then(|doc| {
        doc.nodes()
            .iter()
            .find(|n| n.name().value() == "author")
            .and_then(|n| n.get(0))
            .and_then(|v| {
                if let KdlValue::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            })
    });
    Ok(Project { id, name, author })
}
