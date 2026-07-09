//! Path transaction ops: anchor editing, boolean, snap, symmetry, and contour helpers.
//!
//! Submodules under `path/`:
//! - `apply` / `geometry` / `diagnostics` — set/insert/remove/simplify/transform anchors
//! - `path_anchor` / `path_handle` — move anchor and handle points
//! - `path_boolean` / `path_snap` / `path_symmetry` — higher-level path ops
//! - `path_contour` — contour access helpers shared by move ops

mod apply;
mod diagnostics;
mod geometry;
mod path_anchor;
mod path_boolean;
mod path_contour;
mod path_handle;
mod path_snap;
mod path_symmetry;

pub(crate) use apply::{
    apply_insert_path_anchor, apply_insert_path_anchor_at_point, apply_remove_path_anchor,
    apply_set_path_anchor_kind, apply_set_path_anchors, apply_simplify_path_anchors,
    apply_transform_path_anchors,
};
pub(crate) use diagnostics::{invalid_anchor, reject_compound_path, unknown_node};
pub(crate) use geometry::{
    anchor_coordinate, geometry_anchor_to_core, optional_handle, resolved_path_geometry,
};
pub(crate) use path_anchor::{MovePathAnchorArgs, apply_move_path_anchor};
pub(crate) use path_boolean::{PathBooleanArgs, apply_path_boolean};
pub(crate) use path_handle::{MovePathHandleArgs, apply_move_path_handle};
pub(crate) use path_snap::apply_snap_path_anchors;
pub(crate) use path_symmetry::{MakePathSymmetricArgs, apply_make_path_symmetric};
