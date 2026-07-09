//! Shared contour addressing for path transaction ops.

use zenith_core::{Diagnostic, PathAnchor as CorePathAnchor, PathNode};

pub(crate) struct PathContourMut<'a> {
    pub(crate) anchors: &'a mut Vec<CorePathAnchor>,
    pub(crate) closed: bool,
}

pub(crate) fn path_contour_mut<'a>(
    node_id: &str,
    op_name: &str,
    path: &'a mut PathNode,
    subpath_index: Option<usize>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<PathContourMut<'a>> {
    match subpath_index {
        None if path.subpaths.is_empty() => Some(PathContourMut {
            anchors: &mut path.anchors,
            closed: path.closed == Some(true),
        }),
        None => {
            diagnostics.push(Diagnostic::error(
                "tx.unsupported_property",
                format!("{op_name} requires subpath_index on compound path '{node_id}'"),
                None,
                Some(node_id.to_owned()),
            ));
            None
        }
        Some(0) if path.subpaths.is_empty() => Some(PathContourMut {
            anchors: &mut path.anchors,
            closed: path.closed == Some(true),
        }),
        Some(index) if path.subpaths.is_empty() => {
            diagnostics.push(subpath_out_of_range(node_id, index));
            None
        }
        Some(index) => {
            let Some(subpath) = path.subpaths.get_mut(index) else {
                diagnostics.push(subpath_out_of_range(node_id, index));
                return None;
            };
            Some(PathContourMut {
                anchors: &mut subpath.anchors,
                closed: subpath.closed == Some(true),
            })
        }
    }
}

fn subpath_out_of_range(node_id: &str, subpath_index: usize) -> Diagnostic {
    Diagnostic::error(
        "tx.out_of_range",
        format!("subpath_index {subpath_index} is out of range for path '{node_id}'"),
        None,
        Some(node_id.to_owned()),
    )
}
