//! The WRAP dispatcher (`emit_wrap_path`) and its drop-cap / text-runaround /
//! plain-wrap emit sub-paths. Lifted verbatim out of `compile_text_sized`; the
//! emitted command stream is byte-identical to before.

use std::collections::BTreeMap;

use zenith_core::{Diagnostic, PropertyValue, ResolvedToken, TextNode, dim_to_px};
use zenith_layout::{FontFeature, ShapeRequest, TextDirection, TextLayoutEngine, ZenithGlyphRun};

use crate::compile::paint::resolve_property_color;
use crate::compile::text::baseline::{baseline_grid_snap_failed_diag, snap_to_baseline_grid};
use crate::compile::text::ctx::{EmitStyle, NodeShape, ShapeEnv};
use crate::compile::text::dropcap::{
    DROPCAP_GAP_FACTOR, DropCap, DropCapInitial, drop_cap_font_size, shape_drop_cap,
    take_drop_cap_initial,
};
use crate::compile::text::emit::{emit_lines, emit_lines_profiled};
use crate::compile::text::hyphen::{HyphenationContext, en_us_hyphenator};
use crate::compile::text::pack::{
    Line, LineMetrics, WidthProfile, pack_lines_core, pack_lines_reporting, pack_lines_runaround,
    pack_lines_variable,
};
use crate::compile::text::shape::{
    ResolvedSpan, WordMetrics, WordToken, resolve_font_weight, run_to_scene_glyphs, shape_words,
};
use crate::ir::{Color, SceneCommand};

use super::types::{WrapEnv, WrapGeom, WrapOutcome};

/// Run the single-box WRAP path: convert the resolved spans to word tokens, pick
/// the drop-cap / runaround / plain-wrap sub-path, emit the glyph draws into
/// `commands`, and return the laid-out line count for the overflow checks
/// (plus the grid-snapped content height, when a baseline grid applies).
pub(in crate::compile) fn emit_wrap_path(
    text: &TextNode,
    mut resolved_spans: Vec<ResolvedSpan>,
    families: &[String],
    wrap: WrapEnv,
    geom: WrapGeom,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) -> WrapOutcome {
    let WrapEnv {
        env,
        resolved,
        node_boxes,
        node_fill_prop,
        node_weight_prop,
        color_opacity,
        ctx,
    } = wrap;
    let WrapGeom {
        text_x,
        text_y,
        box_w,
        box_h_opt,
        font_size,
        letter_spacing_px,
        kerning_pairs,
        align,
        deco_thickness,
        direction: node_direction,
        glyph_stroke,
    } = geom;
    let engine = env.engine;
    let fonts = env.fonts;

    let base_weight = resolve_font_weight(node_weight_prop, resolved, 400);

    // ── Drop cap (single-box wrap path only) ─────────────────────────
    // Active when `drop-cap-lines >= 1` and the first body span carries at
    // least one character. The FIRST char (a `char`, the v0 grapheme unit)
    // is lifted out of the body here so the body wrap re-tokenizes only the
    // remainder; the oversized cap glyph is shaped AFTER the body pass so it
    // can use the real body `line_height`. When inactive, `dropcap_initial`
    // stays `None` and the body packs/emits exactly as before
    // (byte-identical).
    let dropcap_initial: Option<(DropCapInitial, u32)> = match text.drop_cap_lines {
        Some(n) if n >= 1 => take_drop_cap_initial(&mut resolved_spans).map(|init| (init, n)),
        _ => None,
    };
    let plain_wrap_features = resolved_spans
        .first()
        .map_or_else(Vec::new, |s| s.features.clone());

    let (tokens, metrics) = shape_words(
        &resolved_spans,
        families,
        NodeShape {
            font_size,
            base_weight,
            letter_spacing_px,
            kerning_pairs,
            direction: node_direction,
        },
        ShapeEnv { engine, fonts },
        diagnostics,
        &text.id,
        text.source_span,
    );

    // ── Baseline-grid snap (single-box wrap path) ────────────────────
    // When the page declares a positive baseline grid `g` AND no drop cap
    // is active on this node, snap the first baseline down to the grid and
    // inflate the inter-line advance to a multiple of `g`. Drop-cap +
    // baseline-grid is a documented v0 follow-up (skip the snap when a drop
    // cap is active, exactly like the existing drop-cap/chain deferral).
    // `text_y` here is already in the post-`ctx.dy` space, the same space
    // the grid origin is measured in. With no grid this leaves `emit_text_y`
    // = `text_y` and `emit_metrics` = `metrics` (byte-identical to before).
    let mut emit_text_y = text_y;
    let mut emit_metrics = metrics;
    let mut snapped = false;
    if let Some(g) = ctx.baseline_grid
        && g.is_finite()
        && g > 0.0
        && dropcap_initial.is_none()
    {
        let (snapped_text_y, effective_line_height) =
            snap_to_baseline_grid(text_y, metrics.ascent, metrics.line_height, g);
        emit_text_y = snapped_text_y;
        emit_metrics.line_height = effective_line_height;
        snapped = true;
        // Advisory: a single line is taller than one grid cell, so leading
        // grows to a multiple of `g`. Emit ONCE per node (not per line).
        if metrics.line_height > g {
            diagnostics.push(baseline_grid_snap_failed_diag(
                &text.id,
                metrics.line_height,
                g,
                text.source_span,
            ));
        }
    }

    // ── Text-runaround exclusion resolution ──────────────────────────
    // Resolve `text-exclusion` against this page's node boxes (moved into
    // render space by the page origin) using the
    // EFFECTIVE (post-baseline-snap) `emit_text_y` and line height, so the
    // band geometry composes with the baseline grid. An id naming no node box
    // → advisory + NO exclusion (uniform path, byte-identical). A drop cap
    // present → no exclusion (drop-cap + runaround is a v0 follow-up). When
    // the attribute is absent, `exclusion` stays `None` and the body packs/
    // emits exactly as before (byte-identical). Resolved here ONCE.
    let exclusion: Option<(f64, f64, f64, f64)> = match &text.text_exclusion {
        None => None,
        Some(target) => match node_boxes.get(target) {
            // Drop cap + runaround is a documented v0 follow-up: skip the
            // exclusion and keep the existing drop-cap path.
            Some(_) if dropcap_initial.is_some() => None,
            // Node boxes are page space; the text origin is render space
            // (page space plus the bleed offset).
            Some(&(ex, ey, ew, eh)) => {
                Some((ex + ctx.page_origin.0, ey + ctx.page_origin.1, ew, eh))
            }
            None => {
                diagnostics.push(Diagnostic::warning(
                    "text-exclusion.unresolved_ref",
                    format!(
                        "text node '{}' references unknown exclusion node '{}'",
                        text.id, target
                    ),
                    text.source_span,
                    Some(text.id.clone()),
                ));
                None
            }
        },
    };

    // Shape the cap now that the body `line_height` is known.
    let dropcap: Option<DropCap> = dropcap_initial.as_ref().and_then(|(init, n)| {
        let cap_size = drop_cap_font_size(font_size as f64, metrics.line_height, *n);
        shape_drop_cap(init, families, base_weight, cap_size, *n, engine, fonts)
    });

    let lines = if let Some(cap) = &dropcap {
        emit_drop_cap(EmitDropCap {
            cap,
            tokens,
            metrics,
            text_x,
            text_y,
            box_w,
            font_size,
            align,
            deco_thickness,
            glyph_stroke,
            source_node_id: text.id.as_str(),
            commands,
        })
    } else if let Some((ex, ey, ew, eh)) = exclusion {
        emit_runaround(EmitRunaround {
            tokens,
            metrics,
            emit_metrics,
            emit_text_y,
            text_x,
            box_w,
            box_h_opt,
            exclusion: (ex, ey, ew, eh),
            font_size,
            align,
            deco_thickness,
            direction: node_direction,
            glyph_stroke,
            source_node_id: text.id.as_str(),
            commands,
        })
    } else {
        emit_plain_wrap(
            text,
            EmitPlainWrap {
                tokens,
                metrics,
                emit_metrics,
                emit_text_y,
                text_x,
                box_w,
                font_size,
                align,
                deco_thickness,
                direction: node_direction,
                glyph_stroke,
                source_node_id: text.id.as_str(),
            },
            PlainWrapStyle {
                env,
                resolved,
                families,
                node_fill_prop,
                color_opacity,
                base_weight,
                features: plain_wrap_features,
                letter_spacing_px,
            },
            commands,
            diagnostics,
        )
    };
    WrapOutcome {
        lines,
        grid_height: snapped
            .then_some((emit_text_y - text_y) + lines as f64 * emit_metrics.line_height),
    }
}

/// Arguments for the drop-cap emit sub-path.
struct EmitDropCap<'a> {
    cap: &'a DropCap,
    tokens: Vec<WordToken>,
    metrics: WordMetrics,
    text_x: f64,
    text_y: f64,
    box_w: f64,
    font_size: f32,
    align: &'a str,
    deco_thickness: f64,
    glyph_stroke: (Option<Color>, Option<f64>),
    source_node_id: &'a str,
    commands: &'a mut Vec<SceneCommand>,
}

/// Emit the drop-cap initial + the wrap-around body lines, returning the body
/// line count.
fn emit_drop_cap(a: EmitDropCap) -> usize {
    let EmitDropCap {
        cap,
        tokens,
        metrics,
        text_x,
        text_y,
        box_w,
        font_size,
        align,
        deco_thickness,
        glyph_stroke,
        source_node_id,
        commands,
    } = a;
    // Gap between the drop cap's right edge and the wrapped body, as a
    // fraction of the body font size (documented constant).
    let gap = font_size as f64 * DROPCAP_GAP_FACTOR;
    let indent = cap.advance + gap;
    let n = cap.lines;
    let profile = WidthProfile {
        narrow_w: (box_w - indent).max(0.0),
        narrow_count: n,
        full_w: box_w,
    };

    let lines = pack_lines_variable(tokens, profile, metrics.space_advance, metrics.line_height);
    let fit_line_count = lines.len();

    // Drop-cap baseline sits on line `n`'s baseline (body ascent +
    // (n-1) line heights below the box top). Because the cap is sized so
    // its cap-height spans (n-1) lines + the body cap-height, this also
    // aligns the cap's cap-top with line 1's cap-top. Emit it ONCE at the
    // box left edge, in the node's resolved color/family.
    let cap_baseline_y = text_y + metrics.ascent + (n as f64 - 1.0) * metrics.line_height;
    commands.push(SceneCommand::DrawGlyphRun {
        x: text_x,
        y: cap_baseline_y,
        font_id: cap.run.font_id.clone(),
        font_size: cap.run.font_size,
        color: cap.color,
        stroke_color: glyph_stroke.0,
        stroke_width: glyph_stroke.1,
        link: None,
        selectable: true,
        source_node_id: Some(source_node_id.to_owned()),
        glyphs: run_to_scene_glyphs(&cap.run),
    });

    // Body wraps around: lines 0..n indented to the cap's right at the
    // narrow measure; line n onward at the box left, full measure.
    emit_lines_profiled(
        &lines,
        move |i| {
            if i < n {
                (text_x + indent, profile.narrow_w)
            } else {
                (text_x, box_w)
            }
        },
        text_y,
        EmitStyle {
            align,
            metrics,
            font_size,
            deco_thickness,
            justify_final_line: false,
            // Drop-cap wrap-around is an LTR feature in v0; RTL drop caps are
            // a documented follow-up.
            direction: TextDirection::Ltr,
            glyph_stroke,
            source_node_id: Some(source_node_id),
        },
        commands,
    );

    fit_line_count
}

/// Arguments for the text-runaround emit sub-path.
struct EmitRunaround<'a> {
    tokens: Vec<WordToken>,
    metrics: WordMetrics,
    emit_metrics: WordMetrics,
    emit_text_y: f64,
    text_x: f64,
    box_w: f64,
    box_h_opt: Option<f64>,
    exclusion: (f64, f64, f64, f64),
    font_size: f32,
    align: &'a str,
    deco_thickness: f64,
    direction: TextDirection,
    glyph_stroke: (Option<Color>, Option<f64>),
    source_node_id: &'a str,
    commands: &'a mut Vec<SceneCommand>,
}

/// Emit the text-runaround (largest-area / jump) lines, returning the line count.
fn emit_runaround(a: EmitRunaround) -> usize {
    let EmitRunaround {
        tokens,
        metrics,
        emit_metrics,
        emit_text_y,
        text_x,
        box_w,
        box_h_opt,
        exclusion: (ex, ey, ew, eh),
        font_size,
        align,
        deco_thickness,
        direction: node_direction,
        glyph_stroke,
        source_node_id,
        commands,
    } = a;

    // For each prospective line `i`, its vertical span is
    // `[lh_y(i), lh_y(i+1))` where `lh_y(i) = emit_text_y + i*lh`. A line
    // whose band overlaps the exclusion `[ey, ey+eh)` flows into the
    // LARGER free horizontal segment (left or right of the rect); a line
    // with neither segment ≥ MIN_W is BLOCKED (empty), so text flows
    // above and below a full-width exclusion. Hyphenation is disabled in
    // v0 runaround (like the drop-cap path).
    let lh = emit_metrics.line_height;
    // A line narrower than one space is useless → treat as blocked.
    let min_w = metrics.space_advance.max(1.0);
    // Half-open vertical-overlap test + larger-segment selection.
    let band_for = move |i: usize| -> (f64, f64) {
        let line_top = emit_text_y + (i as f64) * lh;
        let line_bottom = line_top + lh;
        // No overlap with the exclusion band → full measure.
        if line_bottom <= ey || line_top >= ey + eh {
            return (0.0, box_w);
        }
        let left_w = (ex - text_x).max(0.0);
        let right_w = ((text_x + box_w) - (ex + ew)).max(0.0);
        if left_w >= right_w && left_w >= min_w {
            (0.0, left_w)
        } else if right_w >= min_w {
            ((ex + ew) - text_x, right_w)
        } else {
            // Neither segment is wide enough → blocked line.
            (0.0, 0.0)
        }
    };

    // Bound the blocked-skip loop: at most the number of lines that fit
    // the box height (when known) plus slack, else a safe constant cap.
    let max_lines = match box_h_opt {
        Some(box_h) if lh > 0.0 => ((box_h / lh).ceil() as usize).saturating_add(4),
        _ => 4096,
    };

    let lines = pack_lines_runaround(
        tokens,
        |i| band_for(i).1,
        metrics.space_advance,
        min_w,
        max_lines,
        emit_metrics.line_height,
    );
    let fit_line_count = lines.len();

    // Per-line geometry: blocked lines emit as empty `Line`s (no words),
    // so the baseline advances past them with no glyphs — producing the
    // above/below flow naturally.
    emit_lines_profiled(
        &lines,
        |i| {
            let (dx, w) = band_for(i);
            (text_x + dx, w)
        },
        emit_text_y,
        EmitStyle {
            align,
            metrics: emit_metrics,
            font_size,
            deco_thickness,
            justify_final_line: false,
            direction: node_direction,
            glyph_stroke,
            source_node_id: Some(source_node_id),
        },
        commands,
    );

    fit_line_count
}

/// Geometry/style scalars for the plain-wrap emit sub-path.
struct EmitPlainWrap<'a> {
    tokens: Vec<WordToken>,
    metrics: WordMetrics,
    emit_metrics: WordMetrics,
    emit_text_y: f64,
    text_x: f64,
    box_w: f64,
    font_size: f32,
    align: &'a str,
    deco_thickness: f64,
    direction: TextDirection,
    glyph_stroke: (Option<Color>, Option<f64>),
    source_node_id: &'a str,
}

/// Borrowed environment for the plain-wrap sub-path (bullet + hyphenation).
struct PlainWrapStyle<'a> {
    env: ShapeEnv<'a>,
    resolved: &'a BTreeMap<String, ResolvedToken>,
    families: &'a [String],
    node_fill_prop: Option<&'a PropertyValue>,
    color_opacity: f64,
    base_weight: u16,
    features: Vec<FontFeature>,
    letter_spacing_px: f32,
}

/// Emit the plain wrap path (hyphenation/break-word + bullet/hanging-indent),
/// returning the line count.
fn emit_plain_wrap(
    text: &TextNode,
    geom: EmitPlainWrap,
    style: PlainWrapStyle,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) -> usize {
    let EmitPlainWrap {
        tokens,
        metrics,
        emit_metrics,
        emit_text_y,
        text_x,
        box_w,
        font_size,
        align,
        deco_thickness,
        direction: node_direction,
        glyph_stroke,
        source_node_id,
    } = geom;
    let PlainWrapStyle {
        env,
        resolved,
        families,
        node_fill_prop,
        color_opacity,
        base_weight,
        features,
        letter_spacing_px,
    } = style;
    let engine = env.engine;
    let fonts = env.fonts;

    // Opt-in hyphenation and/or break-word: build a context when EITHER
    // `hyphenate=#true` OR `overflow-wrap="break-word"` is set. The
    // dictionary is loaded regardless (it is needed only by the
    // hyphenation branch; break-word is independent of it), so a
    // break-word-only node still gets a context even if the dict is `None`.
    // When NEITHER is requested the context is `None` → the packer is
    // byte-identical to before.
    let want_hyphenate = text.hyphenate == Some(true);
    let want_break_word = text.overflow_wrap.as_deref() == Some("break-word");
    let hyph_ctx = if want_hyphenate || want_break_word {
        Some(HyphenationContext {
            // `dict` is consulted only by the hyphenation branch (which
            // also requires `want_hyphenate` via a `None` early-return in
            // `try_hyphenate`); a break-word-only node leaves it `None`.
            dict: if want_hyphenate {
                en_us_hyphenator()
            } else {
                None
            },
            engine,
            fonts,
            families,
            hyphen: "-",
            direction: node_direction,
            break_word: want_break_word,
        })
    } else {
        None
    };
    // ── Auto-aligning bullet marker ───────────────────────────────────
    // When `bullet` is `Some(marker)` with a non-empty string on the plain
    // wrap path (drop-cap/runaround/chain are handled above), the marker is:
    //   1. Shaped at the node's own font/weight/size to get `marker_advance`.
    //   2. Combined with the gap (`bullet_gap` or `0.4 × font_size`) to give
    //      `M = marker_advance + gap_px`.
    //   3. Stacked on top of any explicit `padding_left` (ADDED), so the
    //      effective indent is `M + explicit_pl`. An explicit `text_indent`
    //      is ignored on a bullet node (documented v0 follow-up).
    //   4. The marker is drawn once as a `DrawGlyphRun` at `x = text_x`
    //      (the UN-indented box edge, i.e. in the left margin), at the
    //      first line's baseline (`emit_text_y + emit_metrics.ascent`), in
    //      the node's resolved fill color. All text lines (first AND
    //      wrapped) are indented by `M + explicit_pl` via the reused
    //      `emit_lines_profiled` per-line geometry mechanism.
    // When `bullet` is `None` (or empty) this block is a no-op and the
    // node is BYTE-IDENTICAL to a node without the attribute.
    let bullet_run: Option<(ZenithGlyphRun, Color)> =
        match text.bullet.as_deref().filter(|s| !s.is_empty()) {
            None => None,
            Some(marker) => {
                // Resolve node fill color for the marker (same cascade as
                // the body spans: node fill → style fill → black).
                // Reuses `node_fill_prop` already computed above.
                let mut marker_color = node_fill_prop
                    .and_then(|fp| resolve_property_color(fp, resolved, diagnostics, &text.id))
                    .unwrap_or(Color::srgb(0, 0, 0, 255));
                marker_color.a = (marker_color.a as f64 * color_opacity).round() as u8;

                // Shape the marker string at the node's resolved
                // font/weight/size (mirror `shape_drop_cap`). Take only
                // the FIRST run on success (the marker is a single glyph
                // cluster). On shaping failure the bullet is silently
                // skipped (no marker drawn, no extra indent) so the body
                // still renders.
                // `base_weight` was already resolved above for word shaping.
                let req = ShapeRequest {
                    text: marker,
                    families,
                    weight: base_weight,
                    style: zenith_core::FontStyle::Normal,
                    font_size,
                    // Bullet marker is always LTR (the glyph faces left
                    // regardless of body direction in v0).
                    direction: TextDirection::Ltr,
                    features: &features,
                    kerning_pairs: &[],
                    letter_spacing_px,
                };
                match engine.shape_with_fallback(&req, fonts) {
                    Ok(result) => result.runs.into_iter().next().map(|r| (r, marker_color)),
                    Err(_) => None,
                }
            }
        };

    // ── Hanging indent: padding-left + bullet-M + (optional negative) text-indent ─
    // `pl` indents EVERY line's left edge inward (reducing the measure);
    // `ti` shifts line 0 by an additional amount relative to the padded
    // edge (may be negative to pull the first line back out for a hanging
    // bullet). Both default to 0. This composes with hyphenation/break-
    // word (via `hyph_ctx`), justify and RTL (via `emit_lines_profiled`'s
    // align/direction), and the baseline grid (already folded into
    // `emit_text_y`/`emit_metrics` above). Combining indent with the
    // drop-cap, runaround, or chain paths is a documented v0 follow-up:
    // those branches use their own per-line width profiles and are
    // handled above, so this code is reached only on the plain wrap path.
    let explicit_pl = text
        .padding_left
        .as_ref()
        .and_then(|d| dim_to_px(d.value, &d.unit))
        .unwrap_or(0.0);
    // Bullet auto-indent: measured marker advance + gap, added ON TOP of
    // any explicit `padding_left`. When there is no bullet run (bullet
    // absent, empty, or shaping failed) `bullet_m = 0.0` so the rest of
    // the logic is byte-identical to the pre-bullet path.
    let bullet_m: f64 = match &bullet_run {
        None => 0.0,
        Some((run, _)) => {
            let marker_advance = run.advance_width as f64;
            let gap_px = text
                .bullet_gap
                .as_ref()
                .and_then(|d| dim_to_px(d.value, &d.unit))
                .unwrap_or(0.4 * font_size as f64);
            marker_advance + gap_px
        }
    };
    let pl = bullet_m + explicit_pl;
    // Explicit `text_indent` is ignored on a bullet node (documented).
    // On a non-bullet node it is honoured as before (byte-identical).
    let ti = if bullet_run.is_some() {
        0.0
    } else {
        text.text_indent
            .as_ref()
            .and_then(|d| dim_to_px(d.value, &d.unit))
            .unwrap_or(0.0)
    };

    let mut forced_break = false;
    let lines: Vec<Line> = if pl == 0.0 && ti == 0.0 {
        // Default-off: byte-identical to the historical uniform packing.
        pack_lines_reporting(
            tokens,
            box_w,
            metrics.space_advance,
            hyph_ctx.as_ref(),
            &mut forced_break,
            emit_metrics.line_height,
        )
    } else {
        // Line 0 measure is `box_w - pl - ti`; lines ≥1 are `box_w - pl`.
        // Widths clamp to ≥ 0 so a large pad/indent never goes negative.
        let width_for = |i: usize| {
            if i == 0 {
                (box_w - pl - ti).max(0.0)
            } else {
                (box_w - pl).max(0.0)
            }
        };
        pack_lines_core(
            tokens,
            width_for,
            LineMetrics {
                space_advance: metrics.space_advance,
                min_line_width: f64::NEG_INFINITY,
                line_height: emit_metrics.line_height,
            },
            hyph_ctx.as_ref(),
            usize::MAX,
            &mut forced_break,
        )
    };

    // One advisory per node when a forced character-boundary break
    // occurred (break-word split an overlong token).
    if forced_break {
        diagnostics.push(Diagnostic::warning(
            "text.forced_break",
            format!(
                "text node '{}' has a token wider than its column; forced a \
                 character-boundary break (consider editing the copy)",
                text.id
            ),
            text.source_span,
            Some(text.id.clone()),
        ));
    }

    // Record the actual line count for the caller's overflow measurement.
    let fit_line_count = lines.len();

    // Emit the bullet marker BEFORE the text runs (drawn first → below the
    // text in z-order, consistent with drop-cap emission order). The
    // baseline is the SNAPPED first-line baseline so the marker aligns with
    // the body's first line regardless of baseline-grid state.
    if let Some((marker_run, marker_color)) = bullet_run {
        let marker_baseline_y = emit_text_y + emit_metrics.ascent;
        let glyphs = run_to_scene_glyphs(&marker_run);
        commands.push(SceneCommand::DrawGlyphRun {
            x: text_x,
            y: marker_baseline_y,
            font_id: marker_run.font_id,
            font_size: marker_run.font_size,
            color: marker_color,
            stroke_color: glyph_stroke.0,
            stroke_width: glyph_stroke.1,
            link: None,
            selectable: true,
            source_node_id: Some(source_node_id.to_owned()),
            glyphs,
        });
    }

    if pl == 0.0 && ti == 0.0 {
        emit_lines(
            &lines,
            text_x,
            // Baseline-grid snap (no-op when no grid is active): the first
            // baseline lands on the grid and the advance is a multiple of g.
            emit_text_y,
            box_w,
            EmitStyle {
                align,
                metrics: emit_metrics,
                font_size,
                deco_thickness,
                // Single-box wrap: the batch's last line IS the paragraph's
                // last line → leave it ragged under justify.
                justify_final_line: false,
                direction: node_direction,
                glyph_stroke,
                source_node_id: Some(source_node_id),
            },
            commands,
        );
    } else {
        // Per-line geometry mirrors the packing widths: line 0 starts at
        // `text_x + pl + ti` (the outdented bullet edge when `ti < 0`),
        // continuation lines start at `text_x + pl`.
        emit_lines_profiled(
            &lines,
            |i| {
                if i == 0 {
                    (text_x + pl + ti, (box_w - pl - ti).max(0.0))
                } else {
                    (text_x + pl, (box_w - pl).max(0.0))
                }
            },
            emit_text_y,
            EmitStyle {
                align,
                metrics: emit_metrics,
                font_size,
                deco_thickness,
                justify_final_line: false,
                direction: node_direction,
                glyph_stroke,
                source_node_id: Some(source_node_id),
            },
            commands,
        );
    }

    fit_line_count
}
