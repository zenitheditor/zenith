//! Path handle movement ops and authoring-intent handle constraints.

use zenith_core::{
    AnchorKind, Diagnostic, Dimension, Document, Node, PathAnchor as CorePathAnchor,
};
use zenith_geometry::{GeometryError, Point2};

use crate::op::OpPathHandle;

use super::path::{anchor_coordinate, invalid_anchor, optional_handle, unknown_node};
use super::{find_node_any_mut, node_kind_str, px, record_affected};

#[derive(Debug, Clone, Copy)]
struct HandleReplacements {
    in_handle: Option<Point2>,
    out_handle: Option<Point2>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct MovePathHandleArgs<'a> {
    pub(super) node_id: &'a str,
    pub(super) anchor_index: usize,
    pub(super) handle: OpPathHandle,
    pub(super) dx: f64,
    pub(super) dy: f64,
}

pub(super) fn apply_move_path_handle(
    args: MovePathHandleArgs<'_>,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    let MovePathHandleArgs {
        node_id,
        anchor_index,
        handle,
        dx,
        dy,
    } = args;

    match find_node_any_mut(doc, node_id) {
        None => diagnostics.push(unknown_node(node_id)),
        Some(node) => {
            let kind = node_kind_str(node);
            match node {
                Node::Path(path) => {
                    if !dx.is_finite() || !dy.is_finite() {
                        diagnostics.push(Diagnostic::error(
                            "tx.invalid_geometry",
                            "move_path_handle dx and dy must be finite",
                            None,
                            Some(node_id.to_owned()),
                        ));
                        return;
                    }

                    let Some(anchor) = path.anchors.get(anchor_index) else {
                        diagnostics.push(out_of_range(node_id, anchor_index));
                        return;
                    };

                    let replacements =
                        match moved_handle_replacements(node_id, anchor, handle, dx, dy) {
                            Ok(replacements) => replacements,
                            Err(diagnostic) => {
                                diagnostics.push(diagnostic);
                                return;
                            }
                        };

                    let Some(anchor) = path.anchors.get_mut(anchor_index) else {
                        diagnostics.push(out_of_range(node_id, anchor_index));
                        return;
                    };
                    match handle {
                        OpPathHandle::In => {
                            if let Some(point) = replacements.in_handle {
                                anchor.in_x = Some(px(point.x));
                                anchor.in_y = Some(px(point.y));
                            }
                            if let Some(point) = replacements.out_handle {
                                anchor.out_x = Some(px(point.x));
                                anchor.out_y = Some(px(point.y));
                            }
                        }
                        OpPathHandle::Out => {
                            if let Some(point) = replacements.out_handle {
                                anchor.out_x = Some(px(point.x));
                                anchor.out_y = Some(px(point.y));
                            }
                            if let Some(point) = replacements.in_handle {
                                anchor.in_x = Some(px(point.x));
                                anchor.in_y = Some(px(point.y));
                            }
                        }
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
                        format!("move_path_handle is not supported on a {} node", kind),
                        None,
                        Some(node_id.to_owned()),
                    ));
                }
            }
        }
    }
}

fn moved_handle_replacements(
    node_id: &str,
    anchor: &CorePathAnchor,
    handle: OpPathHandle,
    dx: f64,
    dy: f64,
) -> Result<HandleReplacements, Diagnostic> {
    let anchor_point = required_anchor_point(node_id, anchor)?;
    let selected = required_handle(
        node_id,
        selected_handle_x(anchor, handle),
        selected_handle_y(anchor, handle),
        handle,
    )?;
    let moved_selected = Point2::new(selected.x + dx, selected.y + dy)
        .map_err(|error| move_handle_geometry_diagnostic(node_id, error))?;

    let opposite = match anchor.kind.as_ref() {
        Some(AnchorKind::Smooth) | Some(AnchorKind::Symmetric) => optional_handle(
            node_id,
            opposite_handle_x(anchor, handle),
            opposite_handle_y(anchor, handle),
            opposite_handle_label(handle),
        )?,
        None | Some(AnchorKind::Corner) | Some(AnchorKind::Unknown(_)) => None,
    };

    let moved_opposite = match (anchor.kind.as_ref(), opposite) {
        (Some(AnchorKind::Smooth), Some(opposite)) => {
            let vx = moved_selected.x - anchor_point.x;
            let vy = moved_selected.y - anchor_point.y;
            let moved_distance = vx.hypot(vy);
            if !moved_distance.is_finite() || moved_distance == 0.0 {
                return Err(Diagnostic::error(
                    "tx.invalid_geometry",
                    "move_path_handle cannot preserve smooth handle direction when selected handle lands on the anchor",
                    None,
                    Some(node_id.to_owned()),
                ));
            }
            let old_ox = opposite.x - anchor_point.x;
            let old_oy = opposite.y - anchor_point.y;
            let old_distance = old_ox.hypot(old_oy);
            Point2::new(
                anchor_point.x - (vx / moved_distance) * old_distance,
                anchor_point.y - (vy / moved_distance) * old_distance,
            )
            .map(Some)
            .map_err(|error| move_handle_geometry_diagnostic(node_id, error))?
        }
        (Some(AnchorKind::Symmetric), Some(_)) => Point2::new(
            anchor_point.x - (moved_selected.x - anchor_point.x),
            anchor_point.y - (moved_selected.y - anchor_point.y),
        )
        .map(Some)
        .map_err(|error| move_handle_geometry_diagnostic(node_id, error))?,
        (Some(AnchorKind::Smooth) | Some(AnchorKind::Symmetric), None) => None,
        (None | Some(AnchorKind::Corner) | Some(AnchorKind::Unknown(_)), existing) => existing,
    };

    match handle {
        OpPathHandle::In => Ok(HandleReplacements {
            in_handle: Some(moved_selected),
            out_handle: moved_opposite,
        }),
        OpPathHandle::Out => Ok(HandleReplacements {
            in_handle: moved_opposite,
            out_handle: Some(moved_selected),
        }),
    }
}

fn required_anchor_point(node_id: &str, anchor: &CorePathAnchor) -> Result<Point2, Diagnostic> {
    let Some(x) = anchor_coordinate(node_id, &anchor.x, "x")? else {
        return Err(invalid_anchor(
            node_id,
            "path anchor is missing required x coordinate",
        ));
    };
    let Some(y) = anchor_coordinate(node_id, &anchor.y, "y")? else {
        return Err(invalid_anchor(
            node_id,
            "path anchor is missing required y coordinate",
        ));
    };
    Point2::new(x, y).map_err(|error| move_handle_geometry_diagnostic(node_id, error))
}

fn required_handle(
    node_id: &str,
    x: &Option<Dimension>,
    y: &Option<Dimension>,
    handle: OpPathHandle,
) -> Result<Point2, Diagnostic> {
    let label = handle_label(handle);
    match optional_handle(node_id, x, y, label)? {
        Some(point) => Ok(point),
        None => Err(invalid_anchor(
            node_id,
            &format!("path anchor {label} handle is required for move_path_handle"),
        )),
    }
}

fn selected_handle_x(anchor: &CorePathAnchor, handle: OpPathHandle) -> &Option<Dimension> {
    match handle {
        OpPathHandle::In => &anchor.in_x,
        OpPathHandle::Out => &anchor.out_x,
    }
}

fn selected_handle_y(anchor: &CorePathAnchor, handle: OpPathHandle) -> &Option<Dimension> {
    match handle {
        OpPathHandle::In => &anchor.in_y,
        OpPathHandle::Out => &anchor.out_y,
    }
}

fn opposite_handle_x(anchor: &CorePathAnchor, handle: OpPathHandle) -> &Option<Dimension> {
    match handle {
        OpPathHandle::In => &anchor.out_x,
        OpPathHandle::Out => &anchor.in_x,
    }
}

fn opposite_handle_y(anchor: &CorePathAnchor, handle: OpPathHandle) -> &Option<Dimension> {
    match handle {
        OpPathHandle::In => &anchor.out_y,
        OpPathHandle::Out => &anchor.in_y,
    }
}

fn handle_label(handle: OpPathHandle) -> &'static str {
    match handle {
        OpPathHandle::In => "in",
        OpPathHandle::Out => "out",
    }
}

fn opposite_handle_label(handle: OpPathHandle) -> &'static str {
    match handle {
        OpPathHandle::In => "out",
        OpPathHandle::Out => "in",
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

fn move_handle_geometry_diagnostic(node_id: &str, error: GeometryError) -> Diagnostic {
    let message = match error {
        GeometryError::NonFinitePoint => "move_path_handle path coordinates must be finite",
        GeometryError::NonFiniteParameter
        | GeometryError::ParameterOutOfRange
        | GeometryError::NonFiniteTolerance
        | GeometryError::NonPositiveTolerance
        | GeometryError::NonPositiveCount
        | GeometryError::CountOutOfRange
        | GeometryError::DegenerateLine
        | GeometryError::InvalidContour
        | GeometryError::NonFiniteTransform
        | GeometryError::SingularTransform => "move_path_handle geometry is invalid",
    };

    Diagnostic::error(
        "tx.invalid_geometry",
        message,
        None,
        Some(node_id.to_owned()),
    )
}
