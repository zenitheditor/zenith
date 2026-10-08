//! `DrawGlyphRun` → PDF emission: real selectable text or filled outlines.
//!
//! A selectable run whose font was embedded (see [`super::font`]) emits real
//! text with per-glyph text matrices and 2-byte CID shows
//! (Identity-H) — plus a clickable `/Link` annotation when it carries a link.
//! Everything else (a `selectable=false` run, or a font that failed to embed)
//! falls back to filled glyph outlines. Optional strokes follow the fill.
//! Runs without strokes retain historical output bytes.

use pdf_writer::{Content, Str};
use zenith_core::FontProvider;
use zenith_scene::Color;

use super::color;
use super::content::{FONT_PREFIX, LinkAnnot, PageResources, apply_alpha, name};
use super::font::FontPlan;
use super::geometry::GlyphPen;

/// Borrow/scalar context for one [`zenith_scene::SceneCommand::DrawGlyphRun`]
/// emission, bundled into a `Copy` struct so [`emit_glyph_run`] stays within the
/// argument-count budget without an `#[allow]`.
#[derive(Clone, Copy)]
pub(super) struct GlyphRun<'a> {
    /// Text-box origin x in pixels.
    pub(super) x: f64,
    /// Baseline y in pixels.
    pub(super) y: f64,
    /// Stable font-face identifier; resolved via `FontProvider::by_id`.
    pub(super) font_id: &'a str,
    /// Font size at which glyphs were shaped, in pixels.
    pub(super) font_size: f32,
    /// Fill color of the glyph run.
    pub(super) color: &'a Color,
    /// Optional outline color.
    pub(super) stroke_color: Option<&'a Color>,
    /// Outline width in scene pixels.
    pub(super) stroke_width: Option<f64>,
    /// Optional hyperlink URL; emitted as a `/Link` annotation when the run is
    /// selectable and the run's font is embedded.
    pub(super) link: Option<&'a str>,
    /// Whether the run is emitted as real text (`true`) or filled outlines.
    pub(super) selectable: bool,
    /// Positioned glyphs, baseline-relative.
    pub(super) glyphs: &'a [zenith_scene::SceneGlyph],
}

pub(super) fn emit_glyph_run(
    content: &mut Content,
    res: &mut PageResources,
    fonts: &dyn FontProvider,
    font_plan: &FontPlan,
    run: GlyphRun<'_>,
) {
    let Some(font_data) = fonts.by_id(run.font_id) else {
        return;
    };
    let Ok(face) = ttf_parser::Face::parse(&font_data.bytes, font_data.index) else {
        return;
    };
    let units_per_em = face.units_per_em();
    if units_per_em == 0 {
        return;
    }

    // Selectable runs whose font was successfully embedded emit real text (with a
    // clickable link annotation when the run carries one); everything else — a
    // `selectable=false` run, or a font that failed to embed — falls back to the
    // filled-outline path, which is byte-identical to the historical output.
    if run.selectable && font_plan.resource_index(run.font_id).is_some() {
        emit_text_run(content, res, font_plan, &face, run);
        return;
    }
    emit_outline_run(content, res, &face, units_per_em, run);
}

/// Emit selectable text and record one link annotation for the run.
/// Stroked runs fill and stroke each glyph in raster paint order.
/// Unstroked runs retain the historical text stream.
fn emit_text_run(
    content: &mut Content,
    res: &mut PageResources,
    font_plan: &FontPlan,
    face: &ttf_parser::Face<'_>,
    run: GlyphRun<'_>,
) {
    let GlyphRun {
        x,
        y,
        font_id,
        font_size,
        link,
        glyphs,
        ..
    } = run;
    let Some(font_idx) = font_plan.resource_index(font_id) else {
        return;
    };
    let units_per_em = face.units_per_em();
    let scale = font_size / f32::from(units_per_em);
    res.font_indices.insert(font_idx);

    if glyph_stroke(run).is_some() {
        for glyph in run.glyphs {
            let single = GlyphRun {
                glyphs: std::slice::from_ref(glyph),
                ..run
            };
            emit_text_fill(content, res, font_plan, font_idx, single);
            emit_stroke_run(content, res, face, scale, single);
        }
    } else {
        emit_text_fill(content, res, font_plan, font_idx, run);
    }

    // Link annotation over the run's glyph bounds (scene coords, y-down).
    if let Some(url) = link
        && !glyphs.is_empty()
    {
        let ascent = f32::from(face.ascender()) * scale;
        let descent = f32::from(face.descender()) * scale; // negative
        let mut left = f32::INFINITY;
        let mut right = f32::NEG_INFINITY;
        for glyph in glyphs {
            let adv = face
                .glyph_hor_advance(ttf_parser::GlyphId(glyph.glyph_id))
                .map_or(0.0, |a| f32::from(a) * scale);
            left = left.min(glyph.dx);
            right = right.max(glyph.dx + adv);
        }
        if left.is_finite() && right.is_finite() {
            res.links.push(LinkAnnot {
                x0: x + f64::from(left),
                y0: y - f64::from(ascent),
                x1: x + f64::from(right),
                y1: y - f64::from(descent),
                url: url.to_owned(),
            });
        }
    }
}

/// Emit a glyph run as filled vector outlines (the `selectable=false` path and
/// the embed-failure fallback). Optional strokes follow the fill.
fn emit_outline_run(
    content: &mut Content,
    res: &mut PageResources,
    face: &ttf_parser::Face<'_>,
    units_per_em: u16,
    run: GlyphRun<'_>,
) {
    let scale = run.font_size / f32::from(units_per_em);

    if glyph_stroke(run).is_some() {
        for glyph in run.glyphs {
            let single = GlyphRun {
                glyphs: std::slice::from_ref(glyph),
                ..run
            };
            emit_outline_fill(content, res, face, scale, single);
            emit_stroke_run(content, res, face, scale, single);
        }
    } else {
        emit_outline_fill(content, res, face, scale, run);
    }
}

fn emit_stroke_run(
    content: &mut Content,
    res: &mut PageResources,
    face: &ttf_parser::Face<'_>,
    scale: f32,
    run: GlyphRun<'_>,
) {
    if let Some((stroke, width)) = glyph_stroke(run) {
        content.save_state();
        apply_alpha(content, res, stroke);
        color::set_stroke(content, stroke);
        content.set_line_width(width);
        content.set_miter_limit(tiny_skia::Stroke::default().miter_limit);
        if emit_outlines(content, face, scale, run) {
            content.stroke();
        } else {
            content.end_path();
        }
        content.restore_state();
    }
}

fn glyph_stroke(run: GlyphRun<'_>) -> Option<(&Color, f32)> {
    let color = run.stroke_color?;
    let width = run.stroke_width?;
    if !width.is_finite() || width <= 0.0 || width > f64::from(f32::MAX) {
        return None;
    }
    let width = width as f32;
    if width == 0.0 {
        return None;
    }
    Some((color, width))
}

fn emit_positioned_cids(content: &mut Content, font_plan: &FontPlan, run: GlyphRun<'_>) {
    let GlyphRun {
        x,
        y,
        font_id,
        glyphs,
        ..
    } = run;
    for glyph in glyphs {
        let Some((_, cid)) = font_plan.cid_of(font_id, glyph.glyph_id) else {
            // A glyph not in the embedded subset (rare): skip its text — the
            // outline path would have drawn it, but a selectable run trades that
            // for extractable text. Missing-glyph runs are a documented edge.
            continue;
        };
        let tx = x as f32 + glyph.dx;
        let ty = y as f32 + glyph.dy;
        // Tm = [1 0 0 -1 tx ty]: the -1 cancels the page's outer y-flip so glyphs
        // sit upright; font_size scaling comes from `set_font`.
        content.set_text_matrix([1.0, 0.0, 0.0, -1.0, tx, ty]);
        content.show(Str(&[(cid >> 8) as u8, (cid & 0xFF) as u8]));
    }
}

fn emit_outlines(
    content: &mut Content,
    face: &ttf_parser::Face<'_>,
    scale: f32,
    run: GlyphRun<'_>,
) -> bool {
    let GlyphRun {
        x,
        y,
        font_size,
        glyphs,
        ..
    } = run;
    // Build one combined path of all glyph outlines, then a single fill. Color
    // bitmap (emoji) glyphs would return Some from `glyph_raster_image`; for PDF
    // v0 they are skipped (documented). Outline fonts never hit that branch.
    let mut any = false;
    for glyph in glyphs {
        if face
            .glyph_raster_image(ttf_parser::GlyphId(glyph.glyph_id), font_size as u16)
            .is_some()
        {
            // Color-bitmap emoji: omitted in PDF v0 (no scenario uses emoji).
            continue;
        }
        let origin_x = x as f32 + glyph.dx;
        let baseline_y = y as f32 + glyph.dy;
        let mut pen = GlyphPen::new(content, origin_x, baseline_y, scale);
        if face
            .outline_glyph(ttf_parser::GlyphId(glyph.glyph_id), &mut pen)
            .is_some()
        {
            any = true;
        }
    }
    any
}

fn emit_text_fill(
    content: &mut Content,
    res: &mut PageResources,
    font_plan: &FontPlan,
    font_idx: usize,
    run: GlyphRun<'_>,
) {
    let GlyphRun {
        color, font_size, ..
    } = run;
    content.save_state();
    apply_alpha(content, res, color);
    color::set_fill(content, color);
    content.begin_text();
    content.set_font(name(FONT_PREFIX, font_idx).as_name(), font_size);
    emit_positioned_cids(content, font_plan, run);
    content.end_text();
    content.restore_state();
}

fn emit_outline_fill(
    content: &mut Content,
    res: &mut PageResources,
    face: &ttf_parser::Face<'_>,
    scale: f32,
    run: GlyphRun<'_>,
) {
    let color = run.color;
    content.save_state();
    apply_alpha(content, res, color);
    color::set_fill(content, color);

    let any = emit_outlines(content, face, scale, run);
    if any {
        content.fill_nonzero();
    } else {
        content.end_path();
    }
    content.restore_state();
}
