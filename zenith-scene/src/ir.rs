//! Scene IR — the backend-neutral display-list primitives.
//!
//! Every type in this module derives `Debug`, `Clone`, `PartialEq`, and
//! `serde::Serialize`.  No `HashMap` or `HashSet` is used anywhere in this
//! module, so JSON serialization is deterministic (struct field order is
//! stable; `BTreeMap` would be used if maps were ever needed).
//!
//! The `scene` field name is always the first field in `Scene` so the
//! `schema` key appears first in the serialized JSON.

use serde::Serialize;
use zenith_geometry::{CubicBezier, Point2, RectBounds};

pub use zenith_core::{BlendMode, Color, GradientPaint, GradientStop};

// ── LineCap ───────────────────────────────────────────────────────────────────

/// Stroke end-cap style.
///
/// Maps directly to the `tiny_skia::LineCap` values; serialized in lowercase
/// JSON so the scene JSON is human-readable and matches the KDL attribute values.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

// ── LineJoin ──────────────────────────────────────────────────────────────────

/// Stroke corner join style.
///
/// `None` at command sites means the renderer default (`Miter`) and preserves
/// prior serialized IR. Serialized in lowercase JSON to match the KDL
/// `stroke-linejoin` values.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

// ── StrokeAlign ─────────────────────────────────────────────────────────────────

/// Stroke alignment relative to a closed polygon's boundary.
///
/// `Center` (the default) strokes centered on the path — identical to the prior
/// IR and the only alignment valid for open polylines. `Inside`/`Outside` shift
/// the visible stroke fully inside / outside the fill boundary; the renderer
/// implements them via a fill-region clip mask, so self-intersecting shapes
/// (stars) and rotation are handled without geometry offsetting. Serialized in
/// lowercase JSON to match the KDL `stroke-alignment` attribute values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StrokeAlign {
    #[default]
    Center,
    Inside,
    Outside,
}

// ── FillRule ─────────────────────────────────────────────────────────────────

/// Fill rule for closed scene geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FillRule {
    #[default]
    NonZero,
    EvenOdd,
}

impl FillRule {
    pub(crate) fn from_author_value(value: Option<&str>) -> Self {
        match value {
            Some("evenodd") => Self::EvenOdd,
            Some(_) | None => Self::NonZero,
        }
    }
}

/// Structured scene path segment, preserving cubic Bezier geometry for native
/// raster and PDF backends.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind")]
pub enum PathSegment {
    MoveTo {
        x: f64,
        y: f64,
    },
    LineTo {
        x: f64,
        y: f64,
    },
    CubicTo {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        x: f64,
        y: f64,
    },
    Close,
}

/// Return `true` when every coordinate in `segments` is finite.
pub fn path_segments_finite(segments: &[PathSegment]) -> bool {
    segments.iter().all(|segment| match segment {
        PathSegment::MoveTo { x, y } | PathSegment::LineTo { x, y } => {
            x.is_finite() && y.is_finite()
        }
        PathSegment::CubicTo {
            x1,
            y1,
            x2,
            y2,
            x,
            y,
        } => {
            x1.is_finite()
                && y1.is_finite()
                && x2.is_finite()
                && y2.is_finite()
                && x.is_finite()
                && y.is_finite()
        }
        PathSegment::Close => true,
    })
}

/// Axis-aligned bounding box `(x, y, w, h)` of a structured path segment list.
pub fn path_segments_bbox(segments: &[PathSegment]) -> Option<(f64, f64, f64, f64)> {
    let mut bounds: Option<RectBounds> = None;
    let mut current: Option<Point2> = None;
    for segment in segments {
        match segment {
            PathSegment::MoveTo { x, y } | PathSegment::LineTo { x, y } => {
                let point = Point2::new(*x, *y).ok()?;
                bounds = Some(include_point(bounds, point));
                current = Some(point);
            }
            PathSegment::CubicTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                let p1 = Point2::new(*x1, *y1).ok()?;
                let p2 = Point2::new(*x2, *y2).ok()?;
                let end = Point2::new(*x, *y).ok()?;
                bounds = Some(match current {
                    Some(start) => {
                        let curve = CubicBezier::new(start, p1, p2, end).ok()?;
                        include_bounds(bounds, curve.bounds().ok()?)
                    }
                    None => include_point(bounds, p1)
                        .include_point(p2)
                        .include_point(end),
                });
                current = Some(end);
            }
            PathSegment::Close => {}
        }
    }
    let bounds = bounds?;
    Some((bounds.min_x, bounds.min_y, bounds.width(), bounds.height()))
}

fn include_point(bounds: Option<RectBounds>, point: Point2) -> RectBounds {
    bounds.map_or_else(|| RectBounds::from_point(point), |b| b.include_point(point))
}

fn include_bounds(bounds: Option<RectBounds>, next: RectBounds) -> RectBounds {
    bounds.map_or(next, |b| b.include_bounds(next))
}

// ── Paint ───────────────────────────────────────────────────────────────────

/// How a filled region is painted.
///
/// Every fill command carries a `Paint`, so any geometry (rectangle, rounded
/// rectangle, ellipse, polygon, …) can be filled with a flat color or a
/// gradient through one uniform model — there is no per-geometry gradient
/// command. New fill kinds (e.g. patterns) are added here as one more variant,
/// and the exhaustive matches over `Paint` force every backend to handle them.
///
/// Serialized internally-tagged on `kind` so the JSON is self-describing:
/// `{ "kind": "solid", "color": {…} }` or
/// `{ "kind": "gradient", "angle_deg": …, "stops": [...] }`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Paint {
    /// A flat fill color.
    Solid {
        /// The fill color (straight / un-pre-multiplied alpha).
        color: Color,
    },
    /// A linear or radial gradient.
    Gradient(GradientPaint),
}

impl Paint {
    /// Construct a solid paint from a color.
    pub fn solid(color: Color) -> Self {
        Paint::Solid { color }
    }
}

// ── Shadow ────────────────────────────────────────────────────────────────────

/// A single drop-shadow / outer-glow layer.
///
/// `dx`/`dy` are the offset (pixels) of the shadow relative to the ink; `blur`
/// is the Gaussian blur sigma (pixels, `>= 0`); `color` is the shadow color
/// (straight / un-pre-multiplied alpha). A node may carry several layers, all
/// painted behind the ink.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ShadowSpec {
    /// Horizontal offset in pixels (positive = rightward).
    pub dx: f64,
    /// Vertical offset in pixels (positive = downward).
    pub dy: f64,
    /// Gaussian blur sigma in pixels (`>= 0`).
    pub blur: f64,
    /// Shadow color (straight / un-pre-multiplied alpha).
    pub color: Color,
}

// ── Filter ──────────────────────────────────────────────────────────────────

/// A single color-filter operation applied to captured ink (straight-alpha math).
///
/// Each variant carries its already-resolved scalar payload (the per-kind
/// `amount`, defaults substituted at compile time). `Duotone` additionally
/// carries its two resolved colors — the scene IR stays decoupled from the core
/// AST, exactly as [`ShadowSpec`] carries a scene-local [`Color`] rather than a
/// color-token id. The compile step maps core → scene.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum FilterSpec {
    Grayscale(f64),
    Invert(f64),
    Sepia(f64),
    Saturate(f64),
    Brightness(f64),
    Contrast(f64),
    HueRotate(f64),
    /// Maps luma to a blend between `shadow` (dark) and `highlight` (light),
    /// then mixes with the original by `amount`.
    Duotone {
        amount: f64,
        shadow: Color,
        highlight: Color,
    },
    /// Deterministic monochrome additive film grain: adds the same per-pixel
    /// delta to R, G, and B, derived from an integer hash of the page-absolute
    /// pixel cell and `seed`. `amount` scales the grain magnitude; `scale` is the
    /// grain cell size in pixels. Same inputs → same grain on any machine.
    Noise {
        amount: f64,
        seed: i64,
        scale: f64,
    },
}

// ── Mask ──────────────────────────────────────────────────────────────────────

/// The spatial coverage shape of a node mask.
///
/// Mirrors `zenith_core::MaskShape`; the compile step maps core → scene so the
/// scene IR stays decoupled from the core AST (exactly as [`FilterSpec`] carries
/// scene-local payloads rather than core token ids).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum MaskShape {
    Rect,
    RoundedRect,
    Ellipse,
}

/// A resolved soft-mask applied to a node's draws.
///
/// The mask coverage is the `shape` inscribed in the node box `[x, y, w, h]`
/// (page-absolute pixels), optionally with a corner `radius` (RoundedRect),
/// a Gaussian `feather` sigma (`>= 0`), and an `invert` flag. The renderer
/// brackets the node's draws with [`SceneCommand::BeginMask`] /
/// [`SceneCommand::EndMask`] and composites the captured ink through the
/// feathered coverage.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct MaskSpec {
    pub shape: MaskShape,
    /// Resolved corner radius in pixels (RoundedRect; `0.0` otherwise).
    pub radius: f64,
    /// Gaussian feather sigma in pixels (`>= 0`).
    pub feather: f64,
    pub invert: bool,
    /// Node box, page-absolute pixels.
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

// ── Fit mode ────────────────────────────────────────────────────────────────

/// How a raster image asset scales to fill its declared box.
///
/// - `Contain` — scale to fit entirely inside the box (letterboxed).
/// - `Cover` — scale to cover the whole box (cropped, clipped to the box).
/// - `Stretch` — scale each axis independently to exactly fill the box.
/// - `None` — draw at native pixel size, anchored by object-position.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FitMode {
    Contain,
    Cover,
    Stretch,
    None,
}

// ── Image source rect ─────────────────────────────────────────────────────────

/// A sub-rectangle within the source image used as the effective source for a
/// [`SceneCommand::DrawImage`] command.
///
/// All four coordinates are in source-image pixels (top-left origin). The rect
/// is clamped to the source image bounds at render time; a degenerate rect (zero
/// width or height after clamping) causes the draw to be skipped.
///
/// Applies to raster `kind="image"` assets only; ignored for SVG assets (vector
/// assets are resolution-independent and src-rect is a raster concept). This is
/// a documented v0 limitation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SrcRect {
    /// Left edge of the crop in source pixels.
    pub x: f64,
    /// Top edge of the crop in source pixels.
    pub y: f64,
    /// Width of the crop in source pixels (> 0).
    pub w: f64,
    /// Height of the crop in source pixels (> 0).
    pub h: f64,
}

/// Optional SVG-only styling applied at render time before SVG parsing.
///
/// These fields never mutate source asset bytes; they parameterize rendering of
/// `currentColor` / root stroke-fill attributes after locked asset verification.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct SvgStyle {
    /// Override for SVG stroke/currentColor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Color>,
    /// Override for SVG fill/currentColor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Color>,
    /// Override for SVG stroke-width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke_width: Option<f64>,
}

// ── Image clip shape ──────────────────────────────────────────────────────────

/// A non-rectangular clip shape applied to a [`SceneCommand::DrawImage`].
///
/// `None` on the `DrawImage` (no `clip_shape`) means the default rectangular
/// box-clip (the raster is clipped to its declared `[x, y, w, h]` box). A
/// `Some` value constrains the blit to a shape INSCRIBED in that box:
///
/// - `Ellipse` — the ellipse inscribed in the box (a circle when the box is
///   square): the circular-avatar case.
/// - `RoundedRect { radius }` — a rounded rectangle with uniform corner radius.
///
/// Tagged in JSON via `#[serde(tag = "shape")]` for a self-describing form,
/// consistent with the `op`-tagged [`SceneCommand`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "shape")]
pub enum ImageClip {
    /// Clip to the ellipse inscribed in the image's `[x, y, w, h]` box.
    Ellipse,
    /// Clip to a rounded rectangle with uniform corner `radius` (pixels).
    RoundedRect { radius: f64 },
}

fn is_center(a: &StrokeAlign) -> bool {
    matches!(a, StrokeAlign::Center)
}

fn is_nonzero(rule: &FillRule) -> bool {
    matches!(rule, FillRule::NonZero)
}

fn serialize_fill_rule_as_bool<S>(rule: &FillRule, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_bool(matches!(rule, FillRule::EvenOdd))
}

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
    /// Pop the most-recently pushed clip rectangle.
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

// ── Scene glyph ───────────────────────────────────────────────────────────────

/// A single positioned glyph within a [`SceneCommand::DrawGlyphRun`].
///
/// Offsets `dx` and `dy` are pen offsets from the run origin, baseline-relative.
/// Positive `dx` is rightward; positive `dy` is downward (0 = on the baseline).
/// No font bytes appear here — only the glyph ID within the resolved font face.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SceneGlyph {
    /// Glyph identifier within the resolved font face.
    pub glyph_id: u16,
    /// Horizontal pen offset from the run origin, in pixels.
    pub dx: f32,
    /// Vertical offset from the baseline, in pixels (positive = below baseline).
    pub dy: f32,
    /// Source Unicode text this glyph maps back to, for text extraction
    /// (PDF ToUnicode CMap). Empty for the trailing glyphs of a multi-glyph
    /// cluster and for runs that carry no source mapping. Serialized only when
    /// non-empty, so scenes without it stay byte-identical.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
}

// ── Trim rect ───────────────────────────────────────────────────────────────

/// An axis-aligned rectangle in scene (top-left origin, y-down) coordinates,
/// in pixels.
///
/// Used to carry the print **trim box** on a [`Scene`] when a page declares a
/// positive `bleed` margin. The scene canvas (`width`/`height`) is the full
/// media box *including* the bleed; the trim rect is the inner rectangle the
/// finished piece is cut down to.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Rect {
    /// Left edge in pixels (scene coordinates).
    pub x: f64,
    /// Top edge in pixels (scene coordinates).
    pub y: f64,
    /// Width in pixels.
    pub w: f64,
    /// Height in pixels.
    pub h: f64,
}

// ── Scene ─────────────────────────────────────────────────────────────────────

/// A fully resolved, backend-neutral display list.
///
/// The `schema` field is always `"zenith-scene-v1"` and is declared first so
/// that it serializes as the first key in the JSON output, satisfying the
/// normative requirement from the format spec.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Scene {
    /// Always `"zenith-scene-v1"`.  Declared first so it appears first in JSON.
    pub schema: &'static str,
    /// Page / canvas width in pixels.
    pub width: f64,
    /// Page / canvas height in pixels.
    pub height: f64,
    /// Ordered display list.  Paint order: index 0 is painted first (bottom).
    pub commands: Vec<SceneCommand>,
    /// Print **trim box** in scene (top-left origin, y-down) pixel coordinates.
    ///
    /// `Some` only when the page declared a positive `bleed` margin: then
    /// `width`/`height` are the full media box (including bleed) and `trim` is
    /// the inner page rectangle `[b, b, page_w, page_h]`. `None` when there is
    /// no bleed (trim == media box). Skipped in JSON when absent so existing
    /// non-bleed scenes serialize byte-identically.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trim: Option<Rect>,
}

impl Scene {
    /// Construct an empty scene for the given page dimensions.
    ///
    /// `schema` is always set to `"zenith-scene-v1"`.
    pub fn new(width: f64, height: f64) -> Self {
        Self {
            schema: "zenith-scene-v1",
            width,
            height,
            commands: Vec::new(),
            trim: None,
        }
    }

    /// Serialize this scene to a pretty-printed JSON string.
    ///
    /// Uses `serde_json::to_string_pretty` which produces deterministic output
    /// because `Scene` and its fields use only `Vec` (ordered) and `struct`
    /// (stable field order in Rust + serde), never `HashMap`.
    ///
    /// # Errors
    ///
    /// Returns an error only if serialization fails, which cannot happen for
    /// the types used in `Scene` (all fields are plain numerics, strings, and
    /// `u8`s).  The `Result` is kept for API robustness.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_new_sets_schema() {
        let s = Scene::new(800.0, 600.0);
        assert_eq!(s.schema, "zenith-scene-v1");
        assert_eq!(s.width, 800.0);
        assert_eq!(s.height, 600.0);
        assert!(s.commands.is_empty());
    }

    #[test]
    fn to_json_schema_is_first_key() {
        let s = Scene::new(100.0, 200.0);
        let json = s.to_json().expect("serialization must succeed");
        // The very first `"` after `{` must be `"schema"`.
        let trimmed = json.trim_start_matches('{').trim_start();
        assert!(
            trimmed.starts_with(r#""schema""#),
            "schema must be the first JSON key; got: {trimmed}"
        );
    }

    #[test]
    fn to_json_deterministic() {
        let mut s = Scene::new(640.0, 360.0);
        s.commands.push(SceneCommand::FillRect {
            x: 0.0,
            y: 0.0,
            w: 640.0,
            h: 360.0,
            paint: Paint::solid(Color::srgb(10, 20, 30, 255)),
        });
        let a = s.to_json().expect("first serialize");
        let b = s.to_json().expect("second serialize");
        assert_eq!(a, b, "serialization must be deterministic");
    }

    #[test]
    fn fill_rect_serializes_op_tag() {
        let cmd = SceneCommand::FillRect {
            x: 1.0,
            y: 2.0,
            w: 3.0,
            h: 4.0,
            paint: Paint::solid(Color::srgb(255, 0, 0, 255)),
        };
        let json = serde_json::to_string(&cmd).expect("serialize");
        assert!(
            json.contains(r#""op":"FillRect""#),
            "op tag must be FillRect; got: {json}"
        );
    }

    #[test]
    fn glyph_run_source_node_id_does_not_serialize() {
        let cmd = SceneCommand::DrawGlyphRun {
            x: 1.0,
            y: 2.0,
            font_id: "noto-sans-400-normal".to_owned(),
            font_size: 12.0,
            color: Color::srgb(0, 0, 0, 255),
            stroke_color: None,
            stroke_width: None,
            link: None,
            selectable: true,
            source_node_id: Some("text.source".to_owned()),
            glyphs: vec![SceneGlyph {
                glyph_id: 1,
                dx: 0.0,
                dy: 0.0,
                text: "A".to_owned(),
            }],
        };
        let json = serde_json::to_string(&cmd).expect("serialize");
        assert!(!json.contains("source_node_id"), "got: {json}");
    }

    #[test]
    fn path_segments_bbox_uses_cubic_extrema() {
        let segments = [
            PathSegment::MoveTo { x: 0.0, y: 0.0 },
            PathSegment::CubicTo {
                x1: 0.0,
                y1: 10.0,
                x2: 10.0,
                y2: 10.0,
                x: 10.0,
                y: 0.0,
            },
        ];

        assert_eq!(path_segments_bbox(&segments), Some((0.0, 0.0, 10.0, 7.5)));
    }

    #[test]
    fn path_segments_bbox_rejects_non_finite_coordinates() {
        let segments = [PathSegment::MoveTo {
            x: f64::NAN,
            y: 0.0,
        }];

        assert_eq!(path_segments_bbox(&segments), None);
    }

    #[test]
    fn srgb_color_omits_cmyk_in_json() {
        let cmd = SceneCommand::FillRect {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
            paint: Paint::solid(Color::srgb(1, 2, 3, 255)),
        };
        let json = serde_json::to_string(&cmd).expect("serialize");
        assert!(
            !json.contains("cmyk"),
            "sRGB-origin color must not serialize a cmyk key; got: {json}"
        );
    }

    #[test]
    fn cmyk_color_carries_channels_and_serializes() {
        // cmyk(59,85,0,7) → #6124ed (97,36,237).
        let c = Color::cmyk(59.0, 85.0, 0.0, 7.0, 97, 36, 237);
        assert_eq!((c.r, c.g, c.b, c.a), (97, 36, 237, 255));
        assert_eq!(c.cmyk, Some([59.0, 85.0, 0.0, 7.0]));
        let json = serde_json::to_string(&c).expect("serialize");
        assert!(
            json.contains(r#""cmyk":[59.0,85.0,0.0,7.0]"#),
            "got: {json}"
        );
    }

    #[test]
    fn nonseparable_blend_mode_serializes_kebab_case() {
        let cmd = SceneCommand::PushLayer {
            opacity: 1.0,
            blend_mode: Some(BlendMode::Luminosity),
        };
        let json = serde_json::to_string(&cmd).expect("serialize");
        assert!(json.contains(r#""blend_mode":"luminosity""#), "got: {json}");
    }
}
