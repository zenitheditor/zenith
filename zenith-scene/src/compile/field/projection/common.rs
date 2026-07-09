//! Shared node helpers for the projection submodules: node-id extraction and
//! imported-component resolution used by the node-box and connector-target
//! walks alike.

use zenith_core::{ComponentDef, Node};

use crate::compile::imports::{ImportScopes, ImportSource, ImportedScope, parse_import_source};

/// The id of a node, or `None` for `Unknown`.
pub(super) fn node_id(node: &Node) -> Option<&str> {
    match node {
        Node::Rect(n) => Some(&n.id),
        Node::Ellipse(n) => Some(&n.id),
        Node::Line(n) => Some(&n.id),
        Node::Text(n) => Some(&n.id),
        Node::Code(n) => Some(&n.id),
        Node::Frame(n) => Some(&n.id),
        Node::Group(n) => Some(&n.id),
        Node::Image(n) => Some(&n.id),
        Node::Polygon(n) => Some(&n.id),
        Node::Polyline(n) => Some(&n.id),
        Node::Path(n) => Some(&n.id),
        Node::Instance(n) => Some(&n.id),
        Node::Field(n) => Some(&n.id),
        Node::Toc(n) => Some(&n.id),
        Node::Footnote(n) => Some(&n.id),
        Node::Table(n) => Some(&n.id),
        Node::Shape(n) => Some(&n.id),
        Node::Connector(n) => Some(&n.id),
        Node::Pattern(n) => Some(&n.id),
        Node::Chart(n) => Some(&n.id),
        Node::Light(n) => Some(&n.id),
        Node::Mesh(n) => Some(&n.id),
        Node::Unknown(_) => None,
    }
}

pub(super) fn resolve_imported_component<'a>(
    source: &str,
    imports: &'a ImportScopes<'a>,
) -> Option<(&'a ImportedScope<'a>, &'a ComponentDef)> {
    if !imports.is_enabled() {
        return None;
    }
    let ImportSource::Component {
        import_id,
        component_id,
    } = parse_import_source(source)
    else {
        return None;
    };
    let imported = imports.get(import_id)?;
    let component = imported.components.get(component_id)?;
    Some((imported, component))
}
