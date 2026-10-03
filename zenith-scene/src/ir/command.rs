//! The `SceneCommand` display-list enum and its serde skip predicate.

use serde::Serialize;

use super::effects::{is_center, is_nonzero, serialize_fill_rule_as_bool};
use super::{
    BlendMode, Color, FillRule, FilterSpec, FitMode, ImageClip, LineCap, LineJoin, MaskSpec, Paint,
    PathSegment, SceneGlyph, ShadowSpec, SrcRect, StrokeAlign, SvgStyle,
};

// ── Scene commands ────────────────────────────────────────────────────────────

/// A single display-list command in the scene.
///
/// All variants are tagged in JSON via `#[serde(tag = "op")]` so that each
/// serialized command carries an `"op"` field naming the primitive, e.g.
/// `{ "op": "FillRect", "x": 0.0, … }`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "op")]
pub enum SceneCommand {
    // ── Filled shapes ─────────────────────────────────────────────────────
    /// Fill an axis-aligned rectangle.
    FillRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        paint: Paint,
    },
    /// Stroke an axis-aligned rectangle (inside the declared edge by default).
    StrokeRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        color: Color,
        stroke_width: f64,
        /// Dash segment length in pixels. `None` = solid stroke (byte-identical to prior IR).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke_dash: Option<f64>,
        /// Gap length in pixels between dashes. `None` = solid stroke.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke_gap: Option<f64>,
        /// Dash end-cap style. `None` = Butt (default, byte-identical to prior IR).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke_linecap: Option<LineCap>,
    },
    /// Fill a rectangle with uniform corner radius (and optional per-corner overrides).
    FillRoundedRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        radius: f64,
        paint: Paint,
        /// Per-corner radii `[tl, tr, br, bl]`. `None` = use uniform `radius` for all
        /// corners (byte-identical to prior IR when absent).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        radii: Option<[f64; 4]>,
    },
    /// Stroke a rectangle with uniform corner radius (and optional per-corner overrides).
    StrokeRoundedRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        radius: f64,
        color: Color,
        stroke_width: f64,
        /// Dash segment length in pixels. `None` = solid stroke (byte-identical to prior IR).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke_dash: Option<f64>,
        /// Gap length in pixels between dashes. `None` = solid stroke.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke_gap: Option<f64>,
        /// Dash end-cap style. `None` = Butt (default, byte-identical to prior IR).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke_linecap: Option<LineCap>,
        /// Per-corner radii `[tl, tr, br, bl]`. `None` = use uniform `radius` for all
        /// corners (byte-identical to prior IR when absent).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        radii: Option<[f64; 4]>,
    },
    /// Fill an axis-aligned ellipse.
    FillEllipse {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        paint: Paint,
        /// Explicit x-radius (overrides w/2). `None` = inscribed ellipse (byte-identical).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rx: Option<f64>,
        /// Explicit y-radius (overrides h/2). `None` = inscribed ellipse (byte-identical).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ry: Option<f64>,
    },
    /// Stroke an axis-aligned ellipse (centered on the ellipse path; no
    /// stroke-alignment in v0).
    StrokeEllipse {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        color: Color,
        stroke_width: f64,
        /// Dash segment length in pixels. `None` = solid stroke (byte-identical to prior IR).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke_dash: Option<f64>,
        /// Gap length in pixels between dashes. `None` = solid stroke.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke_gap: Option<f64>,
        /// Dash end-cap style. `None` = Butt (default, byte-identical to prior IR).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke_linecap: Option<LineCap>,
        /// Explicit x-radius (overrides w/2). `None` = inscribed ellipse (byte-identical).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rx: Option<f64>,
        /// Explicit y-radius (overrides h/2). `None` = inscribed ellipse (byte-identical).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ry: Option<f64>,
    },
    /// Stroke a line segment.
    StrokeLine {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        color: Color,
        stroke_width: f64,
        /// Dash segment length in pixels. `None` = solid stroke (byte-identical to prior IR).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke_dash: Option<f64>,
        /// Gap length in pixels between dashes. `None` = solid stroke.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke_gap: Option<f64>,
        /// Dash end-cap style. `None` = Butt (default, byte-identical to prior IR).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke_linecap: Option<LineCap>,
    },
    /// Fill a closed polygon.
    FillPolygon {
        /// Flat list of `[x0, y0, x1, y1, …]` vertex coordinates.
        points: Vec<f64>,
        paint: Paint,
        /// Fill rule serialized through the legacy `even_odd` boolean field.
        #[serde(
            default,
            rename = "even_odd",
            serialize_with = "serialize_fill_rule_as_bool"
        )]
        fill_rule: FillRule,
    },
    /// Stroke a polyline (open or closed depending on `closed`).
    StrokePolyline {
        /// Flat list of `[x0, y0, x1, y1, …]` vertex coordinates.
        points: Vec<f64>,
        color: Color,
        stroke_width: f64,
        /// When `true`, the path is closed before stroking (polygon outline).
        #[serde(default)]
        closed: bool,
        /// Stroke alignment relative to the closed-path boundary. Only meaningful
        /// when `closed` is `true`; `Center` is the open-path/default behavior.
        /// Skipped in JSON when `Center` so existing scenes serialize byte-identically.
        #[serde(default, skip_serializing_if = "is_center")]
        align: StrokeAlign,
        /// Fill rule of the clip region used for `Inside`/`Outside` alignment.
        /// Serialized through the legacy `fill_even_odd` boolean field. Only
        /// meaningful when `align != Center` and `closed` is `true`.
        #[serde(
            default,
            rename = "fill_even_odd",
            serialize_with = "serialize_fill_rule_as_bool",
            skip_serializing_if = "is_nonzero"
        )]
        clip_fill_rule: FillRule,
    },
    /// Fill a structured path with line and cubic Bezier segments.
    FillPath {
        segments: Vec<PathSegment>,
        paint: Paint,
        /// Fill rule serialized through the legacy `even_odd` boolean field.
        #[serde(
            default,
            rename = "even_odd",
            serialize_with = "serialize_fill_rule_as_bool"
        )]
        fill_rule: FillRule,
    },
    /// Stroke a structured path with line and cubic Bezier segments.
    StrokePath {
        segments: Vec<PathSegment>,
        color: Color,
        stroke_width: f64,
        /// Whether the source path is closed; used for stroke-alignment semantics.
        #[serde(default)]
        closed: bool,
        /// Stroke alignment relative to the closed-path boundary.
        #[serde(default, skip_serializing_if = "is_center")]
        align: StrokeAlign,
        /// Fill rule of the clip region used for `Inside`/`Outside` alignment.
        #[serde(
            default,
            rename = "fill_even_odd",
            serialize_with = "serialize_fill_rule_as_bool",
            skip_serializing_if = "is_nonzero"
        )]
        clip_fill_rule: FillRule,
        /// Stroke corner join style. `None` = Miter (renderer default).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke_linejoin: Option<LineJoin>,
        /// Stroke end-cap style. `None` = Butt (renderer default).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke_linecap: Option<LineCap>,
        /// Stroke miter limit. `None` = renderer default.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke_miter_limit: Option<f64>,
    },
    // ── Asset commands ────────────────────────────────────────────────────
    /// Draw a raster image asset clipped to its declared box.
    ///
    /// The renderer re-resolves bytes via `AssetProvider::by_id` using only the
    /// `asset_id` string — no raw image bytes appear in the IR. `pos_x`/`pos_y`
    /// are the object-position anchors resolved to `0.0..=100.0`.
    DrawImage {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        /// Stable asset id; renderer resolves bytes via `AssetProvider::by_id`.
        asset_id: String,
        /// How the image scales to fill the box.
        fit: FitMode,
        /// Horizontal object-position anchor in `0.0..=100.0`.
        pos_x: f64,
        /// Vertical object-position anchor in `0.0..=100.0`.
        pos_y: f64,
        /// Effective opacity (node opacity × cascaded ctx opacity), `0.0..=1.0`.
        opacity: f64,
        /// Optional non-rectangular clip shape inscribed in the box. `None` =
        /// the default rectangular box-clip (existing behavior, unchanged).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        clip_shape: Option<ImageClip>,
        /// Optional source sub-rectangle selecting a crop of the source image
        /// before the fit/object-position math is applied. `None` = use the
        /// full source image (byte-identical to scenes without `src_rect`).
        /// Applies to raster assets only; ignored for SVG.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        src_rect: Option<SrcRect>,
        /// SVG-only style overrides. Ignored for raster assets.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        svg_style: Option<SvgStyle>,
    },
    /// Draw a pre-resolved SVG asset.
    DrawSvgAsset {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        /// Asset path (project-relative).
        asset: String,
    },
    // ── Text ──────────────────────────────────────────────────────────────
    /// Draw a shaped, positioned glyph run.
    ///
    /// `x` is the text-box origin x in pixels; `y` is the baseline y in
    /// pixels (`text_box_top + ascent`).  The renderer re-resolves font bytes
    /// via `FontProvider::by_id` using only the `font_id` string — no raw
    /// font bytes appear in the IR.
    DrawGlyphRun {
        /// Text-box origin x in pixels.
        x: f64,
        /// Baseline y in pixels (`text_box_top + ascent`).
        y: f64,
        /// Stable font-face identifier; renderer resolves bytes via
        /// `FontProvider::by_id`.
        font_id: String,
        /// Font size at which glyphs were shaped, in pixels.
        font_size: f32,
        /// Fill color of the glyph run.
        color: Color,
        /// Optional stroke (outline) color applied after the fill.
        /// `None` means no outline — byte-identical to a run without stroke.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke_color: Option<Color>,
        /// Stroke width in pixels. Ignored (and serialized as absent) when
        /// `stroke_color` is `None` or `stroke_width` is `<= 0`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke_width: Option<f64>,
        /// Optional hyperlink URL for this run. When set and the run is
        /// `selectable`, the PDF backend emits a clickable Link annotation over
        /// the run's bounds. `None` = no link — byte-identical to a run without
        /// one. The raster backend ignores it (no clickable concept).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        link: Option<String>,
        /// Whether this run's text is selectable / searchable / indexable in the
        /// PDF backend. `true` (default) → real embedded text + ToUnicode;
        /// `false` → filled glyph outlines (visually identical, not extractable).
        /// The raster backend ignores it. Serialized only when `false`, so
        /// default runs stay byte-identical.
        #[serde(skip_serializing_if = "is_selectable")]
        selectable: bool,
        /// Authored source `text`/`code` node id that produced this run.
        /// Runtime-only attribution for outline materialization; never serialized.
        #[serde(default, skip_serializing, skip_deserializing)]
        source_node_id: Option<String>,
        /// Positioned glyphs, baseline-relative.
        glyphs: Vec<SceneGlyph>,
    },
    // ── Clip / layer stack ────────────────────────────────────────────────
    /// Push an axis-aligned clip rectangle onto the clip stack.
    PushClip { x: f64, y: f64, w: f64, h: f64 },
    /// Push a rounded-rectangle clip with one corner `radius` onto the clip
    /// stack. `PopClip` pops it. Backends clamp `radius` to `min(w, h) / 2`.
    /// Compilers emit `PushClip` instead when the radius is 0.
    PushClipRoundedRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        radius: f64,
    },
    /// Pop the most-recently pushed clip (`PushClip` or `PushClipRoundedRect`).
    PopClip,
    /// Push a compositing layer (for opacity, blend, mask).
    ///
    /// `opacity` is the layer alpha applied when the layer is composited back
    /// onto its parent. `blend_mode` selects the compositing operator used for
    /// that composite; `None` (and `Some(BlendMode::Normal)`) mean plain
    /// source-over and serialize identically to a layer with no blend.
    PushLayer {
        opacity: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        blend_mode: Option<BlendMode>,
    },
    /// Pop the most-recently pushed compositing layer.
    PopLayer,
    /// Push an affine rotation around a pivot; composes onto the renderer's transform stack.
    PushTransform { angle_deg: f64, cx: f64, cy: f64 },
    /// Push a restricted scale+translate transform for composed page content.
    ///
    /// This is intentionally narrower than a general affine matrix so raster
    /// clipping can stay deterministic with axis-aligned clip rectangles.
    PushScaleTranslate { sx: f64, sy: f64, tx: f64, ty: f64 },
    /// Push an arbitrary affine matrix; composes onto the renderer's transform
    /// stack. The six coefficients follow the `x' = a·x + c·y + e`,
    /// `y' = b·x + d·y + f` convention (the row order of tiny-skia
    /// `Transform::from_row` and the PDF `cm` operator).
    ///
    /// Unlike author-facing transforms, this variant is only ever emitted by the
    /// compiler for exactly-computed reflection/rotation composites (mirror
    /// symmetry). The coefficients are pinned f64 values, so the same document
    /// produces the same matrix on any machine.
    PushTransformMatrix {
        a: f64,
        b: f64,
        c: f64,
        d: f64,
        e: f64,
        f: f64,
    },
    /// Pop the most recent pushed transform.
    PopTransform,
    // ── Shadow capture ────────────────────────────────────────────────────
    /// Open an isolated capture of the following draw commands. The captured
    /// ink is buffered offscreen until the matching [`SceneCommand::EndShadow`].
    ///
    /// `shadows` are painted in *reverse* order at `EndShadow` (so the
    /// first-declared layer ends up on top of later layers), all *behind* the
    /// crisp ink.
    BeginShadow { shadows: Vec<ShadowSpec> },
    /// Close the active shadow capture: paint the blurred shadow layers, then
    /// composite the captured ink on top.
    EndShadow,
    // ── Gaussian blur capture ─────────────────────────────────────────────
    /// Open an offscreen capture of the following draw commands and apply a
    /// Gaussian blur with `radius` (sigma in pixels) to the captured ink at
    /// [`SceneCommand::EndBlur`]. `radius == 0` is a no-op (no capture opened).
    BeginBlur { radius: f64 },
    /// Close the active blur capture: blur the captured ink in place, then
    /// composite it onto the current target.
    EndBlur,
    // ── Color filter capture ──────────────────────────────────────────────
    /// Open an offscreen capture; apply `filters` in order to the captured ink
    /// at the matching EndFilter, then composite back. Empty `filters` opens no capture.
    BeginFilter { filters: Vec<FilterSpec> },
    /// Close the active filter capture: transform the captured ink in place, composite onto the target.
    EndFilter,
    // ── Soft-mask capture ─────────────────────────────────────────────────
    /// Open an offscreen capture of the following draw commands; at the
    /// matching [`SceneCommand::EndMask`] the captured ink is composited back
    /// through the feathered coverage described by `mask`.
    BeginMask { mask: MaskSpec },
    /// Close the active mask capture: composite the captured ink through the
    /// mask coverage onto the current target.
    EndMask,
}

/// Serde skip predicate for `DrawGlyphRun::selectable`: omit the default `true`.
fn is_selectable(selectable: &bool) -> bool {
    *selectable
}
