//! Diagnostic constructors for path-op failures, plus the compound-path guard.

use zenith_core::Diagnostic;
use zenith_geometry::GeometryError;

pub(crate) fn reject_compound_path(
    node_id: &str,
    op_name: &str,
    path: &zenith_core::PathNode,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if path.subpaths.is_empty() {
        return false;
    }
    diagnostics.push(Diagnostic::error(
        "tx.unsupported_property",
        format!("{op_name} is not supported on compound path '{node_id}'"),
        None,
        Some(node_id.to_owned()),
    ));
    true
}

pub(super) fn geometry_diagnostic(node_id: &str, error: GeometryError) -> Diagnostic {
    match error {
        GeometryError::NonFiniteTolerance => Diagnostic::error(
            "tx.invalid_geometry_tolerance",
            "simplify_path_anchors tolerance must be finite",
            None,
            Some(node_id.to_owned()),
        ),
        GeometryError::NonPositiveTolerance => Diagnostic::error(
            "tx.invalid_geometry_tolerance",
            "simplify_path_anchors tolerance must be positive",
            None,
            Some(node_id.to_owned()),
        ),
        GeometryError::NonFinitePoint => Diagnostic::error(
            "tx.invalid_geometry",
            "path anchor coordinates must be finite",
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
            "path geometry is invalid",
            None,
            Some(node_id.to_owned()),
        ),
    }
}

pub(super) fn transform_geometry_diagnostic(node_id: &str, error: GeometryError) -> Diagnostic {
    let message = match error {
        GeometryError::NonFinitePoint => "transform_path_anchors point coordinates must be finite",
        GeometryError::NonFiniteParameter => "transform_path_anchors parameters must be finite",
        GeometryError::DegenerateLine => {
            "transform_path_anchors reflect line must use two distinct points"
        }
        GeometryError::InvalidContour => "transform_path_anchors contour geometry is invalid",
        GeometryError::NonFiniteTransform => {
            "transform_path_anchors produced a non-finite transform"
        }
        GeometryError::SingularTransform => {
            "transform_path_anchors transform is singular: a scale factor sx or sy is 0"
        }
        GeometryError::ParameterOutOfRange
        | GeometryError::NonFiniteTolerance
        | GeometryError::NonPositiveTolerance
        | GeometryError::NonPositiveCount
        | GeometryError::CountOutOfRange => "transform_path_anchors geometry is invalid",
    };

    Diagnostic::error(
        "tx.invalid_geometry",
        message,
        None,
        Some(node_id.to_owned()),
    )
}

pub(super) fn insert_geometry_diagnostic(node_id: &str, error: GeometryError) -> Diagnostic {
    let message = match error {
        GeometryError::NonFiniteParameter => "insert_path_anchor t must be finite",
        GeometryError::ParameterOutOfRange => "insert_path_anchor t must be between 0 and 1",
        GeometryError::CountOutOfRange => {
            "insert_path_anchor segment_index is outside the path segment range"
        }
        GeometryError::NonFinitePoint => "insert_path_anchor path coordinates must be finite",
        GeometryError::NonFiniteTolerance
        | GeometryError::NonPositiveTolerance
        | GeometryError::NonPositiveCount
        | GeometryError::DegenerateLine
        | GeometryError::InvalidContour
        | GeometryError::NonFiniteTransform
        | GeometryError::SingularTransform => "insert_path_anchor geometry is invalid",
    };

    Diagnostic::error(
        "tx.invalid_geometry",
        message,
        None,
        Some(node_id.to_owned()),
    )
}

pub(super) fn insert_at_point_geometry_diagnostic(
    node_id: &str,
    error: GeometryError,
) -> Diagnostic {
    match error {
        GeometryError::NonFiniteTolerance => Diagnostic::error(
            "tx.invalid_geometry_tolerance",
            "insert_path_anchor_at_point tolerance must be finite",
            None,
            Some(node_id.to_owned()),
        ),
        GeometryError::NonPositiveTolerance => Diagnostic::error(
            "tx.invalid_geometry_tolerance",
            "insert_path_anchor_at_point tolerance must be positive",
            None,
            Some(node_id.to_owned()),
        ),
        GeometryError::NonFinitePoint => Diagnostic::error(
            "tx.invalid_geometry",
            "insert_path_anchor_at_point point coordinates must be finite",
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
            "insert_path_anchor_at_point geometry is invalid",
            None,
            Some(node_id.to_owned()),
        ),
    }
}

pub(crate) fn invalid_anchor(node_id: &str, message: &str) -> Diagnostic {
    Diagnostic::error(
        "tx.invalid_path_anchor",
        message,
        None,
        Some(node_id.to_owned()),
    )
}

pub(crate) fn unknown_node(node_id: &str) -> Diagnostic {
    Diagnostic::error(
        "tx.unknown_node",
        format!("node {:?} not found in document", node_id),
        None,
        Some(node_id.to_owned()),
    )
}
