//! Transforms for leaf renderable nodes: rect, ellipse, line, text, code,
//! image, polygon, polyline, path — plus the shared `point`, `anchor`, and
//! `span` children.
//!
//! Submodules: `shapes` (rect/image/ellipse/line), `text` (text/code),
//! `paths` (polygon/polyline/path + point/anchor/subpath), `span` (the shared
//! `span` child).

mod paths;
mod shapes;
mod span;
mod text;

pub(crate) use paths::{PATH_KNOWN_PROPS, POLYGON_KNOWN_PROPS, POLYLINE_KNOWN_PROPS};
pub(crate) use shapes::{
    ELLIPSE_KNOWN_PROPS, IMAGE_KNOWN_PROPS, LINE_KNOWN_PROPS, RECT_KNOWN_PROPS,
};
pub(crate) use text::{CODE_KNOWN_PROPS, TEXT_KNOWN_PROPS};

pub(in crate::parse::transform) use paths::{
    transform_path, transform_polygon, transform_polyline,
};
pub(in crate::parse::transform) use shapes::{
    transform_ellipse, transform_image, transform_line, transform_rect,
};
pub(in crate::parse::transform) use span::transform_span;
pub(in crate::parse::transform) use text::{transform_code, transform_text};
