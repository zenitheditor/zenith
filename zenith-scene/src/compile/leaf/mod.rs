//! Vector leaf-node compilation: rect, ellipse, line, polygon, polyline, path,
//! shape, and connector.
//!
//! Each `compile_*` entry mirrors the shared leaf signature and emits the same
//! `SceneCommand` stream that the original inline `compile_node` arms produced.
//! This module root is wiring only — the per-concern logic lives in the
//! submodules and is re-exported here so the dispatcher's `use` paths resolve
//! unchanged.

mod box_paint;
mod common;
mod connector;
mod poly;
mod rect_ellipse;
mod routing;
mod shape;

pub(in crate::compile) use box_paint::{
    BoxGeom, BoxPaintEnv, BoxStroke, push_box_fill, push_box_stroke,
};
pub(in crate::compile) use common::resolve_dash_params;
pub(super) use connector::{ConnectorEnv, compile_connector};
pub(super) use poly::{compile_line, compile_path, compile_polygon, compile_polyline};
pub(in crate::compile) use poly::{path_outline_bounds, path_to_compound_geometry};
pub(super) use rect_ellipse::{RectEllipseEnv, compile_ellipse, compile_rect};
pub(super) use shape::{ShapeCompileEnv, compile_shape};
