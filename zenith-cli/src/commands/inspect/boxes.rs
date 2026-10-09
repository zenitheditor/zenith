//! Resolved node boxes for `zenith inspect`: the final page-absolute geometry
//! every node renders at, after auto-layout.
//!
//! Each page compiles with the render path's setup (text sources, imports,
//! project fonts, image intrinsic sizes), and the compile records the box it
//! used for every node: layout and anchors resolved, text heights measured,
//! and line / path / connector boxes from their stroked bounds. Master
//! projections and instance content record under their expanded ids.

use std::collections::BTreeMap;
use std::path::Path;

use zenith_core::Document;
use zenith_scene::{CompiledBox, LayoutBox};

use super::document::NodeEntry;

/// A resolved node box in page-absolute px.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct NodeBox {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub w: f64,
    /// Height.
    pub h: f64,
}

/// A node's final geometry: its unrotated box, its rotation, and its visual
/// bounds when they differ from the box.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct BoxInfo {
    /// The unrotated box the compile used.
    #[serde(rename = "box")]
    pub rect: NodeBox,
    /// The node's own rotation in degrees, when set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotate: Option<f64>,
    /// The axis-aligned bounds of what the node paints (rotation, stroke,
    /// and glyph ink applied), only when they differ from `box`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<NodeBox>,
}

/// Tolerance in px under which `bounds` equals `box`.
const SAME_PX: f64 = 1e-6;

impl From<CompiledBox> for BoxInfo {
    fn from(c: CompiledBox) -> Self {
        let rect = NodeBox::from(c.rect);
        let visual = NodeBox::from(c.visual);
        let same = [
            (rect.x, visual.x),
            (rect.y, visual.y),
            (rect.w, visual.w),
            (rect.h, visual.h),
        ]
        .iter()
        .all(|(a, b)| (a - b).abs() <= SAME_PX);
        Self {
            rect,
            rotate: c.rotate,
            bounds: (!same).then_some(visual),
        }
    }
}

impl From<LayoutBox> for NodeBox {
    fn from(b: LayoutBox) -> Self {
        Self {
            x: b.x,
            y: b.y,
            w: b.w,
            h: b.h,
        }
    }
}

/// The final geometry of every compiled node of each page of `doc`, by id,
/// from [`zenith_pipeline::resolved_boxes`] on the native host.
/// `project_dir` locates project fonts, text sources, imports, and image
/// assets, as on render.
pub fn resolved_boxes(
    doc: &Document,
    project_dir: Option<&Path>,
) -> Vec<BTreeMap<String, BoxInfo>> {
    zenith_pipeline::resolved_boxes(crate::native::host(), doc, project_dir)
        .into_iter()
        .map(|page| {
            page.into_iter()
                .map(|(id, b)| (id, BoxInfo::from(b)))
                .collect()
        })
        .collect()
}

/// Set `box` on `entry` and its subtree from `boxes`, removing each used id
/// from `unused`.
pub(super) fn attach_boxes(
    entry: &mut NodeEntry,
    boxes: &BTreeMap<String, BoxInfo>,
    unused: &mut BTreeMap<String, BoxInfo>,
) {
    entry.resolved = boxes.get(&entry.id).copied();
    unused.remove(&entry.id);
    for child in &mut entry.children {
        attach_boxes(child, boxes, unused);
    }
}
