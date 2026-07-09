//! Connector port-map builders: `endpoint node → port id → PortTarget`.

use std::collections::BTreeMap;

use zenith_core::{ComponentDef, Node, Page};

use super::common::resolve_imported_component;
use crate::compile::imports::ImportScopes;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::compile) struct PortTarget {
    pub(in crate::compile) node_id: String,
    pub(in crate::compile) anchor: String,
}

pub(in crate::compile) fn build_port_map(
    page: &Page,
    components: &BTreeMap<&str, &ComponentDef>,
    imports: &ImportScopes<'_>,
) -> BTreeMap<String, BTreeMap<String, PortTarget>> {
    let mut map: BTreeMap<String, BTreeMap<String, PortTarget>> = BTreeMap::new();
    for port in &page.ports {
        insert_port(&mut map, &port.node, &port.id, &port.node, &port.anchor);
    }
    collect_component_ports(&page.children, components, imports, "", &mut map);
    map
}

fn insert_port(
    map: &mut BTreeMap<String, BTreeMap<String, PortTarget>>,
    endpoint_node: &str,
    port: &str,
    target_node: &str,
    anchor: &str,
) {
    map.entry(endpoint_node.to_owned())
        .or_default()
        .entry(port.to_owned())
        .or_insert_with(|| PortTarget {
            node_id: target_node.to_owned(),
            anchor: anchor.to_owned(),
        });
}

fn collect_component_ports(
    children: &[Node],
    components: &BTreeMap<&str, &ComponentDef>,
    imports: &ImportScopes<'_>,
    prefix: &str,
    map: &mut BTreeMap<String, BTreeMap<String, PortTarget>>,
) {
    for child in children {
        match child {
            Node::Instance(i) => {
                let instance_id = format!("{prefix}{}", i.id);
                if let Some(source) = i.source.as_deref() {
                    collect_imported_component_ports(source, imports, &instance_id, map);
                    continue;
                }
                if let Some(component_id) = i.component.as_deref()
                    && let Some(component) = components.get(component_id)
                {
                    let child_prefix = format!("{instance_id}/");
                    for port in &component.ports {
                        let target_node = format!("{child_prefix}{}", port.node);
                        insert_port(map, &instance_id, &port.id, &target_node, &port.anchor);
                    }
                    collect_component_ports(
                        &component.children,
                        components,
                        imports,
                        &child_prefix,
                        map,
                    );
                }
            }
            Node::Frame(f) => {
                collect_component_ports(&f.children, components, imports, prefix, map)
            }
            Node::Group(g) => {
                collect_component_ports(&g.children, components, imports, prefix, map)
            }
            Node::Table(t) => {
                for row in &t.rows {
                    for cell in &row.cells {
                        collect_component_ports(&cell.children, components, imports, prefix, map);
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

fn collect_imported_component_ports(
    source: &str,
    imports: &ImportScopes<'_>,
    instance_id: &str,
    map: &mut BTreeMap<String, BTreeMap<String, PortTarget>>,
) {
    let Some((imported, component)) = resolve_imported_component(source, imports) else {
        return;
    };

    let child_prefix = format!("{instance_id}/");
    for port in &component.ports {
        let target_node = format!("{child_prefix}{}", port.node);
        insert_port(map, instance_id, &port.id, &target_node, &port.anchor);
    }
    collect_component_ports(
        &component.children,
        &imported.components,
        imports,
        &child_prefix,
        map,
    );
}
