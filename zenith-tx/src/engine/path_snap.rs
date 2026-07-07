//! Path snapping op application.

use zenith_core::{Diagnostic, Document, Node};
use zenith_geometry::{AffineTransform, GeometryError, PathGeometry, nearest_path_geometry_points};

use super::{find_node_any_mut, node_kind_str, record_affected};
use crate::engine::path::{
    geometry_anchor_to_core, reject_compound_path, resolved_path_geometry, unknown_node,
};

pub(super) fn apply_snap_path_anchors(
    node_id: &str,
    target_id: &str,
    tolerance: f64,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    let source_geometry = match path_geometry_from_doc(doc, node_id) {
        Ok(geometry) => geometry,
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            return;
        }
    };
    let target_geometry = match path_geometry_from_doc(doc, target_id) {
        Ok(geometry) => geometry,
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            return;
        }
    };

    let nearest = match nearest_path_geometry_points(&source_geometry, &target_geometry, tolerance)
    {
        Ok(Some(nearest)) => nearest,
        Ok(None) => {
            diagnostics.push(Diagnostic::error(
                "tx.invalid_geometry",
                "snap_path_anchors requires both paths to have segments",
                None,
                Some(node_id.to_owned()),
            ));
            return;
        }
        Err(error) => {
            diagnostics.push(snap_geometry_diagnostic(node_id, error));
            return;
        }
    };
    let tolerance_squared = tolerance * tolerance;
    if !tolerance_squared.is_finite() {
        diagnostics.push(snap_geometry_diagnostic(
            node_id,
            GeometryError::CountOutOfRange,
        ));
        return;
    }
    if nearest.distance_squared > tolerance_squared {
        diagnostics.push(Diagnostic::error(
            "tx.invalid_geometry",
            "snap_path_anchors found no target path boundary within tolerance",
            None,
            Some(node_id.to_owned()),
        ));
        return;
    }

    let dx = nearest.second_point.x - nearest.first_point.x;
    let dy = nearest.second_point.y - nearest.first_point.y;
    let affine = match AffineTransform::translation(dx, dy) {
        Ok(affine) => affine,
        Err(error) => {
            diagnostics.push(snap_geometry_diagnostic(node_id, error));
            return;
        }
    };
    let snapped = match source_geometry.transform(affine) {
        Ok(snapped) => snapped,
        Err(error) => {
            diagnostics.push(snap_geometry_diagnostic(node_id, error));
            return;
        }
    };

    match find_node_any_mut(doc, node_id) {
        None => diagnostics.push(unknown_node(node_id)),
        Some(node) => {
            let kind = node_kind_str(node);
            match node {
                Node::Path(path) => {
                    if reject_compound_path(node_id, "snap_path_anchors", path, diagnostics) {
                        return;
                    }
                    path.anchors = snapped
                        .anchors()
                        .iter()
                        .zip(path.anchors.iter())
                        .map(|(anchor, original)| {
                            geometry_anchor_to_core(*anchor, original.kind.clone())
                        })
                        .collect();
                    record_affected(node_id, affected);
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
                | Node::Pattern(_)
                | Node::Chart(_)
                | Node::Light(_)
                | Node::Mesh(_)
                | Node::Unknown(_) => diagnostics.push(Diagnostic::error(
                    "tx.unsupported_property",
                    format!("snap_path_anchors is not supported on a {kind} node"),
                    None,
                    Some(node_id.to_owned()),
                )),
            }
        }
    }
}

fn path_geometry_from_doc(doc: &Document, node_id: &str) -> Result<PathGeometry, Diagnostic> {
    match find_node_any(doc, node_id) {
        None => Err(unknown_node(node_id)),
        Some(Node::Path(path)) => {
            if !path.subpaths.is_empty() {
                return Err(Diagnostic::error(
                    "tx.unsupported_property",
                    format!("snap_path_anchors is not supported on compound path '{node_id}'"),
                    None,
                    Some(node_id.to_owned()),
                ));
            }
            resolved_path_geometry(node_id, &path.anchors, path.closed == Some(true))
        }
        Some(node) => Err(Diagnostic::error(
            "tx.unsupported_property",
            format!(
                "snap_path_anchors requires a path node, got {}",
                node_kind_str(node)
            ),
            None,
            Some(node_id.to_owned()),
        )),
    }
}

fn find_node_any<'doc>(doc: &'doc Document, id: &str) -> Option<&'doc Node> {
    doc.body
        .pages
        .iter()
        .find_map(|page| find_in_children_any(&page.children, id))
}

fn find_in_children_any<'a>(children: &'a [Node], id: &str) -> Option<&'a Node> {
    for node in children {
        if super::node_id_of(node) == Some(id) {
            return Some(node);
        }
        match node {
            Node::Frame(f) => {
                if let Some(found) = find_in_children_any(&f.children, id) {
                    return Some(found);
                }
            }
            Node::Group(g) => {
                if let Some(found) = find_in_children_any(&g.children, id) {
                    return Some(found);
                }
            }
            Node::Table(t) => {
                for row in &t.rows {
                    for cell in &row.cells {
                        if let Some(found) = find_in_children_any(&cell.children, id) {
                            return Some(found);
                        }
                    }
                }
            }
            Node::Unknown(u) => {
                if let Some(found) = find_in_children_any(&u.children, id) {
                    return Some(found);
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
    None
}

fn snap_geometry_diagnostic(node_id: &str, error: GeometryError) -> Diagnostic {
    match error {
        GeometryError::NonFiniteTolerance => Diagnostic::error(
            "tx.invalid_geometry_tolerance",
            "snap_path_anchors tolerance must be finite",
            None,
            Some(node_id.to_owned()),
        ),
        GeometryError::NonPositiveTolerance => Diagnostic::error(
            "tx.invalid_geometry_tolerance",
            "snap_path_anchors tolerance must be positive",
            None,
            Some(node_id.to_owned()),
        ),
        GeometryError::NonFinitePoint => Diagnostic::error(
            "tx.invalid_geometry",
            "snap_path_anchors path coordinates must be finite",
            None,
            Some(node_id.to_owned()),
        ),
        GeometryError::NonFiniteParameter
        | GeometryError::ParameterOutOfRange
        | GeometryError::NonPositiveCount
        | GeometryError::CountOutOfRange
        | GeometryError::DegenerateLine
        | GeometryError::InvalidContour
        | GeometryError::NonFiniteTransform
        | GeometryError::SingularTransform => Diagnostic::error(
            "tx.invalid_geometry",
            "snap_path_anchors geometry is invalid",
            None,
            Some(node_id.to_owned()),
        ),
    }
}
