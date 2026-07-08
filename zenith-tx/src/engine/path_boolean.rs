use zenith_core::{Diagnostic, Document, Node, PathAnchor as CorePathAnchor, PathNode};
use zenith_geometry::{
    ClosedPolyline, ClosedPolylineBooleanOp, GeometryError, Point2,
    reconstruct_contour_boolean_result,
};

use super::{find_node_shared, node_id_of, node_kind_str, px, record_affected, subtree_contains};
use crate::engine::path::{resolved_path_geometry, unknown_node};
use crate::op::OpPathBooleanOperation;

pub(super) struct PathBooleanArgs<'a> {
    pub(super) node_id: &'a str,
    pub(super) target_id: &'a str,
    pub(super) new_id: &'a str,
    pub(super) operation: OpPathBooleanOperation,
    pub(super) tolerance: f64,
}

struct BooleanResultArgs<'a> {
    source_id: &'a str,
    source: &'a PathNode,
    target_id: &'a str,
    target: &'a PathNode,
    operation: OpPathBooleanOperation,
    tolerance: f64,
    new_id: &'a str,
}

pub(super) fn apply_path_boolean(
    args: PathBooleanArgs<'_>,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    let source = match input_path(doc, args.node_id, "path_boolean", diagnostics) {
        Some(path) => path,
        None => return,
    };
    let target = match input_path(doc, args.target_id, "path_boolean", diagnostics) {
        Some(path) => path,
        None => return,
    };
    let result = match boolean_result_path(
        BooleanResultArgs {
            source_id: args.node_id,
            source: &source,
            target_id: args.target_id,
            target: &target,
            operation: args.operation,
            tolerance: args.tolerance,
            new_id: args.new_id,
        },
        diagnostics,
    ) {
        Some(result) => result,
        None => return,
    };

    for page in &mut doc.body.pages {
        if page
            .children
            .iter()
            .any(|node| subtree_contains(node, args.node_id))
        {
            let node_to_insert = Node::Path(result);
            if insert_after_source(&mut page.children, args.node_id, &node_to_insert) {
                record_affected(args.new_id, affected);
            }
            return;
        }
    }
}

fn input_path(
    doc: &Document,
    node_id: &str,
    op_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<PathNode> {
    for page in &doc.body.pages {
        if let Some(node) = find_node_shared(&page.children, node_id) {
            return match node {
                Node::Path(path) => {
                    if !path.subpaths.is_empty() {
                        diagnostics.push(invalid_geometry(
                            node_id,
                            &format!("{op_name} requires direct, non-compound path anchors"),
                        ));
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
                            "path_boolean is not supported on a {} node",
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

fn boolean_result_path(
    args: BooleanResultArgs<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<PathNode> {
    if !args.tolerance.is_finite() || args.tolerance <= 0.0 {
        diagnostics.push(invalid_geometry(
            args.source_id,
            "path_boolean tolerance must be finite and positive",
        ));
        return None;
    }
    if args.source.closed != Some(true) {
        diagnostics.push(invalid_geometry(
            args.source_id,
            "path_boolean source path must be closed",
        ));
        return None;
    }
    if args.target.closed != Some(true) {
        diagnostics.push(invalid_geometry(
            args.target_id,
            "path_boolean target path must be closed",
        ));
        return None;
    }
    if args.source.rotate.is_some() {
        diagnostics.push(invalid_geometry(
            args.source_id,
            "path_boolean source path must not have authored rotation",
        ));
        return None;
    }
    if args.target.rotate.is_some() {
        diagnostics.push(invalid_geometry(
            args.target_id,
            "path_boolean target path must not have authored rotation",
        ));
        return None;
    }

    let source_contour = match closed_contour(args.source_id, args.source, args.tolerance) {
        Ok(contour) => contour,
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            return None;
        }
    };
    let target_contour = match closed_contour(args.target_id, args.target, args.tolerance) {
        Ok(contour) => contour,
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            return None;
        }
    };
    let contours = match reconstruct_contour_boolean_result(
        &source_contour,
        &target_contour,
        boolean_operation(args.operation),
        args.tolerance,
    ) {
        Ok(contours) => contours,
        Err(error) => {
            diagnostics.push(boolean_geometry_diagnostic(args.source_id, error));
            return None;
        }
    };
    let [contour] = contours.as_slice() else {
        diagnostics.push(invalid_geometry(
            args.source_id,
            "path_boolean result must be exactly one non-empty contour",
        ));
        return None;
    };
    if contour.points().is_empty() {
        diagnostics.push(invalid_geometry(
            args.source_id,
            "path_boolean result must be exactly one non-empty contour",
        ));
        return None;
    }

    Some(materialize_path(args.source, args.new_id, contour.points()))
}

fn closed_contour(
    node_id: &str,
    path: &PathNode,
    tolerance: f64,
) -> Result<ClosedPolyline, Diagnostic> {
    let geometry = resolved_path_geometry(node_id, &path.anchors, true)?;
    match ClosedPolyline::from_path(&geometry, tolerance) {
        Ok(Some(contour)) => Ok(contour),
        Ok(None) => Err(invalid_geometry(
            node_id,
            "path_boolean input path must contain a closed contour",
        )),
        Err(error) => Err(boolean_geometry_diagnostic(node_id, error)),
    }
}

fn materialize_path(source: &PathNode, new_id: &str, points: &[Point2]) -> PathNode {
    let mut anchors = Vec::with_capacity(points.len());
    for point in points {
        anchors.push(CorePathAnchor {
            x: Some(px(point.x)),
            y: Some(px(point.y)),
            kind: None,
            in_x: None,
            in_y: None,
            out_x: None,
            out_y: None,
        });
    }

    PathNode {
        id: new_id.to_owned(),
        name: None,
        role: None,
        closed: Some(true),
        fill: source.fill.clone(),
        stroke: source.stroke.clone(),
        stroke_width: source.stroke_width.clone(),
        stroke_alignment: source.stroke_alignment.clone(),
        stroke_linejoin: source.stroke_linejoin.clone(),
        stroke_miter_limit: source.stroke_miter_limit,
        fill_rule: source.fill_rule.clone(),
        opacity: source.opacity,
        visible: source.visible,
        locked: None,
        rotate: None,
        style: source.style.clone(),
        anchors,
        subpaths: Vec::new(),
        source_span: None,
        unknown_props: Default::default(),
    }
}

fn boolean_operation(operation: OpPathBooleanOperation) -> ClosedPolylineBooleanOp {
    match operation {
        OpPathBooleanOperation::Union => ClosedPolylineBooleanOp::Union,
        OpPathBooleanOperation::Intersect => ClosedPolylineBooleanOp::Intersect,
        OpPathBooleanOperation::Subtract => ClosedPolylineBooleanOp::Subtract,
    }
}

fn insert_after_source(children: &mut Vec<Node>, node_id: &str, node_to_insert: &Node) -> bool {
    if let Some(index) = children
        .iter()
        .position(|node| node_id_of(node) == Some(node_id))
    {
        children.insert(index + 1, node_to_insert.clone());
        return true;
    }

    for child in children.iter_mut() {
        match child {
            Node::Frame(frame) => {
                if insert_after_source(&mut frame.children, node_id, node_to_insert) {
                    return true;
                }
            }
            Node::Group(group) => {
                if insert_after_source(&mut group.children, node_id, node_to_insert) {
                    return true;
                }
            }
            Node::Table(table) => {
                for row in &mut table.rows {
                    for cell in &mut row.cells {
                        if insert_after_source(&mut cell.children, node_id, node_to_insert) {
                            return true;
                        }
                    }
                }
            }
            Node::Unknown(unknown) => {
                if insert_after_source(&mut unknown.children, node_id, node_to_insert) {
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

fn boolean_geometry_diagnostic(node_id: &str, error: GeometryError) -> Diagnostic {
    let message = match error {
        GeometryError::NonFinitePoint => "path_boolean anchor coordinates must be finite",
        GeometryError::NonFiniteParameter => "path_boolean parameters must be finite",
        GeometryError::ParameterOutOfRange
        | GeometryError::NonFiniteTolerance
        | GeometryError::NonPositiveTolerance
        | GeometryError::NonPositiveCount
        | GeometryError::CountOutOfRange
        | GeometryError::DegenerateLine
        | GeometryError::InvalidContour
        | GeometryError::NonFiniteTransform
        | GeometryError::SingularTransform => "path_boolean geometry is invalid",
    };
    invalid_geometry(node_id, message)
}

fn invalid_geometry(node_id: &str, message: &str) -> Diagnostic {
    Diagnostic::error(
        "tx.invalid_geometry",
        message,
        None,
        Some(node_id.to_owned()),
    )
}
