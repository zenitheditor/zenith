//! Shared helpers for the per-node checks: geometry resolution, bounding-box
//! and AABB computation, role/id extraction, and the anchor/dimension/style-ref
//! validators reused by every per-kind `check_*` function.
//!
//! Submodules: `geometry` (bbox/axis/role/id/rotation), `anchor` (anchor and
//! sibling-anchor validation), `dims` (geometry-dimension validators), `style`
//! (text-span/font/style-ref and the shared rect/pattern visual-prop block).

mod anchor;
mod dims;
mod geometry;
mod style;

pub(in crate::validate::check) use anchor::{
    AnchorParentCtx, AnchorProps, check_anchor, check_sibling_anchors,
};
pub(in crate::validate::check) use dims::{
    TokenEnv, check_dimension_geom, check_optional_dim, pv_to_dim,
};
pub(in crate::validate::check) use geometry::{
    node_bbox, node_id_and_span, node_role, node_rotate_deg, resolve_axis,
};
pub(in crate::validate::check) use style::{
    VisualProps, check_font_alternates, check_font_features, check_spans, check_stroke_join_props,
    check_stroke_linecap_prop, check_style_ref, check_visual_props, is_valid_blend_mode,
};
