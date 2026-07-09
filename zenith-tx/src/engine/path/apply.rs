//! Path op application: `set_path_anchors`, `set_path_anchor_kind`,
//! `remove_path_anchor`, `simplify_path_anchors`, `insert_path_anchor`,
//! `insert_path_anchor_at_point`, and `transform_path_anchors`.

use zenith_core::{AnchorKind, Diagnostic, Document, Node, PathAnchor as CorePathAnchor};
use zenith_geometry::{
    CompoundPathGeometry, Point2, fit_cubic_path_anchors_to_points, simplify_polyline,
};

use super::diagnostics::{
    geometry_diagnostic, insert_at_point_geometry_diagnostic, transform_geometry_diagnostic,
    unknown_node,
};
use super::geometry::{
    flattened_path_points, geometry_anchor_to_core, path_has_handles, path_transform,
    resolved_path_geometry, simplified_points_to_core_anchors, split_geometry_anchors,
    transform_core_anchors,
};
use super::path_contour::path_contour_mut;
use crate::engine::{find_node_any_mut, px, record_affected};
use crate::op::{OpPathAnchor, OpPathTransform};

macro_rules! non_path_nodes {
    () => {
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
            | Node::Unknown(_)
    };
}

pub(crate) fn apply_set_path_anchors(
    node_id: &str,
    subpath_index: Option<usize>,
    anchors: &[OpPathAnchor],
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    match find_node_any_mut(doc, node_id) {
        None => diagnostics.push(unknown_node(node_id)),
        Some(node) => {
            let kind = node.kind_str();
            match node {
                Node::Path(path) => {
                    let Some(contour) = path_contour_mut(
                        node_id,
                        "set_path_anchors",
                        path,
                        subpath_index,
                        diagnostics,
                    ) else {
                        return;
                    };
                    *contour.anchors = anchors
                        .iter()
                        .map(|anchor| CorePathAnchor {
                            x: Some(px(anchor.x)),
                            y: Some(px(anchor.y)),
                            kind: anchor.kind.as_deref().map(AnchorKind::from_kind_str),
                            in_x: anchor.in_x.map(px),
                            in_y: anchor.in_y.map(px),
                            out_x: anchor.out_x.map(px),
                            out_y: anchor.out_y.map(px),
                        })
                        .collect();
                    record_affected(node_id, affected);
                }
                non_path_nodes!() => {
                    diagnostics.push(Diagnostic::error(
                        "tx.unsupported_property",
                        format!("set_path_anchors is not supported on a {} node", kind),
                        None,
                        Some(node_id.to_owned()),
                    ));
                }
            }
        }
    }
}

pub(crate) fn apply_set_path_anchor_kind(
    node_id: &str,
    subpath_index: Option<usize>,
    anchor_index: usize,
    kind: Option<&str>,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    match find_node_any_mut(doc, node_id) {
        None => diagnostics.push(unknown_node(node_id)),
        Some(node) => {
            let node_kind = node.kind_str();
            match node {
                Node::Path(path) => {
                    let Some(contour) = path_contour_mut(
                        node_id,
                        "set_path_anchor_kind",
                        path,
                        subpath_index,
                        diagnostics,
                    ) else {
                        return;
                    };
                    let Some(anchor) = contour.anchors.get_mut(anchor_index) else {
                        diagnostics.push(Diagnostic::error(
                            "tx.out_of_range",
                            format!(
                                "anchor_index {anchor_index} is out of range for path '{node_id}'"
                            ),
                            None,
                            Some(node_id.to_owned()),
                        ));
                        return;
                    };

                    anchor.kind = kind.map(AnchorKind::from_kind_str);
                    record_affected(node_id, affected);
                }
                non_path_nodes!() => {
                    diagnostics.push(Diagnostic::error(
                        "tx.unsupported_property",
                        format!(
                            "set_path_anchor_kind is not supported on a {} node",
                            node_kind
                        ),
                        None,
                        Some(node_id.to_owned()),
                    ));
                }
            }
        }
    }
}

pub(crate) fn apply_remove_path_anchor(
    node_id: &str,
    subpath_index: Option<usize>,
    anchor_index: usize,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    match find_node_any_mut(doc, node_id) {
        None => diagnostics.push(unknown_node(node_id)),
        Some(node) => {
            let node_kind = node.kind_str();
            match node {
                Node::Path(path) => {
                    let Some(contour) = path_contour_mut(
                        node_id,
                        "remove_path_anchor",
                        path,
                        subpath_index,
                        diagnostics,
                    ) else {
                        return;
                    };
                    if anchor_index >= contour.anchors.len() {
                        diagnostics.push(Diagnostic::error(
                            "tx.out_of_range",
                            format!(
                                "anchor_index {anchor_index} is out of range for path '{node_id}'"
                            ),
                            None,
                            Some(node_id.to_owned()),
                        ));
                        return;
                    }

                    contour.anchors.remove(anchor_index);
                    record_affected(node_id, affected);
                }
                non_path_nodes!() => {
                    diagnostics.push(Diagnostic::error(
                        "tx.unsupported_property",
                        format!(
                            "remove_path_anchor is not supported on a {} node",
                            node_kind
                        ),
                        None,
                        Some(node_id.to_owned()),
                    ));
                }
            }
        }
    }
}

pub(crate) fn apply_simplify_path_anchors(
    node_id: &str,
    subpath_index: Option<usize>,
    tolerance: f64,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    match find_node_any_mut(doc, node_id) {
        None => diagnostics.push(unknown_node(node_id)),
        Some(node) => {
            let kind = node.kind_str();
            match node {
                Node::Path(path) => {
                    let Some(contour) = path_contour_mut(
                        node_id,
                        "simplify_path_anchors",
                        path,
                        subpath_index,
                        diagnostics,
                    ) else {
                        return;
                    };
                    if contour.closed {
                        diagnostics.push(Diagnostic::error(
                            "tx.unsupported_closed_path",
                            "simplify_path_anchors only supports open paths",
                            None,
                            Some(node_id.to_owned()),
                        ));
                        return;
                    }

                    let tolerance_budget = tolerance / 2.0;
                    let points =
                        match flattened_path_points(node_id, contour.anchors, tolerance_budget) {
                            Ok(points) => points,
                            Err(diagnostic) => {
                                diagnostics.push(diagnostic);
                                return;
                            }
                        };

                    match simplify_polyline(&points, tolerance_budget) {
                        Ok(simplified) => {
                            *contour.anchors = if path_has_handles(contour.anchors) {
                                match fit_cubic_path_anchors_to_points(
                                    &simplified,
                                    tolerance_budget,
                                ) {
                                    Ok(Some(anchors)) => anchors
                                        .into_iter()
                                        .map(|anchor| geometry_anchor_to_core(anchor, None))
                                        .collect(),
                                    Ok(None) => simplified_points_to_core_anchors(&simplified),
                                    Err(error) => {
                                        diagnostics.push(geometry_diagnostic(node_id, error));
                                        return;
                                    }
                                }
                            } else {
                                simplified_points_to_core_anchors(&simplified)
                            };
                            record_affected(node_id, affected);
                        }
                        Err(error) => diagnostics.push(geometry_diagnostic(node_id, error)),
                    }
                }
                non_path_nodes!() => {
                    diagnostics.push(Diagnostic::error(
                        "tx.unsupported_property",
                        format!("simplify_path_anchors is not supported on a {} node", kind),
                        None,
                        Some(node_id.to_owned()),
                    ));
                }
            }
        }
    }
}

pub(crate) fn apply_insert_path_anchor(
    node_id: &str,
    subpath_index: Option<usize>,
    segment_index: usize,
    t: f64,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    match find_node_any_mut(doc, node_id) {
        None => diagnostics.push(unknown_node(node_id)),
        Some(node) => {
            let kind = node.kind_str();
            match node {
                Node::Path(path) => {
                    let Some(contour) = path_contour_mut(
                        node_id,
                        "insert_path_anchor",
                        path,
                        subpath_index,
                        diagnostics,
                    ) else {
                        return;
                    };
                    let geometry =
                        match resolved_path_geometry(node_id, contour.anchors, contour.closed) {
                            Ok(geometry) => geometry,
                            Err(diagnostic) => {
                                diagnostics.push(diagnostic);
                                return;
                            }
                        };
                    *contour.anchors = match split_geometry_anchors(
                        node_id,
                        &geometry,
                        contour.anchors,
                        segment_index,
                        t,
                    ) {
                        Ok(anchors) => anchors,
                        Err(diagnostic) => {
                            diagnostics.push(diagnostic);
                            return;
                        }
                    };
                    record_affected(node_id, affected);
                }
                non_path_nodes!() => {
                    diagnostics.push(Diagnostic::error(
                        "tx.unsupported_property",
                        format!("insert_path_anchor is not supported on a {} node", kind),
                        None,
                        Some(node_id.to_owned()),
                    ));
                }
            }
        }
    }
}

pub(crate) fn apply_insert_path_anchor_at_point(
    node_id: &str,
    x: f64,
    y: f64,
    tolerance: f64,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    match find_node_any_mut(doc, node_id) {
        None => diagnostics.push(unknown_node(node_id)),
        Some(node) => {
            let kind = node.kind_str();
            match node {
                Node::Path(path) => {
                    let point = match Point2::new(x, y) {
                        Ok(point) => point,
                        Err(error) => {
                            diagnostics.push(insert_at_point_geometry_diagnostic(node_id, error));
                            return;
                        }
                    };
                    if !path.subpaths.is_empty() {
                        let mut geometries = Vec::with_capacity(path.subpaths.len());
                        for subpath in &path.subpaths {
                            let geometry = match resolved_path_geometry(
                                node_id,
                                &subpath.anchors,
                                subpath.closed == Some(true),
                            ) {
                                Ok(geometry) => geometry,
                                Err(diagnostic) => {
                                    diagnostics.push(diagnostic);
                                    return;
                                }
                            };
                            geometries.push(geometry);
                        }
                        let compound = CompoundPathGeometry::new(geometries);
                        let projection = match compound.project(point, tolerance) {
                            Ok(Some(projection)) => projection,
                            Ok(None) => {
                                diagnostics.push(Diagnostic::error(
                                    "tx.invalid_geometry",
                                    "insert_path_anchor_at_point requires a path segment to project onto",
                                    None,
                                    Some(node_id.to_owned()),
                                ));
                                return;
                            }
                            Err(error) => {
                                diagnostics
                                    .push(insert_at_point_geometry_diagnostic(node_id, error));
                                return;
                            }
                        };
                        if projection.projection.distance_squared > tolerance * tolerance {
                            diagnostics.push(Diagnostic::error(
                                "tx.invalid_geometry",
                                "insert_path_anchor_at_point found no path projection within tolerance",
                                None,
                                Some(node_id.to_owned()),
                            ));
                            return;
                        }
                        let Some(geometry) = compound.contours().get(projection.contour_index)
                        else {
                            diagnostics.push(Diagnostic::error(
                                "tx.out_of_range",
                                format!(
                                    "subpath_index {} is out of range for path '{node_id}'",
                                    projection.contour_index
                                ),
                                None,
                                Some(node_id.to_owned()),
                            ));
                            return;
                        };
                        let Some(subpath) = path.subpaths.get_mut(projection.contour_index) else {
                            diagnostics.push(Diagnostic::error(
                                "tx.out_of_range",
                                format!(
                                    "subpath_index {} is out of range for path '{node_id}'",
                                    projection.contour_index
                                ),
                                None,
                                Some(node_id.to_owned()),
                            ));
                            return;
                        };
                        subpath.anchors = match split_geometry_anchors(
                            node_id,
                            geometry,
                            &subpath.anchors,
                            projection.projection.segment_index,
                            projection.projection.segment_t,
                        ) {
                            Ok(anchors) => anchors,
                            Err(diagnostic) => {
                                diagnostics.push(diagnostic);
                                return;
                            }
                        };
                        record_affected(node_id, affected);
                        return;
                    }
                    let geometry = match resolved_path_geometry(
                        node_id,
                        &path.anchors,
                        path.closed == Some(true),
                    ) {
                        Ok(geometry) => geometry,
                        Err(diagnostic) => {
                            diagnostics.push(diagnostic);
                            return;
                        }
                    };
                    let projection = match geometry.project(point, tolerance) {
                        Ok(Some(projection)) => projection,
                        Ok(None) => {
                            diagnostics.push(Diagnostic::error(
                                "tx.invalid_geometry",
                                "insert_path_anchor_at_point requires a path segment to project onto",
                                None,
                                Some(node_id.to_owned()),
                            ));
                            return;
                        }
                        Err(error) => {
                            diagnostics.push(insert_at_point_geometry_diagnostic(node_id, error));
                            return;
                        }
                    };
                    if projection.distance_squared > tolerance * tolerance {
                        diagnostics.push(Diagnostic::error(
                            "tx.invalid_geometry",
                            "insert_path_anchor_at_point found no path projection within tolerance",
                            None,
                            Some(node_id.to_owned()),
                        ));
                        return;
                    }

                    path.anchors = match split_geometry_anchors(
                        node_id,
                        &geometry,
                        &path.anchors,
                        projection.segment_index,
                        projection.segment_t,
                    ) {
                        Ok(anchors) => anchors,
                        Err(diagnostic) => {
                            diagnostics.push(diagnostic);
                            return;
                        }
                    };
                    record_affected(node_id, affected);
                }
                non_path_nodes!() => {
                    diagnostics.push(Diagnostic::error(
                        "tx.unsupported_property",
                        format!(
                            "insert_path_anchor_at_point is not supported on a {} node",
                            kind
                        ),
                        None,
                        Some(node_id.to_owned()),
                    ));
                }
            }
        }
    }
}

pub(crate) fn apply_transform_path_anchors(
    node_id: &str,
    transform: &OpPathTransform,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    match find_node_any_mut(doc, node_id) {
        None => diagnostics.push(unknown_node(node_id)),
        Some(node) => {
            let kind = node.kind_str();
            match node {
                Node::Path(path) => {
                    let affine = match path_transform(transform) {
                        Ok(affine) => affine,
                        Err(error) => {
                            diagnostics.push(transform_geometry_diagnostic(node_id, error));
                            return;
                        }
                    };
                    if path.subpaths.is_empty() {
                        path.anchors = match transform_core_anchors(
                            node_id,
                            &path.anchors,
                            path.closed == Some(true),
                            affine,
                        ) {
                            Ok(anchors) => anchors,
                            Err(diagnostic) => {
                                diagnostics.push(diagnostic);
                                return;
                            }
                        };
                    } else {
                        let mut transformed_subpaths = Vec::with_capacity(path.subpaths.len());
                        for subpath in &path.subpaths {
                            let anchors = match transform_core_anchors(
                                node_id,
                                &subpath.anchors,
                                subpath.closed == Some(true),
                                affine,
                            ) {
                                Ok(anchors) => anchors,
                                Err(diagnostic) => {
                                    diagnostics.push(diagnostic);
                                    return;
                                }
                            };
                            transformed_subpaths.push(anchors);
                        }
                        for (subpath, anchors) in path.subpaths.iter_mut().zip(transformed_subpaths)
                        {
                            subpath.anchors = anchors;
                        }
                    }
                    record_affected(node_id, affected);
                }
                non_path_nodes!() => {
                    diagnostics.push(Diagnostic::error(
                        "tx.unsupported_property",
                        format!("transform_path_anchors is not supported on a {} node", kind),
                        None,
                        Some(node_id.to_owned()),
                    ));
                }
            }
        }
    }
}
