//! Path anchor movement ops.

use zenith_core::{Diagnostic, Document, Node};
use zenith_geometry::{GeometryError, Point2};

use super::super::{find_node_any_mut, px, record_affected};
use super::path_contour::path_contour_mut;
use super::{anchor_coordinate, invalid_anchor, optional_handle, unknown_node};

pub(crate) struct MovePathAnchorArgs<'a> {
    pub(crate) node_id: &'a str,
    pub(crate) subpath_index: Option<usize>,
    pub(crate) anchor_index: usize,
    pub(crate) dx: f64,
    pub(crate) dy: f64,
}

pub(crate) fn apply_move_path_anchor(
    args: MovePathAnchorArgs<'_>,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    let MovePathAnchorArgs {
        node_id,
        subpath_index,
        anchor_index,
        dx,
        dy,
    } = args;

    match find_node_any_mut(doc, node_id) {
        None => diagnostics.push(unknown_node(node_id)),
        Some(node) => {
            let kind = node.kind_str();
            match node {
                Node::Path(path) => {
                    let Some(contour) = path_contour_mut(
                        node_id,
                        "move_path_anchor",
                        path,
                        subpath_index,
                        diagnostics,
                    ) else {
                        return;
                    };
                    if !dx.is_finite() || !dy.is_finite() {
                        diagnostics.push(Diagnostic::error(
                            "tx.invalid_geometry",
                            "move_path_anchor dx and dy must be finite",
                            None,
                            Some(node_id.to_owned()),
                        ));
                        return;
                    }

                    let Some(anchor) = contour.anchors.get(anchor_index) else {
                        diagnostics.push(out_of_range(node_id, anchor_index));
                        return;
                    };

                    let x = match anchor_coordinate(node_id, &anchor.x, "x") {
                        Ok(Some(x)) => x,
                        Ok(None) => {
                            diagnostics.push(invalid_anchor(
                                node_id,
                                "path anchor is missing required x coordinate",
                            ));
                            return;
                        }
                        Err(diagnostic) => {
                            diagnostics.push(diagnostic);
                            return;
                        }
                    };
                    let y = match anchor_coordinate(node_id, &anchor.y, "y") {
                        Ok(Some(y)) => y,
                        Ok(None) => {
                            diagnostics.push(invalid_anchor(
                                node_id,
                                "path anchor is missing required y coordinate",
                            ));
                            return;
                        }
                        Err(diagnostic) => {
                            diagnostics.push(diagnostic);
                            return;
                        }
                    };
                    if let Err(error) = Point2::new(x, y) {
                        diagnostics.push(move_anchor_geometry_diagnostic(node_id, error));
                        return;
                    }
                    let in_handle = match optional_handle(node_id, &anchor.in_x, &anchor.in_y, "in")
                    {
                        Ok(handle) => handle,
                        Err(diagnostic) => {
                            diagnostics.push(diagnostic);
                            return;
                        }
                    };
                    let out_handle =
                        match optional_handle(node_id, &anchor.out_x, &anchor.out_y, "out") {
                            Ok(handle) => handle,
                            Err(diagnostic) => {
                                diagnostics.push(diagnostic);
                                return;
                            }
                        };

                    let moved_x = x + dx;
                    let moved_y = y + dy;
                    if let Err(error) = Point2::new(moved_x, moved_y) {
                        diagnostics.push(move_anchor_geometry_diagnostic(node_id, error));
                        return;
                    }
                    let moved_in_handle = match in_handle {
                        Some(point) => {
                            let moved = Point2::new(point.x + dx, point.y + dy);
                            match moved {
                                Ok(point) => Some(point),
                                Err(error) => {
                                    diagnostics
                                        .push(move_anchor_geometry_diagnostic(node_id, error));
                                    return;
                                }
                            }
                        }
                        None => None,
                    };
                    let moved_out_handle = match out_handle {
                        Some(point) => {
                            let moved = Point2::new(point.x + dx, point.y + dy);
                            match moved {
                                Ok(point) => Some(point),
                                Err(error) => {
                                    diagnostics
                                        .push(move_anchor_geometry_diagnostic(node_id, error));
                                    return;
                                }
                            }
                        }
                        None => None,
                    };

                    let Some(anchor) = contour.anchors.get_mut(anchor_index) else {
                        diagnostics.push(out_of_range(node_id, anchor_index));
                        return;
                    };
                    anchor.x = Some(px(moved_x));
                    anchor.y = Some(px(moved_y));
                    if let Some(point) = moved_in_handle {
                        anchor.in_x = Some(px(point.x));
                        anchor.in_y = Some(px(point.y));
                    }
                    if let Some(point) = moved_out_handle {
                        anchor.out_x = Some(px(point.x));
                        anchor.out_y = Some(px(point.y));
                    }
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
                | Node::Unknown(_) => {
                    diagnostics.push(Diagnostic::error(
                        "tx.unsupported_property",
                        format!("move_path_anchor is not supported on a {} node", kind),
                        None,
                        Some(node_id.to_owned()),
                    ));
                }
            }
        }
    }
}

fn out_of_range(node_id: &str, anchor_index: usize) -> Diagnostic {
    Diagnostic::error(
        "tx.out_of_range",
        format!("anchor_index {anchor_index} is out of range for path '{node_id}'"),
        None,
        Some(node_id.to_owned()),
    )
}

fn move_anchor_geometry_diagnostic(node_id: &str, error: GeometryError) -> Diagnostic {
    let message = match error {
        GeometryError::NonFinitePoint => "move_path_anchor path coordinates must be finite",
        GeometryError::NonFiniteParameter
        | GeometryError::ParameterOutOfRange
        | GeometryError::NonFiniteTolerance
        | GeometryError::NonPositiveTolerance
        | GeometryError::NonPositiveCount
        | GeometryError::CountOutOfRange
        | GeometryError::DegenerateLine
        | GeometryError::InvalidContour
        | GeometryError::NonFiniteTransform
        | GeometryError::SingularTransform => "move_path_anchor geometry is invalid",
    };

    Diagnostic::error(
        "tx.invalid_geometry",
        message,
        None,
        Some(node_id.to_owned()),
    )
}
