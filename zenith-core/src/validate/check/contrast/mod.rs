//! WCAG 3 (APCA) contrast advisory check.
//!
//! Compares text-node fills against the colour they visually sit on: the
//! topmost preceding painted backdrop that geometrically covers the text in
//! page coordinates, falling back to the page background. The metric is APCA
//! lightness contrast (`Lc`) with the current WCAG 3 draft thresholds.

mod geometry;
mod props;

use std::collections::BTreeMap;

use crate::ast::node::{
    ImageNode, Node, PathNode, PolygonNode, PolylineNode, ShapeNode, TableNode, TextNode,
};
use crate::ast::style::Style;
use crate::ast::value::{Dimension, PropertyValue};
use crate::color::{apca_lc, parse_rgb};
use crate::diagnostics::Diagnostic;
use crate::tokens::{ResolvedToken, ResolvedValue};

use geometry::{
    CoverageShape, RectPx, Rotation, group_offset, local_box, path_bounds, polygon_region, text_box,
};
use props::{
    candidate_has_effect, clip_bounds, container_is_unmodeled, leaf_rotation, node_opacity,
    node_rotate_deg, node_visible, rect_coverage_shape, resolve_color_property, resolve_font_size,
    resolve_font_weight, style_property,
};

/// Below this APCA magnitude the text is effectively painted into its backdrop,
/// which is a stronger signal than ordinary sub-threshold contrast.
const INVISIBLE_LC_FLOOR: f64 = 15.0;
const MIN_PAINT_ALPHA: f64 = 1.0 / 255.0;

pub(super) fn check_page_text_contrast(
    children: &[Node],
    page_bg_rgb: Option<(u8, u8, u8)>,
    page_size: (f64, f64),
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
    style_map: &BTreeMap<&str, &Style>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut candidates = Vec::new();
    let ctx = PaintCtx {
        dx: 0.0,
        dy: 0.0,
        clip: None,
        opacity: 1.0,
        unmodeled: false,
        page_bg_rgb,
        page_size,
    };
    let env = ContrastEnv {
        resolved_tokens,
        style_map,
    };
    walk_paint(children, ctx, &mut candidates, env, diagnostics);
}

#[derive(Clone, Copy)]
struct PaintCtx {
    dx: f64,
    dy: f64,
    clip: Option<RectPx>,
    opacity: f64,
    /// True when an ancestor `group`/`frame` carries a transform the validator
    /// cannot model geometrically (rotation) or a paint-altering effect
    /// (mask/filter/blur/non-normal blend). Every candidate under it is forced
    /// to an [`BackdropPaint::Indeterminate`] paint.
    unmodeled: bool,
    page_bg_rgb: Option<(u8, u8, u8)>,
    page_size: (f64, f64),
}

struct BackdropCandidate {
    paint: BackdropPaint,
    bounds: RectPx,
    shape: CoverageShape,
    /// Rigid rotation applied to this candidate about its box center, if any.
    rotation: Option<Rotation>,
}

impl BackdropCandidate {
    fn covers(&self, x: f64, y: f64) -> bool {
        let (x, y) = match self.rotation {
            Some(rot) => rot.inverse_map(x, y),
            None => (x, y),
        };
        self.shape.contains_point(self.bounds, x, y)
    }
}

#[derive(Debug)]
enum BackdropPaint {
    Solid(PaintColor),
    Gradient(Vec<PaintColor>),
    Indeterminate,
}

#[derive(Clone, Copy, Debug)]
struct PaintColor {
    rgb: (u8, u8, u8),
    alpha: f64,
}

#[derive(Clone, Copy)]
struct SampledBackdrop {
    rgb: (u8, u8, u8),
    source: &'static str,
}

#[derive(Clone, Copy)]
struct ContrastSample {
    lc: f64,
    source: &'static str,
}

#[derive(Clone, Copy)]
struct ContrastEnv<'a> {
    resolved_tokens: &'a BTreeMap<String, ResolvedToken>,
    style_map: &'a BTreeMap<&'a str, &'a Style>,
}

fn walk_paint(
    children: &[Node],
    ctx: PaintCtx,
    candidates: &mut Vec<BackdropCandidate>,
    env: ContrastEnv<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for node in children {
        if !node_visible(node) {
            continue;
        }
        match node {
            Node::Rect(r) => push_backdrop(
                node,
                &r.fill,
                &r.style,
                rect_coverage_shape(r, ctx.page_size, env.resolved_tokens),
                ctx,
                candidates,
                env,
            ),
            Node::Ellipse(e) => push_backdrop(
                node,
                &e.fill,
                &e.style,
                CoverageShape::Ellipse,
                ctx,
                candidates,
                env,
            ),
            Node::Shape(s) => push_shape_backdrop(node, s, ctx, candidates, env),
            Node::Image(img) => push_image_backdrop(node, img, ctx, candidates, env),
            Node::Polygon(poly) => push_polygon_backdrop(node, poly, ctx, candidates, env),
            Node::Polyline(poly) => push_polyline_backdrop(node, poly, ctx, candidates, env),
            Node::Frame(f) => {
                let frame_box = absolute_box(node, ctx, env.resolved_tokens);
                let frame_clip = frame_box.and_then(|b| clip_bounds(ctx.clip, b));
                let no_fill: Option<PropertyValue> = None;
                push_backdrop(
                    node,
                    &no_fill,
                    &f.style,
                    CoverageShape::Rect,
                    ctx,
                    candidates,
                    env,
                );
                let child_ctx = PaintCtx {
                    clip: frame_clip,
                    opacity: cascaded_opacity(ctx.opacity, f.opacity),
                    unmodeled: ctx.unmodeled || container_is_unmodeled(node),
                    ..ctx
                };
                walk_paint(&f.children, child_ctx, candidates, env, diagnostics);
            }
            Node::Group(g) => {
                let (gx, gy) = group_offset(
                    g.x.as_ref(),
                    g.y.as_ref(),
                    ctx.page_size,
                    env.resolved_tokens,
                );
                let child_ctx = PaintCtx {
                    dx: ctx.dx + gx,
                    dy: ctx.dy + gy,
                    opacity: cascaded_opacity(ctx.opacity, g.opacity),
                    unmodeled: ctx.unmodeled || container_is_unmodeled(node),
                    ..ctx
                };
                walk_paint(&g.children, child_ctx, candidates, env, diagnostics);
            }
            Node::Text(t) => check_text_node(t, ctx, candidates, env, diagnostics),
            Node::Table(t) => {
                check_table_text_contrast(t, ctx.page_bg_rgb, ctx.page_size, env, diagnostics)
            }
            Node::Path(p) => push_path_backdrop(node, p, ctx, candidates, env),
            Node::Line(_)
            | Node::Code(_)
            | Node::Instance(_)
            | Node::Field(_)
            | Node::Footnote(_)
            | Node::Toc(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Unknown(_) => {}
        }
    }
}

fn push_shape_backdrop(
    node: &Node,
    shape: &ShapeNode,
    ctx: PaintCtx,
    candidates: &mut Vec<BackdropCandidate>,
    env: ContrastEnv<'_>,
) {
    let coverage = match shape.kind.as_deref() {
        Some("decision") => CoverageShape::Diamond,
        Some("terminator") => CoverageShape::Capsule,
        Some("ellipse") => CoverageShape::Ellipse,
        Some("process") | None => CoverageShape::Rect,
        _ => CoverageShape::Rect,
    };
    push_backdrop(
        node,
        &shape.fill,
        &shape.style,
        coverage,
        ctx,
        candidates,
        env,
    );
}

fn push_polygon_backdrop(
    node: &Node,
    polygon: &PolygonNode,
    ctx: PaintCtx,
    candidates: &mut Vec<BackdropCandidate>,
    env: ContrastEnv<'_>,
) {
    push_point_backdrop(
        node,
        &polygon.fill,
        &polygon.style,
        &polygon.points,
        ctx,
        candidates,
        env,
    );
}

fn push_polyline_backdrop(
    node: &Node,
    polyline: &PolylineNode,
    ctx: PaintCtx,
    candidates: &mut Vec<BackdropCandidate>,
    env: ContrastEnv<'_>,
) {
    push_point_backdrop(
        node,
        &polyline.fill,
        &polyline.style,
        &polyline.points,
        ctx,
        candidates,
        env,
    );
}

fn push_point_backdrop(
    node: &Node,
    fill: &Option<PropertyValue>,
    style: &Option<String>,
    points: &[crate::ast::node::Point],
    ctx: PaintCtx,
    candidates: &mut Vec<BackdropCandidate>,
    env: ContrastEnv<'_>,
) {
    let opacity = ctx.opacity * node_opacity(node).unwrap_or(1.0);
    let Some(paint) = resolve_fill_paint(
        fill,
        style.as_deref(),
        env.style_map,
        env.resolved_tokens,
        opacity,
    ) else {
        return;
    };
    let Some((bounds, shape)) = polygon_region(points, ctx.dx, ctx.dy, ctx.page_size) else {
        return;
    };
    // A rotated polygon/polyline pivots on its centroid box (not its bounding-box
    // center), which the validator does not replicate — so any rotation makes the
    // backdrop indeterminate rather than silently mis-testing containment.
    let paint = if ctx.unmodeled || node_rotate_deg(node).is_some() {
        BackdropPaint::Indeterminate
    } else {
        paint
    };
    if let Some(bounds) = clip_bounds(ctx.clip, bounds) {
        candidates.push(BackdropCandidate {
            paint,
            bounds,
            shape,
            rotation: None,
        });
    }
}

fn push_path_backdrop(
    node: &Node,
    path: &PathNode,
    ctx: PaintCtx,
    candidates: &mut Vec<BackdropCandidate>,
    env: ContrastEnv<'_>,
) {
    // A `path` fill is only ever a conservative INDETERMINATE backdrop: its exact
    // Bezier coverage is not modeled, so we advertise its bounding box as an
    // unsampled region rather than approximating it as a solid fill (which would
    // over-cover) or ignoring it (which would silently pass invisible text).
    let opacity = ctx.opacity * node_opacity(node).unwrap_or(1.0);
    if resolve_fill_paint(
        &path.fill,
        path.style.as_deref(),
        env.style_map,
        env.resolved_tokens,
        opacity,
    )
    .is_none()
    {
        return;
    }
    let anchors: Vec<(Option<Dimension>, Option<Dimension>)> = path
        .effective_subpaths()
        .flat_map(|sub| sub.anchors.iter())
        .map(|a| (a.x.clone(), a.y.clone()))
        .collect();
    let Some(bounds) = path_bounds(&anchors, ctx.dx, ctx.dy, ctx.page_size) else {
        return;
    };
    if let Some(bounds) = clip_bounds(ctx.clip, bounds) {
        candidates.push(BackdropCandidate {
            paint: BackdropPaint::Indeterminate,
            bounds,
            shape: CoverageShape::Rect,
            rotation: None,
        });
    }
}

fn push_backdrop(
    node: &Node,
    fill: &Option<PropertyValue>,
    style: &Option<String>,
    shape: CoverageShape,
    ctx: PaintCtx,
    candidates: &mut Vec<BackdropCandidate>,
    env: ContrastEnv<'_>,
) {
    let opacity = ctx.opacity * node_opacity(node).unwrap_or(1.0);
    let Some(paint) = resolve_fill_paint(
        fill,
        style.as_deref(),
        env.style_map,
        env.resolved_tokens,
        opacity,
    ) else {
        return;
    };
    let Some(bounds) = absolute_box(node, ctx, env.resolved_tokens) else {
        return;
    };
    // A paint-altering effect on the leaf itself (mask/filter/blur/non-normal
    // blend), or an unmodeled ancestor transform, makes the composited colour
    // un-sampleable — downgrade to indeterminate rather than trust the raw fill.
    let paint = if ctx.unmodeled || candidate_has_effect(node) {
        BackdropPaint::Indeterminate
    } else {
        paint
    };
    // A rotation on the leaf is modeled EXACTLY: the renderer rotates it about
    // its own box center, so containment is tested by inverse-rotating samples.
    let rotation = leaf_rotation(node, bounds);
    if let Some(clipped) = clip_bounds(ctx.clip, bounds) {
        candidates.push(BackdropCandidate {
            paint,
            bounds: clipped,
            shape,
            rotation,
        });
    }
}

fn push_image_backdrop(
    node: &Node,
    _image: &ImageNode,
    ctx: PaintCtx,
    candidates: &mut Vec<BackdropCandidate>,
    env: ContrastEnv<'_>,
) {
    if ctx.opacity * node_opacity(node).unwrap_or(1.0) < MIN_PAINT_ALPHA {
        return;
    }
    let Some(bounds) = absolute_box(node, ctx, env.resolved_tokens) else {
        return;
    };
    let rotation = leaf_rotation(node, bounds);
    if let Some(clipped) = clip_bounds(ctx.clip, bounds) {
        candidates.push(BackdropCandidate {
            paint: BackdropPaint::Indeterminate,
            bounds: clipped,
            shape: CoverageShape::Rect,
            rotation,
        });
    }
}

fn absolute_box(
    node: &Node,
    ctx: PaintCtx,
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
) -> Option<RectPx> {
    local_box(node, ctx.page_size, resolved_tokens).map(|b| b.translated(ctx.dx, ctx.dy))
}

fn check_text_node(
    text: &TextNode,
    ctx: PaintCtx,
    candidates: &[BackdropCandidate],
    env: ContrastEnv<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(fg_rgb) = resolve_color_property(
        text.fill
            .as_ref()
            .or_else(|| style_property(text.style.as_deref(), "fill", env.style_map)),
        env.resolved_tokens,
    ) else {
        return;
    };

    let size_px = resolve_font_size(text, env.style_map, env.resolved_tokens);
    let weight = resolve_font_weight(text, env.style_map, env.resolved_tokens);
    let is_large = size_px >= 24.0 || (size_px >= 18.66 && weight >= 700);
    let threshold = if is_large { 45.0_f64 } else { 60.0_f64 };

    let hint_rgb = resolve_color_property(text.contrast_bg.as_ref(), env.resolved_tokens);
    let mut backdrop_samples = Vec::new();
    if hint_rgb.is_none() {
        // The text sample box must live in ABSOLUTE page space, translated by the
        // accumulated ancestor offset, so it lands on the same coordinates as the
        // (already-absolute) backdrop candidates and frame clip.
        let Some(text_bbox) = text_box(text, ctx.page_size, env.resolved_tokens)
            .map(|b| b.translated(ctx.dx, ctx.dy))
        else {
            // No resolvable box (e.g. anchored text with no authored w/h). We
            // cannot compute its extent without font metrics, so rather than
            // silently judging it against the page background we flag it honestly.
            if text_has_position(text) {
                push_indeterminate_extent(text, diagnostics);
            }
            return;
        };
        let (samples, indeterminate_backdrop) =
            collect_backdrop_samples(text_bbox, ctx.clip, candidates, ctx.page_bg_rgb);
        backdrop_samples = samples;
        if indeterminate_backdrop {
            diagnostics.push(Diagnostic::advisory(
                "contrast.indeterminate_backdrop",
                format!(
                    "text '{}': its backdrop includes an unsampled paint (image, path, or a rotated/masked/blurred/blended fill) and cannot be sampled during validation; add a contrast-bg hint",
                    text.id
                ),
                text.source_span,
                Some(text.id.clone()),
            ));
        }
    }

    let best = select_contrast_sample(fg_rgb, hint_rgb, &backdrop_samples, ctx.page_bg_rgb);
    if let Some(sample) = best
        && sample.lc < threshold
    {
        emit_contrast_diagnostic(text, sample, threshold, diagnostics);
    }
}

/// Emit the appropriate sub-threshold contrast diagnostic. `contrast.invisible`
/// (a strong Warning signal) fires when the text is effectively painted into its
/// backdrop; the softer, suppressible `contrast.low` is an Advisory.
fn emit_contrast_diagnostic(
    text: &TextNode,
    sample: ContrastSample,
    threshold: f64,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if sample.lc < INVISIBLE_LC_FLOOR {
        diagnostics.push(Diagnostic::warning(
            "contrast.invisible",
            format!(
                "text '{}': APCA contrast Lc {:.1} is effectively invisible against {} (Lc below {:.0})",
                text.id, sample.lc, sample.source, INVISIBLE_LC_FLOOR
            ),
            text.source_span,
            Some(text.id.clone()),
        ));
    } else {
        diagnostics.push(Diagnostic::advisory(
            "contrast.low",
            format!(
                "text '{}': APCA contrast Lc {:.1} against {} is below the WCAG 3 draft threshold (Lc {:.0})",
                text.id, sample.lc, sample.source, threshold
            ),
            text.source_span,
            Some(text.id.clone()),
        ));
    }
}

/// Advisory for a text node with a resolvable position and fill but no
/// computable box (its extent needs font metrics unavailable at validation).
fn push_indeterminate_extent(text: &TextNode, diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.push(Diagnostic::advisory(
        "contrast.indeterminate_backdrop",
        format!(
            "text '{}': its extent (width/height) is unknown during validation, so the backdrop it sits on cannot be sampled; add a contrast-bg hint",
            text.id
        ),
        text.source_span,
        Some(text.id.clone()),
    ));
}

/// Whether a text node carries enough placement to be positioned on the page
/// (an explicit x/y or a page anchor), even when its extent is unknown.
fn text_has_position(text: &TextNode) -> bool {
    text.x.is_some() || text.y.is_some() || text.anchor.is_some()
}

fn collect_backdrop_samples(
    text_box: RectPx,
    clip: Option<RectPx>,
    candidates: &[BackdropCandidate],
    page_bg_rgb: Option<(u8, u8, u8)>,
) -> (Vec<SampledBackdrop>, bool) {
    let mut backdrops = Vec::with_capacity(5);
    let mut indeterminate = false;
    if let Some(clip) = clip
        && !clip.contains_rect(text_box)
    {
        return (backdrops, indeterminate);
    }
    for (x, y) in text_box.sample_points() {
        let mut point_indeterminate = false;
        let mut samples: Vec<SampledBackdrop> = page_bg_rgb
            .into_iter()
            .map(|rgb| SampledBackdrop {
                rgb,
                source: "page background",
            })
            .collect();
        for candidate in candidates {
            if candidate.covers(x, y) {
                match &candidate.paint {
                    BackdropPaint::Solid(color) => {
                        samples = composite_solid_samples(&samples, *color);
                        if color.alpha >= 1.0 {
                            point_indeterminate = false;
                        }
                    }
                    BackdropPaint::Gradient(stops) => {
                        samples = composite_gradient_samples(&samples, stops);
                        if stops.iter().all(|stop| stop.alpha >= 1.0) {
                            point_indeterminate = false;
                        }
                    }
                    BackdropPaint::Indeterminate => {
                        point_indeterminate = true;
                    }
                }
            }
        }
        if point_indeterminate {
            indeterminate = true;
        }
        for sample in samples {
            push_unique_sample(&mut backdrops, sample);
        }
    }
    (backdrops, indeterminate)
}

fn composite_solid_samples(samples: &[SampledBackdrop], paint: PaintColor) -> Vec<SampledBackdrop> {
    if samples.is_empty() {
        return vec![SampledBackdrop {
            rgb: paint.rgb,
            source: "backdrop",
        }];
    }
    samples
        .iter()
        .map(|sample| SampledBackdrop {
            rgb: composite_rgb(paint.rgb, paint.alpha, sample.rgb),
            source: "backdrop",
        })
        .collect()
}

fn composite_gradient_samples(
    samples: &[SampledBackdrop],
    stops: &[PaintColor],
) -> Vec<SampledBackdrop> {
    if stops.is_empty() {
        return samples.to_vec();
    }
    let mut composited = Vec::with_capacity(stops.len() * samples.len().max(1));
    for stop in stops {
        if samples.is_empty() {
            composited.push(SampledBackdrop {
                rgb: stop.rgb,
                source: "backdrop",
            });
        } else {
            composited.extend(samples.iter().map(|sample| SampledBackdrop {
                rgb: composite_rgb(stop.rgb, stop.alpha, sample.rgb),
                source: "backdrop",
            }));
        }
    }
    composited
}

fn composite_rgb(src: (u8, u8, u8), alpha: f64, dst: (u8, u8, u8)) -> (u8, u8, u8) {
    let alpha = alpha.clamp(0.0, 1.0);
    (
        composite_channel(src.0, alpha, dst.0),
        composite_channel(src.1, alpha, dst.1),
        composite_channel(src.2, alpha, dst.2),
    )
}

fn composite_channel(src: u8, alpha: f64, dst: u8) -> u8 {
    ((src as f64 * alpha) + (dst as f64 * (1.0 - alpha))).round() as u8
}

fn push_unique_sample(backdrops: &mut Vec<SampledBackdrop>, sample: SampledBackdrop) {
    if !backdrops
        .iter()
        .any(|backdrop| backdrop.rgb == sample.rgb && backdrop.source == sample.source)
    {
        backdrops.push(sample);
    }
}

fn select_contrast_sample(
    fg_rgb: (u8, u8, u8),
    hint_rgb: Option<(u8, u8, u8)>,
    sampled_backdrops: &[SampledBackdrop],
    page_bg_rgb: Option<(u8, u8, u8)>,
) -> Option<ContrastSample> {
    if let Some(rgb) = hint_rgb {
        return Some(ContrastSample {
            lc: apca_lc(fg_rgb, rgb).abs(),
            source: "contrast-bg hint",
        });
    }
    if !sampled_backdrops.is_empty() {
        let mut worst: Option<ContrastSample> = None;
        for backdrop in sampled_backdrops {
            let sample = ContrastSample {
                lc: apca_lc(fg_rgb, backdrop.rgb).abs(),
                source: backdrop.source,
            };
            if worst.is_none_or(|w| sample.lc < w.lc) {
                worst = Some(sample);
            }
        }
        return worst;
    }
    page_bg_rgb.map(|rgb| ContrastSample {
        lc: apca_lc(fg_rgb, rgb).abs(),
        source: "page background",
    })
}

fn check_table_text_contrast(
    table: &TableNode,
    page_bg_rgb: Option<(u8, u8, u8)>,
    page_size: (f64, f64),
    env: ContrastEnv<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let header_rows = table.header_rows.unwrap_or(0);
    let resolve_fill = |pv: &Option<PropertyValue>| -> Option<(u8, u8, u8)> {
        resolve_fill_paint(pv, None, env.style_map, env.resolved_tokens, 1.0)?.as_solid_rgb()
    };

    for (row_idx, row) in table.rows.iter().enumerate() {
        let is_header = (row_idx as u32) < header_rows;
        for cell in &row.cells {
            let cell_bg = if let Some(rgb) = resolve_fill(&cell.fill) {
                Some(rgb)
            } else if is_header {
                resolve_fill(&table.header_fill)
                    .or_else(|| resolve_fill(&table.fill))
                    .or(page_bg_rgb)
            } else {
                resolve_fill(&table.fill).or(page_bg_rgb)
            };
            let ctx = PaintCtx {
                dx: 0.0,
                dy: 0.0,
                clip: None,
                opacity: 1.0,
                unmodeled: false,
                page_bg_rgb: cell_bg,
                page_size,
            };
            let mut candidates = Vec::new();
            walk_paint(&cell.children, ctx, &mut candidates, env, diagnostics);
        }
    }
}

fn resolve_fill_paint(
    fill: &Option<PropertyValue>,
    style: Option<&str>,
    style_map: &BTreeMap<&str, &Style>,
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
    opacity: f64,
) -> Option<BackdropPaint> {
    let pv = fill
        .as_ref()
        .or_else(|| style_property(style, "fill", style_map))?;
    let PropertyValue::TokenRef(id) = pv else {
        return None;
    };
    let token = resolved_tokens.get(id.as_str())?;
    match &token.value {
        ResolvedValue::Color(hex) => solid_paint_from_hex(hex, opacity),
        ResolvedValue::CmykColor { hex, .. } => solid_paint_from_hex(hex, opacity),
        ResolvedValue::Gradient(gradient) => {
            let stops: Vec<PaintColor> = gradient
                .stops
                .iter()
                .filter_map(|(_, color_id)| {
                    resolved_tokens
                        .get(color_id.as_str())
                        .and_then(|token| resolved_color_paint(token, opacity))
                })
                .collect();
            if stops.is_empty() {
                None
            } else {
                Some(BackdropPaint::Gradient(stops))
            }
        }
        ResolvedValue::Dimension(_)
        | ResolvedValue::Number(_)
        | ResolvedValue::FontFamily(_)
        | ResolvedValue::FontWeight(_)
        | ResolvedValue::Shadow(_)
        | ResolvedValue::Filter(_)
        | ResolvedValue::Mask(_) => None,
    }
}

impl BackdropPaint {
    fn as_solid_rgb(&self) -> Option<(u8, u8, u8)> {
        match self {
            BackdropPaint::Solid(color) if color.alpha >= 1.0 => Some(color.rgb),
            BackdropPaint::Solid(_) => None,
            BackdropPaint::Gradient(_) | BackdropPaint::Indeterminate => None,
        }
    }
}

fn solid_paint_from_hex(hex: &str, opacity: f64) -> Option<BackdropPaint> {
    parse_paint_color(hex, opacity).map(BackdropPaint::Solid)
}

fn resolved_color_paint(token: &ResolvedToken, opacity: f64) -> Option<PaintColor> {
    match &token.value {
        ResolvedValue::Color(hex) => parse_paint_color(hex, opacity),
        ResolvedValue::CmykColor { hex, .. } => parse_paint_color(hex, opacity),
        ResolvedValue::Dimension(_)
        | ResolvedValue::Number(_)
        | ResolvedValue::FontFamily(_)
        | ResolvedValue::FontWeight(_)
        | ResolvedValue::Gradient(_)
        | ResolvedValue::Shadow(_)
        | ResolvedValue::Filter(_)
        | ResolvedValue::Mask(_) => None,
    }
}

fn parse_paint_color(hex: &str, opacity: f64) -> Option<PaintColor> {
    let rgb = parse_rgb(hex)?;
    let token_alpha = hex
        .strip_prefix('#')
        .filter(|h| h.len() == 8)
        .and_then(|h| u8::from_str_radix(&h[6..8], 16).ok())
        .unwrap_or(255) as f64
        / 255.0;
    let alpha = (token_alpha * opacity.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    if alpha < MIN_PAINT_ALPHA {
        return None;
    }
    Some(PaintColor { rgb, alpha })
}

fn cascaded_opacity(parent: f64, opacity: Option<f64>) -> f64 {
    parent * opacity.unwrap_or(1.0).clamp(0.0, 1.0)
}
