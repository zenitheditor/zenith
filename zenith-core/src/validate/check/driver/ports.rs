//! Connector-port collection helpers used by the document walk: building the
//! per-node declared-port map and folding in component-instance ports.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::document::{ComponentDef, PortDef};
use crate::ast::node::{ConnectorAnchorParseError, Node, parse_connector_anchor};
use crate::diagnostics::Diagnostic;

pub(super) fn build_ports_by_node(
    ports: &[PortDef],
    local_node_ids: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut ports_by_node: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for port in ports {
        if !local_node_ids.contains(&port.node) {
            diagnostics.push(Diagnostic::warning(
                "connector.port_invalid_target",
                format!(
                    "port '{}#{}' targets unknown node '{}'",
                    port.node, port.id, port.node
                ),
                port.source_span,
                Some(port.node.clone()),
            ));
        }
        let ids = ports_by_node.entry(port.node.clone()).or_default();
        if !ids.insert(port.id.clone()) {
            diagnostics.push(Diagnostic::warning(
                "connector.port_duplicate",
                format!(
                    "port '{}#{}' is declared more than once for node '{}'",
                    port.node, port.id, port.node
                ),
                port.source_span,
                Some(port.node.clone()),
            ));
        }
        match parse_connector_anchor(&port.anchor) {
            Ok(_) => {}
            Err(ConnectorAnchorParseError::ZeroCount) => {
                diagnostics.push(Diagnostic::warning(
                    "connector.invalid_anchor",
                    format!(
                        "port '{}#{}': anchor '{}' has a divided anchor count of 0",
                        port.node, port.id, port.anchor
                    ),
                    port.source_span,
                    Some(port.node.clone()),
                ));
            }
            Err(ConnectorAnchorParseError::IndexOutOfRange { index, count }) => {
                diagnostics.push(Diagnostic::warning(
                    "connector.invalid_anchor",
                    format!(
                        "port '{}#{}': anchor '{}' has index {index} outside divided anchor count {count}",
                        port.node, port.id, port.anchor
                    ),
                    port.source_span,
                    Some(port.node.clone()),
                ));
            }
            Err(ConnectorAnchorParseError::InvalidSyntax) => {
                diagnostics.push(Diagnostic::warning(
                    "connector.invalid_anchor",
                    format!(
                        "port '{}#{}': anchor '{}' is not 'auto', a divided anchor like '4/16', or a nine-point anchor (top/center/bottom × left/center/right, e.g. bottom-right)",
                        port.node, port.id, port.anchor
                    ),
                    port.source_span,
                    Some(port.node.clone()),
                ));
            }
        }
    }
    ports_by_node
}

pub(super) fn add_component_instance_ports(
    children: &[Node],
    components: &BTreeMap<&str, &ComponentDef>,
    prefix: &str,
    ports_by_node: &mut BTreeMap<String, BTreeSet<String>>,
) {
    for child in children {
        match child {
            Node::Instance(instance) => {
                let instance_id = format!("{prefix}{}", instance.id);
                if let Some(component_id) = instance.component.as_deref()
                    && let Some(component) = components.get(component_id)
                {
                    let ports = ports_by_node.entry(instance_id.clone()).or_default();
                    for port in &component.ports {
                        ports.insert(port.id.clone());
                    }
                    let child_prefix = format!("{instance_id}/");
                    add_component_instance_ports(
                        &component.children,
                        components,
                        &child_prefix,
                        ports_by_node,
                    );
                }
            }
            Node::Frame(frame) => {
                add_component_instance_ports(&frame.children, components, prefix, ports_by_node);
            }
            Node::Group(group) => {
                add_component_instance_ports(&group.children, components, prefix, ports_by_node);
            }
            Node::Table(table) => {
                for row in &table.rows {
                    for cell in &row.cells {
                        add_component_instance_ports(
                            &cell.children,
                            components,
                            prefix,
                            ports_by_node,
                        );
                    }
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
            | Node::Field(_)
            | Node::Footnote(_)
            | Node::Toc(_)
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
