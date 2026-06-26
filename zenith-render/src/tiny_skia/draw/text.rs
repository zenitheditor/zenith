//! Glyph-run rasterization: resolve the font face, build a clip mask once per
//! run, then blit color-bitmap glyphs or fill (and optionally stroke) outline
//! glyphs. Pulls fields from [`SceneCommand::DrawGlyphRun`] and draws into
//! `target` under the shared [`DrawCtx`]; byte-identical to the prior arm.

use tiny_skia::{FillRule, FilterQuality, Paint, Pixmap, PixmapPaint, Stroke, Transform};
use zenith_core::FontProvider;
use zenith_scene::SceneCommand;

use super::super::commands::DrawCtx;
use super::super::paths::{GlyphOutlinePen, clip_mask};

pub(in crate::tiny_skia) fn draw_glyph_run(
    target: &mut Pixmap,
    ctx: DrawCtx,
    cmd: &SceneCommand,
    fonts: &dyn FontProvider,
) {
    let SceneCommand::DrawGlyphRun {
        x,
        y,
        font_id,
        font_size,
        color,
        stroke_color,
        stroke_width,
        // The raster backend has no clickable-link or text-extraction concept;
        // both are PDF-only and render-identical here.
        link: _,
        selectable: _,
        glyphs,
    } = cmd
    else {
        return;
    };
    // ── 1. Resolve font bytes ─────────────────────────────────
    let font_data = match fonts.by_id(font_id) {
        Some(fd) => fd,
        None => {
            // Unknown font id: skip the run silently. The page
            // renders correctly for all other commands.
            return;
        }
    };

    // ── 2. Parse the font face ────────────────────────────────
    let face = match ttf_parser::Face::parse(&font_data.bytes, font_data.index) {
        Ok(f) => f,
        Err(_) => return, // malformed font bytes: skip run
    };

    // ── 3. Compute scale from font units to pixels ────────────
    let units_per_em = face.units_per_em();
    if units_per_em == 0 {
        return; // degenerate font: skip
    }
    let scale = font_size / f32::from(units_per_em);

    // ── 4. Build the paint for the glyph color ────────────────
    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, color.a);
    // AA is on for glyphs: curved outlines need sub-pixel coverage.
    // tiny-skia AA is pure-software; output is deterministic on
    // the same machine (no GPU, no random state).
    paint.anti_alias = true;

    // ── 5. Build the clip mask (once per run) ─────────────────
    // Glyph ink is clipped to the effective clip via the mask, so
    // text inside a frame is truncated at the frame edge; deterministic
    // same-machine (pure-software AA, no GPU).
    let effective_clip = ctx.effective_clip;
    let mask = match clip_mask(effective_clip, ctx.width, ctx.height) {
        None => return, // entire run is off-canvas / clip is empty
        Some(m) => m,
    };

    // ── 6. Rasterize each glyph ───────────────────────────────
    for glyph in glyphs {
        let origin_x = *x as f32 + glyph.dx;
        let baseline_y = *y as f32 + glyph.dy;

        // ── 6a. Color-bitmap path (CBDT/sbix emoji) ───────────
        // If the font supplies an embedded PNG raster image for
        // this glyph, blit it instead of an outline. Outline
        // (monochrome) fonts never return Some here, so this
        // branch is inert for them → byte-identical output.
        if let Some(img) =
            face.glyph_raster_image(ttf_parser::GlyphId(glyph.glyph_id), *font_size as u16)
            && img.format == ttf_parser::RasterImageFormat::PNG
            && img.pixels_per_em > 0
            && let Ok(decoded) = Pixmap::decode_png(img.data)
        {
            // Strike ppem → target ppem scale.
            let s = *font_size / f32::from(img.pixels_per_em);
            // ttf-parser's `img.y` is the offset of the image's
            // BOTTOM from the baseline (positive up); the image
            // top in baseline space is therefore:
            //   baseline_y - (img.y + img.height) * s
            let draw_x = origin_x + f32::from(img.x) * s;
            let draw_y = baseline_y - (f32::from(img.y) + f32::from(img.height)) * s;
            // Compose the rotation stack on top of the per-glyph
            // scale+translate. Identity case → emoji_ts == fit,
            // matching the DrawImage arm's pattern.
            let emoji_fit = Transform::from_row(s, 0.0, 0.0, s, draw_x, draw_y);
            let emoji_ts = ctx.current_ts.pre_concat(emoji_fit);
            let emoji_paint = PixmapPaint {
                quality: FilterQuality::Bilinear,
                ..Default::default()
            };
            target.draw_pixmap(
                0,
                0,
                decoded.as_ref(),
                &emoji_paint,
                emoji_ts,
                mask.as_ref(),
            );
            continue;
        }

        // Build path via outline pen.
        let mut pen = GlyphOutlinePen::new(origin_x, baseline_y, scale);

        // outline_glyph returns None for glyphs with no outlines
        // (e.g. space, .notdef in some fonts). Skip those.
        if face
            .outline_glyph(ttf_parser::GlyphId(glyph.glyph_id), &mut pen)
            .is_none()
        {
            continue;
        }

        // Finalise the path; None means an empty or degenerate path.
        let path = match pen.builder.finish() {
            Some(p) => p,
            None => continue,
        };

        target.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            ctx.current_ts,
            mask.as_ref(),
        );

        // Stroke pass — outline on top of fill. Skipped when no
        // stroke_color/stroke_width is set (byte-identical to before).
        if let (Some(sc), Some(sw)) = (stroke_color, stroke_width)
            && *sw > 0.0
        {
            let mut spaint = Paint::default();
            spaint.set_color_rgba8(sc.r, sc.g, sc.b, sc.a);
            spaint.anti_alias = true;
            let sstroke = Stroke {
                width: *sw as f32,
                ..Default::default()
            };
            target.stroke_path(&path, &spaint, &sstroke, ctx.current_ts, mask.as_ref());
        }
    }
}
