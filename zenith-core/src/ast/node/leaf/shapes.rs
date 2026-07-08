//! Axis-aligned leaf shape structs: `image`, `rect`, `line`, `ellipse`.

use std::collections::BTreeMap;

use crate::ast::Span;
use crate::ast::value::{Dimension, PropertyValue};

use crate::ast::node::common::{ObjectPosition, UnknownProperty};

/// An `image` node — a LEAF that draws a raster (PNG) asset into a declared
/// `[x, y, w, h]` box with a `fit` mode, ALWAYS clipped to that box
/// (normative image box-clip).
///
/// The `asset` field references an [`AssetDecl`](crate::ast::AssetDecl) by its
/// stable id, declared in the document's `assets {}` block.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageNode {
    pub id: String,
    pub name: Option<String>,
    pub role: Option<String>,
    /// Required: the referenced asset id (matches an `AssetDecl.id`).
    pub asset: String,
    pub x: Option<PropertyValue>,
    pub y: Option<PropertyValue>,
    pub w: Option<PropertyValue>,
    pub h: Option<PropertyValue>,
    /// Optional source-sub-rectangle: left edge within the source image (pixels).
    /// All four src-* fields must be present together; partial presence is a hard
    /// error (`image.partial_src_rect`). Absent ⇒ the full source image is used.
    pub src_x: Option<Dimension>,
    /// Source-sub-rectangle: top edge within the source image (pixels).
    pub src_y: Option<Dimension>,
    /// Source-sub-rectangle: width within the source image (pixels, must be > 0).
    pub src_w: Option<Dimension>,
    /// Source-sub-rectangle: height within the source image (pixels, must be > 0).
    pub src_h: Option<Dimension>,
    /// Fit mode string (`contain`/`cover`/`stretch`/`none`); validated, not
    /// enum-typed in the AST so unknown values survive for forward-compat.
    pub fit: Option<String>,
    /// SVG-only stroke color override. When the referenced asset is SVG, the
    /// renderer applies this to `currentColor`/root stroke data without
    /// mutating the source asset bytes. Raster assets ignore it.
    pub svg_stroke: Option<PropertyValue>,
    /// SVG-only fill color override. Raster assets ignore it.
    pub svg_fill: Option<PropertyValue>,
    /// SVG-only stroke-width override. Raster assets ignore it.
    pub svg_stroke_width: Option<PropertyValue>,
    /// Clip-to-shape mode (`"ellipse"`/`"rounded"`/`"rect"`); absent or an
    /// unrecognized value means the default rectangular box-clip. Validated as a
    /// plain string so unknown values survive for forward-compat.
    pub clip: Option<String>,
    /// Corner radius for `clip="rounded"`, as a `(token)` dimension ref. Only
    /// meaningful when `clip="rounded"`; absent → radius 0 (sharp corners).
    pub clip_radius: Option<PropertyValue>,
    /// Horizontal object-position anchor (string anchor or `(pct)N`).
    pub object_position_x: Option<ObjectPosition>,
    /// Vertical object-position anchor (string anchor or `(pct)N`).
    pub object_position_y: Option<ObjectPosition>,
    pub opacity: Option<f64>,
    /// Drop shadow / outer glow, as a `(token)` ref to a `shadow` token.
    pub shadow: Option<PropertyValue>,
    /// Color/image filter ops, as a `(token)` ref to a `filter` token.
    pub filter: Option<PropertyValue>,
    /// Spatial coverage mask, as a `(token)` ref to a `mask` token.
    pub mask: Option<PropertyValue>,
    /// Compositing blend mode: `"normal"` (default) or one of the 11 separable
    /// blends. `None`/`"normal"` render source-over (byte-identical).
    pub blend_mode: Option<String>,
    /// Gaussian blur radius applied to the node's own rendered ink (sigma in
    /// the declared unit, resolved to pixels at compile time). `None` / 0 →
    /// no blur (byte-identical to having no attribute).
    pub blur: Option<Dimension>,
    pub visible: Option<bool>,
    pub locked: Option<bool>,
    pub rotate: Option<Dimension>,
    pub style: Option<String>,
    /// Page-relative placement anchor (one of the nine named positions, e.g.
    /// `"bottom-right"`). When present and recognized, the compile step derives
    /// the node's x and/or y from the page and node dimensions. An explicitly-
    /// authored x or y always wins.
    pub anchor: Option<String>,
    /// Optional safe-zone id selecting the reference rectangle for `anchor`
    /// (page-relative when absent). See [`Anchor`](super::super::Anchor).
    pub anchor_zone: Option<String>,
    /// Optional sibling node id for sibling-relative anchor positioning.
    /// See [`RectNode::anchor_sibling`].
    pub anchor_sibling: Option<String>,
    /// Adjacent-placement edge relative to `anchor-sibling`: `above`/`below`/`before`/`after`.
    /// See [`RectNode::anchor_edge`].
    pub anchor_edge: Option<String>,
    /// Gap (px) between this node and its `anchor-sibling` edge when `anchor-edge` is set.
    /// See [`RectNode::anchor_gap`].
    pub anchor_gap: Option<Dimension>,
    /// Parent-relative anchor toggle. See [`RectNode::anchor_parent`].
    pub anchor_parent: Option<bool>,
    /// Source declaration span, when available.
    pub source_span: Option<Span>,
    /// Unknown properties preserved for forward-compat.
    pub unknown_props: BTreeMap<String, UnknownProperty>,
}

/// A `rect` node.
#[derive(Debug, Clone, PartialEq)]
pub struct RectNode {
    pub id: String,
    pub name: Option<String>,
    pub role: Option<String>,
    pub x: Option<PropertyValue>,
    pub y: Option<PropertyValue>,
    pub w: Option<PropertyValue>,
    pub h: Option<PropertyValue>,
    pub radius: Option<PropertyValue>,
    /// Per-corner radius overrides (top-left, top-right, bottom-right, bottom-left).
    /// When `Some`, the value overrides the uniform `radius` for that corner only.
    /// When `None`, the uniform `radius` applies. All four are `None` for existing docs.
    pub radius_tl: Option<PropertyValue>,
    pub radius_tr: Option<PropertyValue>,
    pub radius_br: Option<PropertyValue>,
    pub radius_bl: Option<PropertyValue>,
    pub style: Option<String>,
    pub fill: Option<PropertyValue>,
    pub stroke: Option<PropertyValue>,
    pub stroke_width: Option<PropertyValue>,
    pub stroke_alignment: Option<String>,
    /// Dash segment length in pixels; `None` = solid stroke.
    pub stroke_dash: Option<PropertyValue>,
    /// Gap length in pixels between dashes; defaults to `stroke_dash` when absent.
    pub stroke_gap: Option<PropertyValue>,
    /// Dash end-cap style: `"butt"` (default), `"round"`, or `"square"`.
    pub stroke_linecap: Option<String>,
    /// Per-side border color for the top edge. Token-required (color token).
    /// When `Some`, a `StrokeLine` is emitted along the top edge of the rect.
    pub border_top: Option<PropertyValue>,
    /// Per-side border color for the bottom edge. Token-required (color token).
    pub border_bottom: Option<PropertyValue>,
    /// Per-side border color for the left edge. Token-required (color token).
    pub border_left: Option<PropertyValue>,
    /// Per-side border color for the right edge. Token-required (color token).
    pub border_right: Option<PropertyValue>,
    /// Shared border width for per-side borders. Token-required (dimension).
    /// Falls back to `stroke_width`, then to 1px when absent.
    pub border_width: Option<PropertyValue>,
    /// Outer stroke color: a SECOND stroke painted OUTSIDE the rect geometry.
    /// Token-required (color token). When `Some`, a `StrokeRect` /
    /// `StrokeRoundedRect` is emitted at outset geometry in addition to the
    /// primary stroke. `None` → no outer stroke (byte-identical).
    pub stroke_outer: Option<PropertyValue>,
    /// Outer stroke width for `stroke_outer`. Token-required (dimension).
    /// Defaults to 1px when absent. Only effective when `stroke_outer` is set.
    pub stroke_outer_width: Option<PropertyValue>,
    /// Drop shadow / outer glow, as a `(token)` ref to a `shadow` token.
    pub shadow: Option<PropertyValue>,
    /// Color/image filter ops, as a `(token)` ref to a `filter` token.
    pub filter: Option<PropertyValue>,
    /// Spatial coverage mask, as a `(token)` ref to a `mask` token.
    pub mask: Option<PropertyValue>,
    /// Compositing blend mode: `"normal"` (default) or one of the 11 separable
    /// blends (`multiply`, `screen`, `overlay`, …). `None`/`"normal"` render
    /// source-over (byte-identical to having no blend).
    pub blend_mode: Option<String>,
    /// Gaussian blur radius applied to the node's own rendered ink (sigma in
    /// the declared unit, resolved to pixels at compile time). `None` / 0 →
    /// no blur (byte-identical to having no attribute).
    pub blur: Option<Dimension>,
    pub opacity: Option<f64>,
    pub visible: Option<bool>,
    pub locked: Option<bool>,
    pub rotate: Option<Dimension>,
    /// Page-relative placement anchor (one of the nine named positions, e.g.
    /// `"bottom-right"`). When present and recognized, the compile step derives
    /// the node's x and/or y from the page and node dimensions. An explicitly-
    /// authored x or y always wins.
    pub anchor: Option<String>,
    /// Optional safe-zone reference for the anchor. When `Some(id)` and a
    /// safe-zone with that id is declared on the page, the `anchor` is resolved
    /// relative to that zone's rectangle instead of the full page. Requires
    /// `anchor` to be set; `anchor_zone` without `anchor` has no effect and
    /// triggers an `anchor.zone_without_anchor` warning.
    pub anchor_zone: Option<String>,
    /// Optional sibling node id for sibling-relative anchor positioning.
    /// Requires `anchor` to be set; `anchor_sibling` without `anchor` has no
    /// effect and triggers an `anchor.sibling_without_anchor` warning.
    pub anchor_sibling: Option<String>,
    /// Adjacent-placement edge relative to `anchor-sibling`: `above`/`below`/`before`/`after`.
    /// When `Some`, positions this node's corresponding edge flush to the named
    /// edge of `anchor-sibling`. Requires `anchor-sibling` to be set.
    pub anchor_edge: Option<String>,
    /// Gap (px) between this node and its `anchor-sibling` edge when `anchor-edge` is set.
    /// A positive value pushes the node away from the sibling; negative pulls it closer.
    pub anchor_gap: Option<Dimension>,
    /// Parent-relative anchor toggle. When `Some(true)` AND a recognized
    /// `anchor` is present (and `anchor_zone` is absent), the `anchor` is
    /// resolved relative to this node's DIRECT PARENT CONTAINER's box (a frame
    /// or group) instead of the full page. An explicitly-authored `x`/`y` still
    /// wins. `anchor_zone` takes precedence when both are set. Requires the node
    /// to be inside a frame/group with a usable box; otherwise the validator
    /// emits `anchor.unresolvable_parent`. `anchor_parent` without `anchor`
    /// triggers an `anchor.parent_without_anchor` warning. `None`/`Some(false)`
    /// keeps page/zone-relative behavior (byte-identical).
    pub anchor_parent: Option<bool>,
    /// Source declaration span, when available.
    pub source_span: Option<Span>,
    /// Unknown properties preserved for forward-compat.
    pub unknown_props: BTreeMap<String, UnknownProperty>,
}

/// A `line` node (stroke-only; defined by two endpoints x1/y1/x2/y2).
///
/// Unlike `rect` and `ellipse` there is no bounding box, no fill, no radius,
/// no rotate, and no stroke-alignment — a line is a 1-D geometry whose only
/// visual property is its centered stroke.
#[derive(Debug, Clone, PartialEq)]
pub struct LineNode {
    pub id: String,
    pub name: Option<String>,
    pub role: Option<String>,
    pub x1: Option<Dimension>,
    pub y1: Option<Dimension>,
    pub x2: Option<Dimension>,
    pub y2: Option<Dimension>,
    pub style: Option<String>,
    pub stroke: Option<PropertyValue>,
    pub stroke_width: Option<PropertyValue>,
    /// Dash segment length in pixels; `None` = solid stroke.
    pub stroke_dash: Option<PropertyValue>,
    /// Gap length in pixels between dashes; defaults to `stroke_dash` when absent.
    pub stroke_gap: Option<PropertyValue>,
    /// Dash end-cap style: `"butt"` (default), `"round"`, or `"square"`.
    pub stroke_linecap: Option<String>,
    pub opacity: Option<f64>,
    pub visible: Option<bool>,
    pub locked: Option<bool>,
    /// Source declaration span, when available.
    pub source_span: Option<Span>,
    /// Unknown properties preserved for forward-compat.
    pub unknown_props: BTreeMap<String, UnknownProperty>,
}

/// An `ellipse` node (fill + centered stroke; bounded by x/y/w/h bounding box).
///
/// `stroke-alignment` is not supported for ellipse in v0 — stroke is always
/// centered on the ellipse path. `stroke_alignment` may be added in a later
/// schema version.
#[derive(Debug, Clone, PartialEq)]
pub struct EllipseNode {
    pub id: String,
    pub name: Option<String>,
    pub role: Option<String>,
    pub x: Option<PropertyValue>,
    pub y: Option<PropertyValue>,
    pub w: Option<PropertyValue>,
    pub h: Option<PropertyValue>,
    /// Explicit x-radius override (half-width of the ellipse). When absent, the
    /// ellipse is inscribed in the bounding box (w/2). Backward-compatible: None
    /// leaves all existing ellipses byte-identical.
    pub rx: Option<PropertyValue>,
    /// Explicit y-radius override (half-height of the ellipse). When absent, the
    /// ellipse is inscribed in the bounding box (h/2). Backward-compatible: None
    /// leaves all existing ellipses byte-identical.
    pub ry: Option<PropertyValue>,
    pub style: Option<String>,
    pub fill: Option<PropertyValue>,
    pub stroke: Option<PropertyValue>,
    pub stroke_width: Option<PropertyValue>,
    /// Dash segment length in pixels; `None` = solid stroke.
    pub stroke_dash: Option<PropertyValue>,
    /// Gap length in pixels between dashes; defaults to `stroke_dash` when absent.
    pub stroke_gap: Option<PropertyValue>,
    /// Dash end-cap style: `"butt"` (default), `"round"`, or `"square"`.
    pub stroke_linecap: Option<String>,
    /// Drop shadow / outer glow, as a `(token)` ref to a `shadow` token.
    pub shadow: Option<PropertyValue>,
    /// Color/image filter ops, as a `(token)` ref to a `filter` token.
    pub filter: Option<PropertyValue>,
    /// Spatial coverage mask, as a `(token)` ref to a `mask` token.
    pub mask: Option<PropertyValue>,
    /// Compositing blend mode: `"normal"` (default) or one of the 11 separable
    /// blends. `None`/`"normal"` render source-over (byte-identical).
    pub blend_mode: Option<String>,
    /// Gaussian blur radius applied to the node's own rendered ink (sigma in
    /// the declared unit, resolved to pixels at compile time). `None` / 0 →
    /// no blur (byte-identical to having no attribute).
    pub blur: Option<Dimension>,
    pub opacity: Option<f64>,
    pub visible: Option<bool>,
    pub locked: Option<bool>,
    pub rotate: Option<Dimension>,
    /// Page-relative placement anchor (one of the nine named positions, e.g.
    /// `"bottom-right"`). When present and recognized, the compile step derives
    /// the node's x and/or y from the page and node dimensions. An explicitly-
    /// authored x or y always wins.
    pub anchor: Option<String>,
    /// Optional safe-zone reference for the anchor. See [`RectNode::anchor_zone`].
    pub anchor_zone: Option<String>,
    /// Optional sibling node id for sibling-relative anchor positioning.
    /// See [`RectNode::anchor_sibling`].
    pub anchor_sibling: Option<String>,
    /// Adjacent-placement edge relative to `anchor-sibling`: `above`/`below`/`before`/`after`.
    /// See [`RectNode::anchor_edge`].
    pub anchor_edge: Option<String>,
    /// Gap (px) between this node and its `anchor-sibling` edge when `anchor-edge` is set.
    /// See [`RectNode::anchor_gap`].
    pub anchor_gap: Option<Dimension>,
    /// Parent-relative anchor toggle. See [`RectNode::anchor_parent`].
    pub anchor_parent: Option<bool>,
    /// Source declaration span, when available.
    pub source_span: Option<Span>,
    /// Unknown properties preserved for forward-compat.
    pub unknown_props: BTreeMap<String, UnknownProperty>,
}
