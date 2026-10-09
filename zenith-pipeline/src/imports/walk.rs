//! Node-tree walks and page-size comparison used by root-target validation.

use std::collections::BTreeSet;

use zenith_core::{Dimension, InstanceNode, Node, Page, dim_to_px};

/// Recursively collect every authored node id in `nodes`, descending into
/// `frame`/`group` containers and `table` cells (mirrors the scene's node walk).
pub(super) fn collect_all_node_ids(nodes: &[Node], out: &mut BTreeSet<String>) {
    for node in nodes {
        match node {
            Node::Rect(n) => {
                out.insert(n.id.clone());
            }
            Node::Ellipse(n) => {
                out.insert(n.id.clone());
            }
            Node::Line(n) => {
                out.insert(n.id.clone());
            }
            Node::Text(n) => {
                out.insert(n.id.clone());
            }
            Node::Code(n) => {
                out.insert(n.id.clone());
            }
            Node::Image(n) => {
                out.insert(n.id.clone());
            }
            Node::Polygon(n) => {
                out.insert(n.id.clone());
            }
            Node::Polyline(n) => {
                out.insert(n.id.clone());
            }
            Node::Path(n) => {
                out.insert(n.id.clone());
            }
            Node::Frame(n) => {
                out.insert(n.id.clone());
                collect_all_node_ids(&n.children, out);
            }
            Node::Group(n) => {
                out.insert(n.id.clone());
                collect_all_node_ids(&n.children, out);
            }
            Node::Instance(n) => {
                out.insert(n.id.clone());
            }
            Node::Field(n) => {
                out.insert(n.id.clone());
            }
            Node::Toc(n) => {
                out.insert(n.id.clone());
            }
            Node::Footnote(n) => {
                out.insert(n.id.clone());
            }
            Node::Table(n) => {
                out.insert(n.id.clone());
                for row in &n.rows {
                    for cell in &row.cells {
                        collect_all_node_ids(&cell.children, out);
                    }
                }
            }
            Node::Shape(n) => {
                out.insert(n.id.clone());
            }
            Node::Connector(n) => {
                out.insert(n.id.clone());
            }
            Node::Pattern(n) => {
                out.insert(n.id.clone());
            }
            Node::Chart(n) => {
                out.insert(n.id.clone());
            }
            Node::Light(n) => {
                out.insert(n.id.clone());
            }
            Node::Mesh(n) => {
                out.insert(n.id.clone());
            }
            Node::Unknown(_) => {}
        }
    }
}

/// Recursively collect every `instance` node in `nodes`, descending into
/// `frame`/`group` containers and `table` cells.
pub(super) fn collect_instances<'a>(nodes: &'a [Node], out: &mut Vec<&'a InstanceNode>) {
    for node in nodes {
        match node {
            Node::Instance(n) => out.push(n),
            Node::Frame(n) => collect_instances(&n.children, out),
            Node::Group(n) => collect_instances(&n.children, out),
            Node::Table(n) => {
                for row in &n.rows {
                    for cell in &row.cells {
                        collect_instances(&cell.children, out);
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

pub(super) fn same_page_size(host: &Page, imported: &Page) -> bool {
    same_dimension(&host.width, &imported.width) && same_dimension(&host.height, &imported.height)
}

fn same_dimension(left: &Dimension, right: &Dimension) -> bool {
    match (
        dim_to_px(left.value, &left.unit),
        dim_to_px(right.value, &right.unit),
    ) {
        (Some(left_px), Some(right_px)) => (left_px - right_px).abs() <= f64::EPSILON,
        _ => left == right,
    }
}
