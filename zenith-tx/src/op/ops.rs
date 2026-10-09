//! The [`Op`] enum: every mutating operation a [`super::Transaction`] can carry.

use super::anchor::AnchorEdit;
use super::layout::{LayoutEdit, SizeInput};
use super::types::{
    AddAssetMetadata, FilterOpInput, GradientStopInput, OpPathAnchor, OpPathBooleanOperation,
    OpPathHandle, OpPathSubpath, OpPathTransform, OpPoint, OpSpan, Position, ShadowLayerInput,
};

/// A single operation within a [`super::Transaction`].
///
/// The `op` field in JSON is the snake_case tag, e.g. `"set_text_align"`.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    /// Set the `align` property on a text node.
    SetTextAlign {
        /// The stable node `id` to target.
        node: String,
        /// The new alignment value.
        align: String,
    },
    /// Move a node one sibling position toward the end (front/top of z-order).
    MoveForward {
        /// The stable node `id` to target.
        node: String,
    },
    /// Move a node one sibling position toward the beginning (back/bottom of z-order).
    MoveBackward {
        /// The stable node `id` to target.
        node: String,
    },
    /// Move a node to the topmost position (last child) in its parent's children.
    MoveToFront {
        /// The stable node `id` to target.
        node: String,
    },
    /// Move a node to the bottommost position (first child) in its parent's children.
    MoveToBack {
        /// The stable node `id` to target.
        node: String,
    },
    /// Set the `fill` property on a node that supports fill.
    SetFill {
        /// The stable node `id` to target.
        node: String,
        /// Token id to set as the fill (e.g. `"color.brand"`).
        fill: String,
    },
    /// Set the authored `fill-rule` property on a vector node that supports it.
    SetFillRule {
        /// The stable node `id` to target.
        node: String,
        /// Fill winding rule to store in the authored `fill-rule` field.
        fill_rule: String,
    },
    /// Set the `stroke` (outline color) property on a node that supports stroke.
    SetStroke {
        /// The stable node `id` to target.
        node: String,
        /// Token id to set as the stroke color (e.g. `"color.rule"`).
        stroke: String,
    },
    /// Set the `stroke-width` property on a node that supports stroke.
    SetStrokeWidth {
        /// The stable node `id` to target.
        node: String,
        /// Dimension token id to set as the stroke width (e.g. `"size.stroke"`).
        stroke_width: String,
    },
    /// Bind one token-valued property of a node to a token, or remove it:
    /// `radius` (rect, frame, shape, pattern, chart), `font-family` and
    /// `font-size` (text, code, field, footnote, toc), `font-weight` (text,
    /// code). Another property, or a kind without it, is
    /// `tx.unsupported_property`.
    SetNodeToken {
        /// The stable node `id` to target.
        node: String,
        /// The property in its KDL spelling (`font-size`); underscore
        /// spellings (`font_size`) are accepted.
        property: String,
        /// The token id to bind. `null` or omitted removes the attribute, so
        /// the node takes the value from its style or the defaults.
        #[serde(default)]
        token: Option<String>,
    },
    /// Replace the text of one span of a `text` or `shape` node. The span
    /// keeps every attribute of its own (fill, weight, link, …).
    SetSpanText {
        /// The stable node `id` to target.
        node: String,
        /// The 0-based span index.
        span: usize,
        /// The new span text.
        text: String,
    },
    /// Show or hide a node by setting its `visible` property.
    SetVisible {
        /// The stable node `id` to target.
        node: String,
        /// `false` hides the node; `true` makes it visible.
        visible: bool,
    },
    /// Lock or unlock a node by setting its `locked` property.
    SetLocked {
        /// The stable node `id` to target.
        node: String,
        /// `true` locks the node; `false` unlocks it.
        locked: bool,
    },
    /// Move and/or resize a bbox node by updating its `x`, `y`, `w`, `h`
    /// and `rotate`.
    ///
    /// Every field is tri-state: omit it to leave the attribute unchanged,
    /// pass `null` to remove the attribute, pass a value to set it. Removing
    /// `x` / `y` / `w` / `h` is rejected with `tx.geometry_required` when the
    /// node then has no placement: no row/column/grid frame places it in flow,
    /// and (for `x` / `y`) no anchor places it.
    ///
    /// Every written value is px. It replaces a token ref, a `(pt)` value,
    /// or a `hug` / `fill` keyword on that axis, and an `x` / `y` overrides
    /// an anchor on that axis. This is the explicit absolute write. Use
    /// `nudge_geometry` to move by a delta and keep tokens and units.
    SetGeometry {
        /// The stable node `id` to target.
        node: String,
        /// New left edge in pixels. Omit to leave unchanged, `null` to remove.
        #[serde(
            default,
            deserialize_with = "super::layout::nullable",
            skip_serializing_if = "Option::is_none"
        )]
        x: Option<Option<f64>>,
        /// New top edge in pixels. Omit to leave unchanged, `null` to remove.
        #[serde(
            default,
            deserialize_with = "super::layout::nullable",
            skip_serializing_if = "Option::is_none"
        )]
        y: Option<Option<f64>>,
        /// New width: pixels, or `"hug"` / `"fill"` inside a row/column frame.
        /// Omit to leave unchanged, `null` to remove (px size and keyword).
        #[serde(
            default,
            deserialize_with = "super::layout::nullable",
            skip_serializing_if = "Option::is_none"
        )]
        w: Option<Option<SizeInput>>,
        /// New height: pixels, or `"hug"` / `"fill"` inside a row/column frame.
        /// Omit to leave unchanged, `null` to remove (px size and keyword).
        #[serde(
            default,
            deserialize_with = "super::layout::nullable",
            skip_serializing_if = "Option::is_none"
        )]
        h: Option<Option<SizeInput>>,
        /// New rotation in degrees. Omit to leave unchanged, `null` to remove.
        #[serde(
            default,
            deserialize_with = "super::layout::nullable",
            skip_serializing_if = "Option::is_none"
        )]
        rotate: Option<Option<f64>>,
    },
    /// Move and/or resize a box node by px deltas, keeping each attribute's
    /// unit.
    ///
    /// Deltas are in the node's authored space: the space its `x` / `y` are
    /// written in, before its own `rotate`. A `(pt)` value stays `(pt)`. An
    /// absent `x` / `y` on a `group` or `instance` counts as `(px)0`, as the
    /// scene places it. An omitted delta leaves that attribute untouched.
    ///
    /// Rejected axes, with no change to the node:
    /// - a token-bound axis: `tx.token_bound`, unless `detach` is `true`.
    /// - an absent `w` / `h` or a `hug` / `fill` size: `tx.computed_size`.
    /// - an `x` / `y` an anchor supplies: `tx.anchored`.
    /// - a value with no px conversion (`pct`, `deg`, literal, data ref):
    ///   `tx.value_unresolved`.
    /// - a `w` / `h` that would become negative: `tx.invalid_geometry`.
    /// - `dx` / `dy` on an in-flow child of a row/column/grid frame:
    ///   `tx.layout_managed`.
    NudgeGeometry {
        /// The stable node `id` to target.
        node: String,
        /// Px delta added to `x`. Must be finite.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dx: Option<f64>,
        /// Px delta added to `y`. Must be finite.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dy: Option<f64>,
        /// Px delta added to `w`. Must be finite.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dw: Option<f64>,
        /// Px delta added to `h`. Must be finite.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dh: Option<f64>,
        /// `true` replaces a token-bound axis with a px literal: the
        /// resolved token value plus the delta. Default `false`.
        #[serde(default)]
        detach: bool,
    },
    /// Set or clear the anchor placement attributes of a node.
    ///
    /// Removing the anchor from a node with no `x` / `y` is rejected with
    /// `tx.geometry_required`: run `detach_anchor` to keep its position.
    SetAnchor(AnchorEdit),
    /// Move an edge-anchored node along its anchor axis by changing its
    /// `anchor-gap`, in the gap's unit.
    ///
    /// The node needs `anchor-sibling` and `anchor-edge`. `dx` / `dy` are the
    /// drag delta in the node's authored space. Only the edge axis counts:
    /// `below` adds `dy`, `above` subtracts `dy`, `after` adds `dx`, `before`
    /// subtracts `dx`. A non-zero delta on the other axis is rejected.
    NudgeAnchorGap {
        /// The stable node `id` to target.
        node: String,
        /// Px drag delta along x. Must be finite.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dx: Option<f64>,
        /// Px drag delta along y. Must be finite.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dy: Option<f64>,
    },
    /// Remove every anchor attribute from a node and keep its position.
    ///
    /// Each `x` / `y` the anchor supplied is written as px at the
    /// anchor-derived position. Nodes anchored to this node keep their
    /// position. When the anchor position does not derive from authored
    /// values, the op is rejected with `tx.anchored`.
    DetachAnchor {
        /// The stable node `id` to target.
        node: String,
    },
    /// Move the endpoints of a `line` node by px deltas, keeping each
    /// endpoint's unit.
    ///
    /// `dx1` / `dy1` move the start point, `dx2` / `dy2` the end point. Move
    /// the whole line with all four. A connector is rejected with
    /// `tx.derived_geometry`: its endpoints come from its targets.
    NudgeLinePoints {
        /// The stable node `id` to target.
        node: String,
        /// Px delta added to `x1`. Must be finite.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dx1: Option<f64>,
        /// Px delta added to `y1`. Must be finite.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dy1: Option<f64>,
        /// Px delta added to `x2`. Must be finite.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dx2: Option<f64>,
        /// Px delta added to `y2`. Must be finite.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dy2: Option<f64>,
    },
    /// Replace the entire vertex list of a `polygon` or `polyline` node.
    SetPoints {
        /// The stable node `id` to target.
        node: String,
        /// Replacement vertex list. Each vertex is in document pixels.
        points: Vec<OpPoint>,
    },
    /// Replace the entire anchor list of a `path` node.
    SetPathAnchors {
        /// The stable node `id` to target.
        node: String,
        /// Optional zero-based subpath index for compound paths.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subpath_index: Option<usize>,
        /// Replacement anchor list. Each coordinate is in document pixels.
        anchors: Vec<OpPathAnchor>,
    },
    /// Set or clear the authoring intent metadata on one anchor of a `path` node.
    SetPathAnchorKind {
        /// The stable node `id` to target.
        node: String,
        /// Optional zero-based subpath index for compound paths.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subpath_index: Option<usize>,
        /// Zero-based anchor index to update.
        anchor_index: usize,
        /// Optional authoring intent. `None`/`null` clears it.
        #[serde(default)]
        kind: Option<String>,
    },
    /// Remove one anchor from a `path` node by index.
    RemovePathAnchor {
        /// The stable node `id` to target.
        node: String,
        /// Optional zero-based subpath index for compound paths.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subpath_index: Option<usize>,
        /// Zero-based anchor index to remove.
        anchor_index: usize,
    },
    /// Move one `path` anchor and its complete handles by a pixel delta.
    MovePathAnchor {
        /// The stable node `id` to target.
        node: String,
        /// Optional zero-based subpath index for compound paths.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subpath_index: Option<usize>,
        /// Zero-based anchor index to move.
        anchor_index: usize,
        /// X-axis translation in document pixels. Must be finite.
        dx: f64,
        /// Y-axis translation in document pixels. Must be finite.
        dy: f64,
    },
    /// Move one complete handle on a `path` anchor by a pixel delta.
    MovePathHandle {
        /// The stable node `id` to target.
        node: String,
        /// Optional zero-based subpath index for compound paths.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subpath_index: Option<usize>,
        /// Zero-based anchor index whose handle should move.
        anchor_index: usize,
        /// Which handle on the anchor to move.
        handle: OpPathHandle,
        /// X-axis translation in document pixels. Must be finite.
        dx: f64,
        /// Y-axis translation in document pixels. Must be finite.
        dy: f64,
    },
    /// Insert an anchor into a `path` node by splitting an existing segment.
    InsertPathAnchor {
        /// The stable node `id` to target.
        node: String,
        /// Optional zero-based subpath index for compound paths.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subpath_index: Option<usize>,
        /// Zero-based segment index to split.
        segment_index: usize,
        /// Normalized position along the segment. Must be finite and in the range 0..=1.
        t: f64,
    },
    /// Insert an anchor into a `path` node at the nearest point on the path.
    InsertPathAnchorAtPoint {
        /// The stable node `id` to target.
        node: String,
        /// Query point X coordinate in document pixels. Must be finite.
        x: f64,
        /// Query point Y coordinate in document pixels. Must be finite.
        y: f64,
        /// Maximum accepted projection distance in pixels. Must be finite and positive.
        tolerance: f64,
    },
    /// Simplify an open `path` node's anchors using a pixel tolerance.
    SimplifyPathAnchors {
        /// The stable node `id` to target.
        node: String,
        /// Optional zero-based subpath index for compound paths.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subpath_index: Option<usize>,
        /// Maximum perpendicular deviation in pixels. Must be finite and positive.
        tolerance: f64,
    },
    /// Apply an affine transform to every editable anchor and complete handle point of a `path` node.
    TransformPathAnchors {
        /// The stable node `id` to target.
        node: String,
        /// Transform mode and scalar parameters.
        transform: OpPathTransform,
    },
    /// Translate one `path` node so its nearest boundary point lands on another path.
    /// The target path counts in its page position, so the two paths can sit
    /// in different containers.
    SnapPathAnchors {
        /// The stable source path `id` to translate.
        node: String,
        /// The stable target path `id` to snap onto.
        target: String,
        /// Maximum accepted nearest-boundary distance in pixels.
        tolerance: f64,
    },
    /// Materialize radial symmetry copies of one `path` as editable sibling path nodes.
    MakePathSymmetric {
        /// Stable source path `id`.
        node: String,
        /// Prefix used to form generated ids; copy ids are `id_prefix + index`.
        id_prefix: String,
        /// Total radial positions including the unchanged source path.
        count: usize,
        /// Symmetry center X coordinate in pixels.
        cx: f64,
        /// Symmetry center Y coordinate in pixels.
        cy: f64,
        /// Optional starting angle in degrees for generated transform index 0.
        /// In `mirror` mode this is the angle of the primary reflection axis.
        #[serde(default)]
        start_angle_degrees: f64,
        /// When `true`, bake a dihedral (mirror) symmetry — `count` mirror axes
        /// producing `2·count` reflected/rotated copies — instead of the default
        /// radial rotation of `count` copies.
        #[serde(default)]
        mirror: bool,
    },
    /// Materialize a boolean result between two simple closed `path` nodes as a new sibling path.
    /// The target path counts in its page position, and the result lands in
    /// the source path's container.
    PathBoolean {
        /// Stable source path id. The result inherits render-relevant style from this path.
        node: String,
        /// Stable target path id.
        target: String,
        /// Id assigned to the newly materialized sibling path.
        new_id: String,
        /// Boolean operation to apply.
        operation: OpPathBooleanOperation,
        /// Flattening and contour classification tolerance in pixels.
        tolerance: f64,
    },
    /// Construct a new node from a `.zen` source fragment and insert it into a
    AddNode {
        /// Stable id of the container to insert into: a page id, master id, or
        /// a group/frame id.
        parent: String,
        /// Where among the container's children to insert. Defaults to `last`.
        #[serde(default)]
        position: Position,
        /// A single `.zen` node fragment to construct and insert.
        source: String,
    },
    /// Construct a typed path node and insert it into a container.
    AddPath {
        /// Stable id of the container to insert into: a page id, master id, or
        /// a group/frame id.
        parent: String,
        /// Stable id assigned to the new path node.
        id: String,
        /// Where among the container's children to insert. Defaults to `last`.
        #[serde(default)]
        position: Position,
        /// Direct-path closure. Invalid for compound paths.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        closed: Option<bool>,
        /// Direct path anchors. Must be non-empty when `subpaths` is empty.
        #[serde(default)]
        anchors: Vec<OpPathAnchor>,
        /// Compound path contours. Must be non-empty when `anchors` is empty.
        #[serde(default)]
        subpaths: Vec<OpPathSubpath>,
    },
    /// Remove a node (and its subtree) by id from whatever container holds it.
    RemoveNode {
        /// The stable node `id` to remove.
        node: String,
    },
    /// Set the `opacity` of a node (0.0 = fully transparent, 1.0 = fully opaque).
    SetOpacity {
        /// The stable node `id` to target.
        node: String,
        /// New opacity value; clamped to `[0.0, 1.0]`.
        opacity: f64,
    },
    /// Replace the entire span list of a `text` node with a new set of spans.
    ReplaceText {
        /// The stable node `id` to target.
        node: String,
        /// Replacement span list. Each span's `text` is required; all other fields
        /// are optional and default to `None` (inherit from node-level styles).
        spans: Vec<OpSpan>,
    },
    /// Duplicate a node and its whole subtree, assigning the copy a new id, and
    /// insert the copy directly after the original.
    ///
    /// Every descendant id of the copy is the original id plus a suffix: what
    /// `new_id` adds to `node` (`box` to `box-copy` gives `-copy`), or
    /// `.{new_id}` when `new_id` does not start with `node`. A numeric
    /// tail (`-copy2`) is added when those ids exist already. `anchor-sibling`
    /// and connector `from` / `to` that name a node inside the subtree follow
    /// to the copy. References to nodes outside the subtree stay unchanged.
    DuplicateNode {
        /// The stable id of the node to duplicate.
        node: String,
        /// The id to assign to the root of the copy.
        new_id: String,
    },
    /// Duplicate an entire page (and its full subtree), inserting the copy
    DuplicatePage {
        /// Source page id to clone.
        page: String,
        /// Id for the new (duplicated) page.
        new_id: String,
        /// Suffix appended to EVERY descendant node id in the copy (keeps ids unique).
        id_suffix: String,
    },
    /// Wrap a set of sibling nodes inside a new group node.
    ///
    /// The group takes the slot of the earliest member. The members keep
    /// their document order (paint order) inside the group, whatever order
    /// `node_ids` lists them in.
    ///
    /// The children keep their page position, except in a row/column/grid
    /// frame: the new group takes a flow slot, and `tx.flow_placed` reports
    /// each grouped id. The CLI warns with `tx.page_box_changed` when a
    /// grouped node's compiled page box changes.
    Group {
        /// Ids of the nodes to group. Must be ≥ 1 and share a common parent.
        node_ids: Vec<String>,
        /// The id to assign to the newly created group node.
        group_id: String,
    },
    /// Dissolve a group node, moving its children up to the group's parent.
    ///
    /// The children take the group's slot, in their order inside the group.
    ///
    /// Each child shifts by the group origin, so it keeps its page position.
    /// A child that takes a flow slot of a layout parent frame does not shift:
    /// the frame places it, and `tx.flow_placed` reports it. When the group
    /// origin does not resolve to px, children keep their x/y and
    /// `tx.coordinate_unresolved` reports it. The CLI warns with
    /// `tx.page_box_changed` when a child's compiled page box changes.
    Ungroup {
        /// The id of the group node to dissolve.
        group_id: String,
    },
    /// Move a node to a different container (page, group, or frame).
    ///
    /// The node's x/y convert into the new container's space, so it keeps its
    /// page position. Into a flow slot of a row/column/grid frame, x/y stay
    /// unchanged: the frame places the node, siblings reflow, and
    /// `tx.flow_placed` reports it. When a container origin does not resolve
    /// to px, x/y stay unchanged and `tx.coordinate_unresolved` reports it.
    /// The CLI warns with `tx.page_box_changed` when the node's or a
    /// descendant's compiled page box changes.
    Reparent {
        /// The stable id of the node to move.
        node: String,
        /// The id of the container to move the node into.
        new_parent: String,
        /// Where to insert the node in the new parent. Defaults to `last`.
        #[serde(default)]
        position: Position,
    },
    /// Align a set of nodes to a common edge or centre along one axis.
    ///
    /// Boxes compare in page space (for a `page` or dimension anchor) or in
    /// the space of the deepest container they share. Each node is written
    /// back in its parent's space.
    AlignNodes {
        /// Ids of the nodes to align.
        node_ids: Vec<String>,
        /// Which edge or centre to align to: `left`, `hcenter`, `right`,
        /// `top`, `vcenter`, or `bottom`.
        align: String,
        /// Reference rectangle: `"selection"` (union bbox), `"page"`, a node id,
        /// or an explicit page-space dimension like `"(px)120"`. Defaults to
        /// `"selection"`.
        #[serde(default = "default_anchor")]
        anchor: String,
    },
    /// Set the `overflow` property of a `text` or `code` node.
    SetTextOverflow {
        /// The stable node `id` to target.
        node_id: String,
        /// The new overflow value: `fit`, `clip`, or `visible`.
        overflow: String,
    },
    /// Create a new EMPTY page (no children) and insert it into the document
    AddPage {
        /// Stable id for the new page (must be unique document-wide).
        id: String,
        /// Page width as a canonical dimension string, e.g. `"(px)1800"`.
        w: String,
        /// Page height as a canonical dimension string, e.g. `"(px)1200"`.
        h: String,
        /// Optional background token-ref id (e.g. `"color.bg"`). `None` = no fill.
        #[serde(default)]
        background: Option<String>,
        /// 0-based insert position. `None` appends at the end.
        #[serde(default)]
        index: Option<usize>,
    },
    /// Remove the page whose id == `page` (and its entire subtree) from the
    DeletePage {
        /// Id of the page to remove.
        page: String,
    },
    /// Reorder the document body's pages to match `order`.
    ReorderPages {
        /// The new full ordering of page ids (a permutation of the existing set).
        order: Vec<String>,
    },
    /// Declare a new asset in the document's `assets` block.
    AddAsset {
        /// Globally unique asset id (e.g. `"asset.logo"`).
        id: String,
        /// Asset kind string: `"image"`, `"svg"`, or `"font"`.
        kind: String,
        /// Relative path to the asset file.
        src: String,
        /// Optional SHA-256 hex digest for content integrity.
        #[serde(default)]
        sha256: Option<String>,
        /// Optional producer and AI-generation metadata.
        #[serde(default)]
        #[serde(flatten)]
        metadata: Box<AddAssetMetadata>,
    },
    /// Set the asset reference on an `image` node.
    SetAsset {
        /// The stable `id` of the image node to update.
        node_id: String,
        /// The asset id to assign to the image node's `asset` field.
        asset_id: String,
    },
    /// Evenly distribute a set of nodes along one axis so the gaps between
    /// them are equal. Boxes compare in the space of the deepest container
    /// they share, and each node is written back in its parent's space.
    DistributeNodes {
        /// Ids of the nodes to distribute.
        node_ids: Vec<String>,
        /// Axis to distribute along: `"horizontal"` or `"vertical"`.
        axis: String,
    },
    /// Create a new design token in the document's `tokens` block.
    CreateToken {
        /// Globally unique token id (e.g. `"color.brand"`).
        id: String,
        /// Token type string: scalar types, `"shadow"`, `"filter"`,
        /// `"gradient"`, or `"mask"`.
        #[serde(rename = "type")]
        token_type: String,
        /// Literal value for scalar types. Optional for structured types.
        #[serde(default)]
        value: String,
        /// Optional free-form provenance id (e.g. a theme/pack id). Omit for a
        /// plain, unstamped token.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        set: Option<String>,
        /// Shadow layers when `type` is `"shadow"`. Each `color` is a color
        /// token id.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        layers: Vec<ShadowLayerInput>,
        /// Filter ops when `type` is `"filter"`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        filter_ops: Vec<FilterOpInput>,
        /// Gradient stops when `type` is `"gradient"`. At least two required.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        stops: Vec<GradientStopInput>,
        /// Linear gradient angle in degrees (clockwise from +x). Default 0.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        angle: Option<f64>,
        /// When `true`, create a radial gradient (uses `center_x`/`center_y`/
        /// `radius`). Default linear.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        radial: Option<bool>,
        /// Radial center X as a fraction of box width (0..1). Default 0.5.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        center_x: Option<f64>,
        /// Radial center Y as a fraction of box height (0..1). Default 0.5.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        center_y: Option<f64>,
        /// Radial gradient radius (fraction of box diagonal) **or** rounded-mask
        /// corner radius in px, depending on `type`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        radius: Option<f64>,
        /// Mask coverage shape when `type` is `"mask"`: `"rect"`, `"rounded"`,
        /// or `"ellipse"`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shape: Option<String>,
        /// Mask feather sigma in px (>= 0). Default 0.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        feather: Option<f64>,
        /// Invert mask coverage. Default false.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        invert: Option<bool>,
    },
    /// Replace the literal value of an existing token, preserving its declared
    UpdateTokenValue {
        /// The id of the token to update.
        id: String,
        /// New literal value in string form appropriate for the token's existing type.
        value: String,
        /// Optional new provenance id to stamp on the token. Omit to leave the
        /// existing `set` unchanged, `null` to remove it.
        #[serde(
            default,
            deserialize_with = "super::layout::nullable",
            skip_serializing_if = "Option::is_none"
        )]
        set: Option<Option<String>>,
    },
    /// Set one recognized visual property on a named style to a token
    /// reference, or to an enum value for `align` / `v-align`.
    SetStyleProperty {
        /// The id of the style definition to update (matches `style id="…"`).
        style_id: String,
        /// The style property key to set (e.g. `font-family`, `fill`).
        /// Underscore spellings such as `font_family` are accepted.
        property: String,
        /// Token id to store as `PropertyValue::TokenRef` (e.g. `"font.body"`).
        /// For `align` / `v-align`, the enum value (e.g. `"center"`).
        value: String,
    },
    /// Create a named style in the document `styles { }` block.
    CreateStyle {
        /// Globally unique style id (e.g. `"cta.label"`).
        id: String,
        /// Map of style property key → token id. May be empty (properties can
        /// be filled later with `set_style_property`).
        #[serde(default)]
        properties: std::collections::BTreeMap<String, String>,
    },
    /// Remove a named style from the document `styles { }` block.
    DeleteStyle {
        /// The style id to remove.
        id: String,
    },
    /// Create an empty master-page definition in the document `masters { }` block.
    CreateMaster {
        /// Master id (must not collide with another master or page).
        id: String,
    },
    /// Remove a master-page definition from the document `masters { }` block.
    DeleteMaster {
        /// The master id to remove.
        id: String,
    },
    /// Set or clear a page's `master` attribute (shared chrome projection).
    SetPageMaster {
        /// Page id to update.
        page: String,
        /// Master id to assign, or `null`/omit to clear.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        master: Option<String>,
    },
    /// Set the `direction` property on a text node. Valid values: `"ltr"`, `"rtl"`.
    SetTextDirection {
        /// The stable node `id` to target.
        node: String,
        /// The new direction value: `"ltr"` or `"rtl"`.
        direction: String,
    },
    /// Literal find-and-replace across text node spans and shape label spans,
    FindReplaceText {
        /// The literal substring to search for (not a regex). Must be non-empty.
        find: String,
        /// The replacement string (may be empty to delete occurrences).
        replace: String,
        /// When `Some(id)`, only the named text node or shape is scoped.
        /// When `None`, all text nodes and shape labels in the document are scanned.
        #[serde(default)]
        node: Option<String>,
    },
    /// Resize a page (artboard). `w`/`h` are canonical dimension strings like
    SetPageSize {
        /// Id of the page to resize.
        page: String,
        /// New page width as a canonical dimension string, e.g. `"(px)794"`.
        w: String,
        /// New page height as a canonical dimension string, e.g. `"(px)1123"`.
        h: String,
    },
    /// Snap a single node's edge (or center) to the boundary of the page that
    /// contains it. The edge is in page space, and the node is written back in
    /// its parent's space.
    AlignToEdge {
        /// The stable node `id` to snap.
        node: String,
        /// Which edge or centre to snap to: `left`, `right`, `top`, `bottom`,
        /// `hcenter`, or `vcenter`.
        edge: String,
        /// Margin in pixels inset from the page edge. Defaults to 0. Ignored for
        /// `hcenter` and `vcenter`.
        #[serde(default)]
        margin: f64,
    },
    /// Create a new recipe entry in the document's `recipes` block.
    CreateRecipe {
        /// Globally unique recipe id (e.g. `"recipe.scatter"`).
        id: String,
        /// Generator kind string (e.g. `"scatter"`, `"aurora"`).
        kind: String,
        /// Optional integer seed for deterministic generation.
        #[serde(default)]
        seed: Option<i64>,
        /// Optional generator version/hash string (e.g. `"aurora@1"`).
        #[serde(default)]
        generator: Option<String>,
        /// Optional frame/page id this recipe applies within.
        #[serde(default)]
        bounds: Option<String>,
        /// Optional detach state: `true` = detached, `false` = linked.
        #[serde(default)]
        detached: Option<bool>,
    },
    /// Replace the scalar fields of an existing recipe, preserving its
    UpdateRecipe {
        /// The id of the recipe to update.
        id: String,
        /// New generator kind string.
        kind: String,
        /// New seed value; `null`/absent clears the field.
        #[serde(default)]
        seed: Option<i64>,
        /// New generator version/hash; `null`/absent clears the field.
        #[serde(default)]
        generator: Option<String>,
        /// New bounds frame/page id; `null`/absent clears the field.
        #[serde(default)]
        bounds: Option<String>,
        /// New detach state; `null`/absent clears the field.
        #[serde(default)]
        detached: Option<bool>,
    },
    /// Remove a recipe from the document's `recipes` block by id.
    DeleteRecipe {
        /// The id of the recipe to remove.
        id: String,
    },
    /// Upsert one `defaults` entry (node kind → default style) in the document
    /// or one page `defaults { }` block.
    SetDefault {
        /// Page id for a page-scope block, or `null`/omit for document scope.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page: Option<String>,
        /// Node kind the entry applies to (e.g. `"text"`, `"shape"`).
        kind: String,
        /// Style id the kind takes when a node sets no `style` of its own.
        style: String,
        /// Default label style id (`shape` and `connector` only). Omit to keep
        /// the current value, `null` to clear it, a style id to set it.
        #[serde(
            default,
            deserialize_with = "super::layout::nullable",
            skip_serializing_if = "Option::is_none"
        )]
        text_style: Option<Option<String>>,
    },
    /// Remove one `defaults` entry from the document or one page block.
    RemoveDefault {
        /// Page id for a page-scope block, or `null`/omit for document scope.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page: Option<String>,
        /// Node kind whose entry to remove.
        kind: String,
    },
    /// Set or clear auto-layout attributes: frame container fields (`layout`,
    /// `gap`, `padding*`, `justify`, `align`, `wrap`, `clip`) and item fields
    /// (`position`, `min_w`, `max_w`, `min_h`, `max_h`). `null` clears a field.
    SetLayout(LayoutEdit),
    /// Materialize a `pattern` node into an editable `group` of native shapes —
    DetachPattern {
        /// The stable id of the pattern node to detach into a native group.
        node: String,
    },
}

fn default_anchor() -> String {
    "selection".to_owned()
}
