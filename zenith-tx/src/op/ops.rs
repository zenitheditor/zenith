//! The [`Op`] enum: every mutating operation a [`super::Transaction`] can carry.

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
    SetGeometry {
        /// The stable node `id` to target.
        node: String,
        /// New left edge in pixels. Omit to leave unchanged.
        #[serde(default)]
        x: Option<f64>,
        /// New top edge in pixels. Omit to leave unchanged.
        #[serde(default)]
        y: Option<f64>,
        /// New width in pixels. Omit to leave unchanged.
        #[serde(default)]
        w: Option<f64>,
        /// New height in pixels. Omit to leave unchanged.
        #[serde(default)]
        h: Option<f64>,
        /// New rotation in degrees. Omit to leave unchanged.
        #[serde(default)]
        rotate: Option<f64>,
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
    /// Duplicate a leaf node, assigning it a new id, and insert the clone
    DuplicateNode {
        /// The stable id of the node to duplicate.
        node: String,
        /// The id to assign to the newly created clone.
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
    Group {
        /// Ids of the nodes to group. Must be ≥ 1 and share a common parent.
        node_ids: Vec<String>,
        /// The id to assign to the newly created group node.
        group_id: String,
    },
    /// Dissolve a group node, moving its children up to the group's parent.
    Ungroup {
        /// The id of the group node to dissolve.
        group_id: String,
    },
    /// Move a node to a different container (page, group, or frame).
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
    AlignNodes {
        /// Ids of the nodes to align.
        node_ids: Vec<String>,
        /// Which edge or centre to align to: `left`, `hcenter`, `right`,
        /// `top`, `vcenter`, or `bottom`.
        align: String,
        /// Reference rectangle: `"selection"` (union bbox), `"page"`, a node id,
        /// or an explicit dimension like `"(px)120"`. Defaults to `"selection"`.
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
        /// existing `set` unchanged.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        set: Option<String>,
    },
    /// Set one recognized visual property on a named style to a token reference.
    SetStyleProperty {
        /// The id of the style definition to update (matches `style id="…"`).
        style_id: String,
        /// The style property key to set (e.g. `font-family`, `fill`).
        /// Underscore spellings such as `font_family` are accepted.
        property: String,
        /// Token id to store as `PropertyValue::TokenRef` (e.g. `"font.body"`).
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
    /// Materialize a `pattern` node into an editable `group` of native shapes —
    DetachPattern {
        /// The stable id of the pattern node to detach into a native group.
        node: String,
    },
}

fn default_anchor() -> String {
    "selection".to_owned()
}
