use zenith_core::{Diagnostic, Document, Node, PathNode};
use zenith_geometry::{AffineTransform, GeometryError, Point2};

use super::{find_node_shared, node_id_of, node_kind_str, record_affected, subtree_contains};
use crate::engine::path::{
    geometry_anchor_to_core, reject_compound_path, resolved_path_geometry, unknown_node,
};

pub(super) struct MakePathSymmetricArgs<'a> {
    pub node_id: &'a str,
    pub id_prefix: &'a str,
    pub count: usize,
    pub cx: f64,
    pub cy: f64,
    pub start_angle_degrees: f64,
}

pub(super) fn apply_make_path_symmetric(
    args: MakePathSymmetricArgs<'_>,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    let source = match source_path(doc, args.node_id, diagnostics) {
        Some(source) => source,
        None => return,
    };
    let copies = match symmetry_copies(&source, &args, diagnostics) {
        Some(copies) => copies,
        None => return,
    };

    if copies.is_empty() {
        return;
    }

    for page in &mut doc.body.pages {
        if subtree_contains_path(&page.children, args.node_id) {
            insert_after_source(&mut page.children, args.node_id, &copies);
            for index in 1..args.count {
                record_affected(&format!("{}{}", args.id_prefix, index), affected);
            }
            return;
        }
    }
}

fn source_path(
    doc: &Document,
    node_id: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<PathNode> {
    for page in &doc.body.pages {
        if let Some(node) = find_node_shared(&page.children, node_id) {
            return match node {
                Node::Path(path) => {
                    if reject_compound_path(node_id, "make_path_symmetric", path, diagnostics) {
                        None
                    } else {
                        Some(path.clone())
                    }
                }
                Node::Rect(_)
                | Node::Ellipse(_)
                | Node::Line(_)
                | Node::Text(_)
                | Node::Code(_)
                | Node::Frame(_)
                | Node::Group(_)
                | Node::Image(_)
                | Node::Polygon(_)
                | Node::Polyline(_)
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
                            "make_path_symmetric is not supported on a {} node",
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

    diagnostics.push(unknown_node(node_id));
    None
}

fn symmetry_copies(
    source: &PathNode,
    args: &MakePathSymmetricArgs<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Vec<Node>> {
    if args.count < 2 {
        diagnostics.push(Diagnostic::error(
            "tx.invalid_geometry",
            "make_path_symmetric count must be at least 2",
            None,
            Some(args.node_id.to_owned()),
        ));
        return None;
    }

    let center = match Point2::new(args.cx, args.cy) {
        Ok(center) => center,
        Err(error) => {
            diagnostics.push(symmetry_geometry_diagnostic(args.node_id, error));
            return None;
        }
    };
    let transforms =
        match AffineTransform::radial_symmetry(args.count, center, args.start_angle_degrees) {
            Ok(transforms) => transforms,
            Err(error) => {
                diagnostics.push(symmetry_geometry_diagnostic(args.node_id, error));
                return None;
            }
        };
    let geometry =
        match resolved_path_geometry(args.node_id, &source.anchors, source.closed == Some(true)) {
            Ok(geometry) => geometry,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                return None;
            }
        };

    let mut copies = Vec::with_capacity(args.count.saturating_sub(1));
    for (index, transform) in transforms.iter().copied().enumerate().skip(1) {
        let transformed = match geometry.transform(transform) {
            Ok(transformed) => transformed,
            Err(error) => {
                diagnostics.push(symmetry_geometry_diagnostic(args.node_id, error));
                return None;
            }
        };
        let mut copy = source.clone();
        copy.id = format!("{}{}", args.id_prefix, index);
        copy.anchors = transformed
            .anchors()
            .iter()
            .zip(source.anchors.iter())
            .map(|(anchor, original)| geometry_anchor_to_core(*anchor, original.kind.clone()))
            .collect();
        copy.source_span = None;
        copies.push(Node::Path(copy));
    }

    Some(copies)
}

fn insert_after_source(children: &mut Vec<Node>, node_id: &str, copies: &[Node]) -> bool {
    if let Some(index) = children
        .iter()
        .position(|node| node_id_of(node) == Some(node_id))
    {
        children.splice(index + 1..index + 1, copies.iter().cloned());
        return true;
    }

    for child in children.iter_mut() {
        match child {
            Node::Frame(frame) => {
                if insert_after_source(&mut frame.children, node_id, copies) {
                    return true;
                }
            }
            Node::Group(group) => {
                if insert_after_source(&mut group.children, node_id, copies) {
                    return true;
                }
            }
            Node::Table(table) => {
                for row in &mut table.rows {
                    for cell in &mut row.cells {
                        if insert_after_source(&mut cell.children, node_id, copies) {
                            return true;
                        }
                    }
                }
            }
            Node::Unknown(unknown) => {
                if insert_after_source(&mut unknown.children, node_id, copies) {
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
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_) => {}
        }
    }

    false
}

fn subtree_contains_path(children: &[Node], node_id: &str) -> bool {
    children.iter().any(|node| subtree_contains(node, node_id))
}

fn symmetry_geometry_diagnostic(node_id: &str, error: GeometryError) -> Diagnostic {
    let message = match error {
        GeometryError::NonFinitePoint => "make_path_symmetric center coordinates must be finite",
        GeometryError::NonFiniteParameter => "make_path_symmetric parameters must be finite",
        GeometryError::NonPositiveCount => "make_path_symmetric count must be positive",
        GeometryError::CountOutOfRange => {
            "make_path_symmetric count is outside the supported range"
        }
        GeometryError::NonFiniteTransform => "make_path_symmetric produced a non-finite transform",
        GeometryError::SingularTransform => "make_path_symmetric transform is singular",
        GeometryError::DegenerateLine
        | GeometryError::InvalidContour
        | GeometryError::ParameterOutOfRange
        | GeometryError::NonFiniteTolerance
        | GeometryError::NonPositiveTolerance => "make_path_symmetric geometry is invalid",
    };

    Diagnostic::error(
        "tx.invalid_geometry",
        message,
        None,
        Some(node_id.to_owned()),
    )
}
