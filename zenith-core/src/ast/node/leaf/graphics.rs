//! Procedural graphics leaf structs: `pattern` and `chart` (plus `ChartSeries`).

use std::collections::BTreeMap;

use crate::ast::Span;
use crate::ast::value::{Dimension, PropertyValue};

use crate::ast::node::common::{Node, UnknownProperty};

/// A `pattern` node — a compact procedural primitive.
///
/// A `pattern` carries one TEMPLATE child — the [`motif`](PatternNode::motif) —
/// a single [`Node`] that will be expanded deterministically into many native
/// shapes (a grid or scatter of the motif). The node currently renders nothing;
/// expansion is not yet implemented. The motif is NOT an addressable/rendered
/// node — id-collection, validation, anchor, and tx passes treat the pattern as
/// a LEAF and never descend into the motif.
///
/// The common visual/geometry fields mirror [`RectNode`](crate::ast::node::RectNode); the pattern-specific
/// fields (`kind`, `seed`, `count`, `spacing`, `jitter`) describe the expansion.
#[derive(Debug, Clone, PartialEq)]
pub struct PatternNode {
    pub id: String,
    pub name: Option<String>,
    pub role: Option<String>,
    pub x: Option<PropertyValue>,
    pub y: Option<PropertyValue>,
    pub w: Option<PropertyValue>,
    pub h: Option<PropertyValue>,
    pub radius: Option<PropertyValue>,
    /// Per-corner radius overrides (top-left, top-right, bottom-right, bottom-left).
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
    pub border_top: Option<PropertyValue>,
    /// Per-side border color for the bottom edge. Token-required (color token).
    pub border_bottom: Option<PropertyValue>,
    /// Per-side border color for the left edge. Token-required (color token).
    pub border_left: Option<PropertyValue>,
    /// Per-side border color for the right edge. Token-required (color token).
    pub border_right: Option<PropertyValue>,
    /// Shared border width for per-side borders. Token-required (dimension).
    pub border_width: Option<PropertyValue>,
    /// Outer stroke color: a SECOND stroke painted OUTSIDE the geometry.
    pub stroke_outer: Option<PropertyValue>,
    /// Outer stroke width for `stroke_outer`. Token-required (dimension).
    pub stroke_outer_width: Option<PropertyValue>,
    /// Drop shadow / outer glow, as a `(token)` ref to a `shadow` token.
    pub shadow: Option<PropertyValue>,
    /// Color/image filter ops, as a `(token)` ref to a `filter` token.
    pub filter: Option<PropertyValue>,
    /// Spatial coverage mask, as a `(token)` ref to a `mask` token.
    pub mask: Option<PropertyValue>,
    /// Compositing blend mode: `"normal"` (default) or one of the separable blends.
    pub blend_mode: Option<String>,
    /// Gaussian blur radius applied to the node's own rendered ink.
    pub blur: Option<Dimension>,
    pub opacity: Option<f64>,
    pub visible: Option<bool>,
    pub locked: Option<bool>,
    pub rotate: Option<Dimension>,
    /// Page-relative placement anchor. See [`RectNode::anchor`](crate::ast::node::RectNode::anchor).
    pub anchor: Option<String>,
    /// Optional safe-zone reference for the anchor. See [`RectNode::anchor_zone`](crate::ast::node::RectNode::anchor_zone).
    pub anchor_zone: Option<String>,
    /// Optional sibling node id for sibling-relative anchor positioning.
    pub anchor_sibling: Option<String>,
    /// Adjacent-placement edge relative to `anchor-sibling`: `above`/`below`/`before`/`after`.
    /// See [`RectNode::anchor_edge`](crate::ast::node::RectNode::anchor_edge).
    pub anchor_edge: Option<String>,
    /// Gap (px) between this node and its `anchor-sibling` edge when `anchor-edge` is set.
    /// See [`RectNode::anchor_gap`](crate::ast::node::RectNode::anchor_gap).
    pub anchor_gap: Option<Dimension>,
    /// Parent-relative anchor toggle. See [`RectNode::anchor_parent`](crate::ast::node::RectNode::anchor_parent).
    pub anchor_parent: Option<bool>,
    /// Required: the pattern kind (`"grid"` | `"scatter"`; freeform, validated later).
    pub kind: String,
    /// Deterministic jitter seed.
    pub seed: Option<i64>,
    /// Scatter: number of instances.
    pub count: Option<i64>,
    /// Grid: cell spacing.
    pub spacing: Option<Dimension>,
    /// Positional jitter amount in `0..1`.
    pub jitter: Option<f64>,
    /// The single template child shape expanded by the pattern (mandatory).
    /// This is a TEMPLATE, NOT an addressable/rendered node: id-collection,
    /// validation, anchor, and tx passes never descend into it.
    pub motif: Box<Node>,
    /// Source declaration span, when available.
    pub source_span: Option<Span>,
    /// Unknown properties preserved for forward-compat.
    pub unknown_props: BTreeMap<String, UnknownProperty>,
}

/// One data series within a [`ChartNode`].
///
/// A series is PURE DATA — it is not a renderable [`Node`] and is never
/// descended into by id-collection, validation, anchor, or tx passes. It
/// carries an ordered list of numeric values and optional legend/styling hints.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartSeries {
    /// Optional legend or category label for this series.
    pub label: Option<String>,
    /// Optional series color; a `(token)` color ref. When absent the renderer
    /// picks a palette color by series index.
    pub color: Option<PropertyValue>,
    /// Per-series value-label color override; falls back to the chart
    /// `value_color` then the default on-fill contrasting color.
    pub label_color: Option<PropertyValue>,
    /// Optional binding to a whole series from a [`DataContext`](crate::data::DataContext) field.
    /// `None` means the values are inline in [`ChartSeries::values`].
    pub data_ref: Option<String>,
    /// Ordered numeric data points for this series.
    pub values: Vec<f64>,
}

/// A `chart` node — a compact data-visualization primitive.
///
/// A `chart` declares its data inline via [`series`](ChartNode::series) children
/// (one child KDL node per series, each with positional f64 arguments) and
/// paints into its `[x, y, w, h]` bounding box. The node currently renders
/// nothing; chart rendering is deferred. The series children are pure DATA,
/// not renderable nodes: id-collection, validation, anchor, and tx passes
/// treat the chart as a LEAF and never descend into them.
///
/// The common visual/geometry fields mirror [`PatternNode`]; the chart-specific
/// fields (`kind`, `title`, `caption`, `legend`, `axis_*`, `bar_mode`,
/// `orientation`, `point_placement`, `value_labels`, `value_color`, `label_colors`,
/// `slice_colors`, `categories`, `series`, `legend_position`, `legend_layout`,
/// `legend_align`) describe the chart content.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartNode {
    pub id: String,
    pub name: Option<String>,
    pub role: Option<String>,
    pub x: Option<PropertyValue>,
    pub y: Option<PropertyValue>,
    pub w: Option<PropertyValue>,
    pub h: Option<PropertyValue>,
    pub radius: Option<PropertyValue>,
    /// Per-corner radius overrides (top-left, top-right, bottom-right, bottom-left).
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
    pub border_top: Option<PropertyValue>,
    /// Per-side border color for the bottom edge. Token-required (color token).
    pub border_bottom: Option<PropertyValue>,
    /// Per-side border color for the left edge. Token-required (color token).
    pub border_left: Option<PropertyValue>,
    /// Per-side border color for the right edge. Token-required (color token).
    pub border_right: Option<PropertyValue>,
    /// Shared border width for per-side borders. Token-required (dimension).
    pub border_width: Option<PropertyValue>,
    /// Outer stroke color: a SECOND stroke painted OUTSIDE the geometry.
    pub stroke_outer: Option<PropertyValue>,
    /// Outer stroke width for `stroke_outer`. Token-required (dimension).
    pub stroke_outer_width: Option<PropertyValue>,
    /// Drop shadow / outer glow, as a `(token)` ref to a `shadow` token.
    pub shadow: Option<PropertyValue>,
    /// Color/image filter ops, as a `(token)` ref to a `filter` token.
    pub filter: Option<PropertyValue>,
    /// Spatial coverage mask, as a `(token)` ref to a `mask` token.
    pub mask: Option<PropertyValue>,
    /// Compositing blend mode: `"normal"` (default) or one of the separable blends.
    pub blend_mode: Option<String>,
    /// Gaussian blur radius applied to the node's own rendered ink.
    pub blur: Option<Dimension>,
    pub opacity: Option<f64>,
    pub visible: Option<bool>,
    pub locked: Option<bool>,
    pub rotate: Option<Dimension>,
    /// Page-relative placement anchor. See [`RectNode::anchor`](crate::ast::node::RectNode::anchor).
    pub anchor: Option<String>,
    /// Optional safe-zone reference for the anchor. See [`RectNode::anchor_zone`](crate::ast::node::RectNode::anchor_zone).
    pub anchor_zone: Option<String>,
    /// Optional sibling node id for sibling-relative anchor positioning.
    pub anchor_sibling: Option<String>,
    /// Adjacent-placement edge relative to `anchor-sibling`: `above`/`below`/`before`/`after`.
    /// See [`RectNode::anchor_edge`](crate::ast::node::RectNode::anchor_edge).
    pub anchor_edge: Option<String>,
    /// Gap (px) between this node and its `anchor-sibling` edge when `anchor-edge` is set.
    /// See [`RectNode::anchor_gap`](crate::ast::node::RectNode::anchor_gap).
    pub anchor_gap: Option<Dimension>,
    /// Parent-relative anchor toggle. See [`RectNode::anchor_parent`](crate::ast::node::RectNode::anchor_parent).
    pub anchor_parent: Option<bool>,
    /// Required: the chart kind (`"bar"` | `"line"` | `"sparkline"` | `"pie"` | `"donut"`;
    /// freeform, validated later).
    pub kind: String,
    /// Optional chart title rendered above the plot area.
    pub title: Option<String>,
    /// Optional caption rendered below the chart.
    pub caption: Option<String>,
    /// Whether to render a legend. `None` defers to the renderer default.
    pub legend: Option<bool>,
    /// Legend placement: `"right"` (default) | `"left"` | `"top"` | `"bottom"`.
    /// freeform, validated later.
    pub legend_position: Option<String>,
    /// Legend layout for top/bottom placement: `"wrapped"` (default; horizontal
    /// flow) | `"list"` (vertical stack). Ignored for left/right (always a
    /// vertical list). freeform, validated later.
    pub legend_layout: Option<String>,
    /// Legend alignment for top/bottom placement: `"center"` (default) | `"left"`
    /// | `"right"`. freeform, validated later.
    pub legend_align: Option<String>,
    /// Minimum value for the value axis. `None` = auto-fit to data.
    pub axis_min: Option<f64>,
    /// Maximum value for the value axis. `None` = auto-fit to data.
    pub axis_max: Option<f64>,
    /// Style string for the axis (e.g. `"hidden"`, `"minimal"`); freeform for now.
    pub axis_style: Option<String>,
    /// Bar layout mode: `"grouped"` (default) | `"stacked"`; freeform,
    /// validated later. Mirrors how `kind` is typed/documented.
    pub bar_mode: Option<String>,
    /// Bar orientation: `"vertical"` (default; bars grow up from the X axis) |
    /// `"horizontal"` (bars grow right from the Y axis, categories on the Y
    /// axis). Applies to bar charts; freeform, validated later.
    pub orientation: Option<String>,
    /// X placement for line/area points: `"edge"` (default; first point on the
    /// value axis, last at the right edge) | `"center"` (category-band centers).
    /// freeform, validated later.
    pub point_placement: Option<String>,
    /// Value-label display/placement: `"auto"` (default) | `"none"` | `"top"` |
    /// `"center"`. freeform, validated later.
    pub value_labels: Option<String>,
    /// Explicit color (token) for value labels; when absent the renderer
    /// auto-picks a contrasting color.
    pub value_color: Option<PropertyValue>,
    /// Per-slice value-label colors for pie/donut (one per category, in order);
    /// empty = use the chart `value_color` or the white on-fill default.
    /// Populated from a `label-colors` child node whose positional arguments
    /// are each a `PropertyValue` (e.g. `(token)"color.x"`).
    pub label_colors: Vec<PropertyValue>,
    /// Per-slice FILL colors for pie/donut (one per category, in order);
    /// empty = fall back to the palette (`slice_color(idx)`).
    /// Populated from a `slice-colors` child node whose positional arguments
    /// are each a `PropertyValue` (e.g. `(token)"color.x"`).
    pub slice_colors: Vec<PropertyValue>,
    /// X-axis category labels (one per category slot); empty = derive index
    /// labels at render. Populated from a `categories` child node whose
    /// positional arguments are the label strings.
    pub categories: Vec<String>,
    /// Ordered data series. Each series carries labels, an optional color, and
    /// a list of f64 data points.
    pub series: Vec<ChartSeries>,
    /// Source declaration span, when available.
    pub source_span: Option<Span>,
    /// Unknown properties preserved for forward-compat.
    pub unknown_props: BTreeMap<String, UnknownProperty>,
}
