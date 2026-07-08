//! The `code` leaf compile path: monospace multi-line emission with an optional
//! line-number gutter and optional per-token syntax highlighting.

use zenith_core::{
    CodeNode, Diagnostic, FontStyle, TokenKind, builtin_color, is_supported, scan,
    token_id_for_kind,
};
use zenith_layout::{ShapeRequest, TextDirection, TextLayoutEngine};

use crate::color::parse_srgb_hex;
use crate::ir::{Color, SceneCommand};

use super::super::RenderCtx;
use super::super::paint::resolve_property_color;
use super::super::style_prop;
use super::super::util::{resolve_geometry_px, rotation_degrees, unsupported_unit_diag};
use super::ctx::TextCompileEnv;
use super::resolve_kerning_pairs;
use super::shape::{
    resolve_family_with_fallback, resolve_font_family_name, resolve_font_features,
    resolve_font_weight, resolve_letter_spacing, run_to_scene_glyphs,
};

/// Pixels of padding on the left and right sides of the line-number digit area.
const GUTTER_PAD: f64 = 6.0;

/// Resolved geometry for the line-number gutter.
///
/// `None` when line numbers are disabled or the digit "0" could not be shaped
/// (graceful degradation — no panic, just no gutter).
struct GutterMetrics {
    /// Width of the full gutter band (digits + both pads).
    gutter_width: f64,
    /// Right edge of the digit area inside the gutter (left pad + digits).
    right_edge: f64,
    /// Ascent for the gutter font (same face/size as code text).
    ascent: f64,
    /// Line height for the gutter font.
    line_height: f64,
    /// Muted color used for line numbers (comment color, theme-aware).
    number_color: Color,
}

/// Compile a `code` leaf node.
///
/// Returns the laid-out content height in pixels (`line_count * line_height`,
/// where `line_count` counts every physical source line including blanks),
/// which the flow-layout path in [`super::super::container`] uses to advance its
/// vertical cursor past a code child that declares no explicit `h`. Early
/// returns (invisible, missing/bad geometry) yield `0.0`.
pub(in crate::compile) fn compile_code(
    code: &CodeNode,
    env: TextCompileEnv,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: RenderCtx,
) -> f64 {
    // Emit, then downgrade this node's glyph runs to outlines when the node opts
    // out of selectable text. Purely a PDF render concern, applied as a post-pass
    // over exactly the commands this node produced (see `compile_text`). Default
    // (`None`/`Some(true)`) is byte-identical.
    let start = commands.len();
    let height = compile_code_impl(code, env, commands, diagnostics, ctx);
    if code.selectable == Some(false) {
        super::shape::mark_runs_unselectable(&mut commands[start..]);
    }
    height
}

fn compile_code_impl(
    code: &CodeNode,
    env: TextCompileEnv,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: RenderCtx,
) -> f64 {
    let resolved = env.resolved;
    let style_map = env.style_map;
    let fonts = env.fonts;
    let engine = env.engine;
    let anchors = env.anchors;

    // Skip invisible code nodes.
    if code.visible == Some(false) {
        return 0.0;
    }

    // Width/height are OPTIONAL; they bound the clip rectangle when
    // present. A bad unit yields None (no clip), not a hard skip.
    let code_w: Option<f64> = resolve_geometry_px(code.w.as_ref(), resolved);
    let code_h: Option<f64> = resolve_geometry_px(code.h.as_ref(), resolved);

    // Anchor-derived (x, y): look up the pre-pass map when x or y is absent.
    // The pre-pass requires w/h to be px, so anchor derivation only activates
    // when code has resolvable dimensions.
    let anchor_xy = anchors.get(&code.id).copied();

    // Resolve x — use authored value when present, anchor derivation when absent.
    let code_x_raw = match &code.x {
        Some(x_dim) => {
            let Some(v) = resolve_geometry_px(Some(x_dim), resolved) else {
                diagnostics.push(unsupported_unit_diag(
                    "code node",
                    &code.id,
                    "x",
                    code.source_span,
                ));
                return 0.0;
            };
            v
        }
        None => {
            if let Some((ax, _)) = anchor_xy {
                ax
            } else {
                diagnostics.push(Diagnostic::advisory(
                    "scene.missing_geometry",
                    format!(
                        "code node '{}' is missing x or y geometry; skipped",
                        code.id
                    ),
                    code.source_span,
                    Some(code.id.clone()),
                ));
                return 0.0;
            }
        }
    };

    // Resolve y — same pattern.
    let code_y_raw = match &code.y {
        Some(y_dim) => {
            let Some(v) = resolve_geometry_px(Some(y_dim), resolved) else {
                diagnostics.push(unsupported_unit_diag(
                    "code node",
                    &code.id,
                    "y",
                    code.source_span,
                ));
                return 0.0;
            };
            v
        }
        None => {
            if let Some((_, ay)) = anchor_xy {
                ay
            } else {
                diagnostics.push(Diagnostic::advisory(
                    "scene.missing_geometry",
                    format!(
                        "code node '{}' is missing x or y geometry; skipped",
                        code.id
                    ),
                    code.source_span,
                    Some(code.id.clone()),
                ));
                return 0.0;
            }
        }
    };

    // Apply group translation offset.
    let code_x = code_x_raw + ctx.dx;
    let code_y = code_y_raw + ctx.dy;

    // Resolve font family with style cascade.
    // Priority: node-local font_family → style font-family → default
    // "Noto Sans Mono" (the monospace default for code).
    let font_family_prop = code
        .font_family
        .as_ref()
        .or_else(|| style_prop(&code.style, style_map, "font-family"));
    let raw_family_name = resolve_font_family_name(font_family_prop, resolved, "Noto Sans Mono");
    // Probe the provider before shaping to avoid silently dropping lines
    // when the requested mono family is unregistered.
    let (family_name, fell_back, is_local) = resolve_family_with_fallback(
        fonts,
        &raw_family_name,
        "Noto Sans Mono",
        400,
        FontStyle::Normal,
    );
    if fell_back {
        diagnostics.push(Diagnostic::advisory(
            "font.unresolved",
            format!(
                "code node '{}': font family '{}' not available, falling back to 'Noto Sans Mono'",
                code.id, raw_family_name
            ),
            code.source_span,
            Some(code.id.clone()),
        ));
    }
    if is_local {
        diagnostics.push(Diagnostic::advisory(
            "font.local",
            format!(
                "code node '{}': font family '{}' resolved from a local/system font; rendering is \
                 NOT guaranteed deterministic across machines — bundle the font or guarantee the \
                 target OS provides it",
                code.id, raw_family_name
            ),
            code.source_span,
            Some(code.id.clone()),
        ));
    }
    let families = vec![family_name];

    // Resolve font size in pixels with style cascade; default to 14.0.
    let font_size_prop = code
        .font_size
        .clone()
        .or_else(|| style_prop(&code.style, style_map, "font-size").cloned());
    let font_size: f32 =
        super::super::util::resolve_property_dimension_px(font_size_prop.as_ref(), resolved, 14.0)
            as f32;

    // Resolve font weight with style cascade; default to 400.
    // A weight of 700 causes the provider to select Noto Sans Mono Bold
    // instead of Noto Sans Mono Regular (unchanged → byte-identical for 400).
    let font_weight_prop = code
        .font_weight
        .as_ref()
        .or_else(|| style_prop(&code.style, style_map, "font-weight"));
    let weight = resolve_font_weight(font_weight_prop, resolved, 400);
    let font_features = resolve_font_features(
        code.font_features.as_deref(),
        diagnostics,
        &code.id,
        code.source_span,
    );
    let letter_spacing_prop = code
        .letter_spacing
        .as_ref()
        .or_else(|| style_prop(&code.style, style_map, "letter-spacing"));
    let letter_spacing_px = resolve_letter_spacing(letter_spacing_prop, resolved);
    let kerning_pairs = resolve_kerning_pairs(&code.kerning_pairs, resolved);

    // Resolve fill color with style cascade; default to opaque black.
    let fill_prop = code
        .fill
        .as_ref()
        .or_else(|| style_prop(&code.style, style_map, "fill"));
    let mut color = fill_prop
        .and_then(|fp| resolve_property_color(fp, resolved, diagnostics, &code.id))
        .unwrap_or(Color::srgb(0, 0, 0, 255));

    // Apply node opacity then cascade ctx.opacity on top.
    let node_opacity = code.opacity.unwrap_or(1.0).clamp(0.0, 1.0);
    color.a = (color.a as f64 * node_opacity * ctx.opacity).round() as u8;

    // Tab expansion: replace each literal tab with `tab_width` spaces
    // (default 4). A `tab_width` of 0 makes tabs vanish — acceptable.
    let tab_width = code.tab_width.unwrap_or(4) as usize;
    let expanded = code.content.replace('\t', &" ".repeat(tab_width));

    // Rotation bracket (outermost — wraps the clip). Only when both w and h
    // are present (needed for a safe pivot center). Unrotated code nodes
    // emit no PushTransform → byte-identical to before.
    let rot = rotation_degrees(code.rotate.as_ref());
    let code_rot = rot
        .zip(code_w)
        .zip(code_h)
        .map(|((a, cw), ch)| (a, code_x + cw / 2.0, code_y + ch / 2.0));
    if let Some((angle, cx, cy)) = code_rot {
        commands.push(SceneCommand::PushTransform {
            angle_deg: angle,
            cx,
            cy,
        });
    }

    // Overflow clip: clipping is the default; only `overflow="visible"`
    // disables it. The clip is applied only when enabled AND both w and
    // h resolved (the clip rectangle is fully determined). Resolve the
    // decision BEFORE the PushClip so push/pop stay balanced across the
    // emission loop (mirrors compile_frame/compile_image discipline).
    let clip_enabled = code.overflow.as_deref() != Some("visible");
    let clip_box = match (clip_enabled, code_w, code_h) {
        (true, Some(w), Some(h)) => Some((w, h)),
        _ => None,
    };
    if let Some((w, h)) = clip_box {
        commands.push(SceneCommand::PushClip {
            x: code_x,
            y: code_y,
            w,
            h,
        });
    }

    // Resolve syntax-highlighting settings before the per-line loop.
    // `theme` drives builtin fallback colors; `hl_lang` is `Some` only
    // when the node declares a language that oxidoc-highlight supports.
    // When `hl_lang` is `None` the existing single-run path is used
    // unchanged, guaranteeing byte-identical output for all non-highlighted
    // documents.
    let theme = code.syntax_theme.unwrap_or_default();
    let hl_lang: Option<&str> = code.language.as_deref().filter(|l| is_supported(l));

    // Helper: resolve a TokenKind to a Color, consulting doc tokens first
    // and falling back to the builtin palette. Opacity is baked in.
    let syntax_color = |kind: TokenKind| -> Color {
        let hex: &str = resolved
            .get(token_id_for_kind(kind))
            .and_then(|rt| rt.value.as_color_hex())
            .unwrap_or_else(|| builtin_color(theme, kind));
        let mut c = parse_srgb_hex(hex).unwrap_or(Color::srgb(0, 0, 0, 255));
        c.a = (c.a as f64 * node_opacity * ctx.opacity).round() as u8;
        c
    };

    // ── Line-number gutter (only when `line_numbers = true`) ─────────────
    //
    // When enabled:
    //   - Shape the digit "0" once to obtain per-digit advance width and
    //     the gutter font's ascent/line_height.
    //   - Compute gutter_width = digit_count * digit_advance + 2*GUTTER_PAD.
    //   - Shift the code-text x-origin right by gutter_width.
    //   - For every physical line (including blank ones) emit a right-aligned
    //     line-number glyph run in the gutter, colored with the comment color
    //     (the muted/secondary color used by the syntax highlighter).
    //
    // When disabled the entire block is unreachable and the code path is
    // byte-identical to before.
    let gutter: Option<GutterMetrics> = if code.line_numbers == Some(true) {
        let physical_line_count = expanded.split('\n').count();
        // Number of decimal digits in the total line count (minimum 1).
        let digit_count = {
            let mut n = physical_line_count;
            let mut d = 0usize;
            loop {
                d += 1;
                n /= 10;
                if n == 0 {
                    break;
                }
            }
            d
        };

        // Shape the digit "0" to obtain per-digit metrics.
        let digit_req = ShapeRequest {
            text: "0",
            families: &families,
            weight,
            style: FontStyle::Normal,
            font_size,
            direction: TextDirection::Ltr,
            features: &font_features,
            kerning_pairs: &[],
            letter_spacing_px,
        };
        match engine.shape(&digit_req, fonts) {
            Err(_) => {
                // Cannot shape the digit → skip the gutter entirely, proceed
                // without line numbers rather than panicking.
                None
            }
            Ok(digit_run) => {
                let digit_advance = digit_run.advance_width as f64;
                let gutter_digits_w = digit_count as f64 * digit_advance;
                let gutter_width = gutter_digits_w + 2.0 * GUTTER_PAD;
                let right_edge = code_x + gutter_digits_w + GUTTER_PAD;
                let number_color = syntax_color(TokenKind::Comment);
                Some(GutterMetrics {
                    gutter_width,
                    right_edge,
                    ascent: digit_run.ascent as f64,
                    line_height: digit_run.line_height as f64,
                    number_color,
                })
            }
        }
    } else {
        None
    };

    // Shift the code-text x-origin right by the gutter width so that
    // gutter and code-text occupy non-overlapping horizontal bands.
    let code_x = match &gutter {
        Some(g) => code_x + g.gutter_width,
        None => code_x,
    };

    // Multi-line emission: each physical line becomes its own glyph run,
    // stacked by `line_height`. Blank lines emit no run but the index `i`
    // still advances, preserving their vertical space. All non-blank
    // lines share identical ascent/line_height (same font + size), so the
    // per-line metrics give consistent stacking.
    //
    // `measured_line_height` captures the shared per-line height from the
    // first successfully shaped run; combined with the physical line count
    // it gives the laid-out content height returned for flow layout.
    let mut measured_line_height: f64 = 0.0;
    // Largest index of a non-empty (ink-bearing) line; `(last + 1)` is the
    // line count spanned by the block, ignoring trailing blank lines.
    let mut last_inked_line: Option<usize> = None;
    for (i, line) in expanded.split('\n').enumerate() {
        // ── Gutter: emit line number BEFORE the code text for this line ──
        // Blank lines are numbered too (editors number every physical line).
        // We emit the gutter run first so ordering is gutter-then-code.
        if let Some(g) = &gutter {
            // Use the gutter font metrics for the baseline; they share the
            // same font family/size/weight as the code text, so the baselines
            // are identical.
            let baseline_y = code_y + g.ascent + (i as f64) * g.line_height;
            let label = (i + 1).to_string();
            let num_req = ShapeRequest {
                text: &label,
                families: &families,
                weight,
                style: FontStyle::Normal,
                font_size,
                direction: TextDirection::Ltr,
                features: &font_features,
                kerning_pairs: &kerning_pairs,
                letter_spacing_px,
            };
            // If shaping fails for a particular label, skip gracefully.
            if let Ok(num_run) = engine.shape(&num_req, fonts) {
                // Right-align: use the actual shaped advance so the right edge
                // of the number lands exactly at `right_edge`, even if the
                // font does not use identical advance for every digit.
                let actual_width = num_run.advance_width as f64;
                let actual_x = g.right_edge - actual_width;
                let glyphs = run_to_scene_glyphs(&num_run);
                commands.push(SceneCommand::DrawGlyphRun {
                    x: actual_x,
                    y: baseline_y,
                    font_id: num_run.font_id,
                    font_size: num_run.font_size,
                    color: g.number_color,
                    stroke_color: None,
                    stroke_width: None,
                    link: None,
                    selectable: true,
                    source_node_id: Some(code.id.clone()),
                    glyphs,
                });
            }
        }

        if line.is_empty() {
            continue;
        }
        last_inked_line = Some(i);

        if let Some(lang) = hl_lang {
            // ── Highlighted path: per-token colored segments ──────────
            // Tokenise the line, walk gaps between tokens, collect
            // (text_slice, color) pairs, shape each, then emit.
            let plain_color = syntax_color(TokenKind::Plain);
            let tokens = scan(line, lang);

            // Build segment list: (slice, color)
            let mut segments: Vec<(&str, Color)> = Vec::new();
            let mut pos: usize = 0;
            for tok in &tokens {
                // Gap before this token → plain color.
                if tok.start > pos
                    && let Some(gap) = line.get(pos..tok.start)
                    && !gap.is_empty()
                {
                    segments.push((gap, plain_color));
                }
                if let Some(slice) = line.get(tok.start..tok.end)
                    && !slice.is_empty()
                {
                    segments.push((slice, syntax_color(tok.kind)));
                }
                pos = tok.end;
            }
            // Trailing gap after last token.
            if pos < line.len()
                && let Some(tail) = line.get(pos..)
                && !tail.is_empty()
            {
                segments.push((tail, plain_color));
            }

            // Shape all segments; collect (run, color) pairs so metrics
            // can be read from the first successful run before emitting.
            let mut shaped = Vec::new();
            for (seg_text, seg_color) in segments {
                let req = ShapeRequest {
                    text: seg_text,
                    families: &families,
                    weight,
                    style: FontStyle::Normal,
                    font_size,
                    // Code is shaped LTR (source code is left-to-right).
                    direction: TextDirection::Ltr,
                    features: &font_features,
                    kerning_pairs: &kerning_pairs,
                    letter_spacing_px,
                };
                match engine.shape(&req, fonts) {
                    Err(e) => {
                        diagnostics.push(Diagnostic::advisory(
                            "scene.text_unshaped",
                            format!("code node '{}' could not be shaped: {}", code.id, e.message),
                            code.source_span,
                            Some(code.id.clone()),
                        ));
                        // Skip this segment; cursor does not advance.
                    }
                    Ok(run) => {
                        shaped.push((run, seg_color));
                    }
                }
            }

            // Emit: metrics are font-constant, read from first shaped run.
            if let Some((first_run, _)) = shaped.first() {
                if measured_line_height == 0.0 {
                    measured_line_height = first_run.line_height as f64;
                }
                let baseline_y =
                    code_y + first_run.ascent as f64 + (i as f64) * first_run.line_height as f64;
                let mut x_cursor = code_x;
                for (run, seg_color) in shaped {
                    let advance = run.advance_width as f64;
                    let glyphs = run_to_scene_glyphs(&run);
                    commands.push(SceneCommand::DrawGlyphRun {
                        x: x_cursor,
                        y: baseline_y,
                        font_id: run.font_id,
                        font_size: run.font_size,
                        color: seg_color,
                        stroke_color: None,
                        stroke_width: None,
                        link: None,
                        selectable: true,
                        source_node_id: Some(code.id.clone()),
                        glyphs,
                    });
                    x_cursor += advance;
                }
            }
        } else {
            // ── Plain path (no highlighting): single run per line ──────
            // weight defaults to 400 (from resolve_font_weight), so code
            // nodes without font-weight remain byte-identical to before.
            let req = ShapeRequest {
                text: line,
                families: &families,
                weight,
                style: FontStyle::Normal,
                font_size,
                // Code is shaped LTR (source code is left-to-right).
                direction: TextDirection::Ltr,
                features: &font_features,
                kerning_pairs: &kerning_pairs,
                letter_spacing_px,
            };

            match engine.shape(&req, fonts) {
                Err(e) => {
                    diagnostics.push(Diagnostic::advisory(
                        "scene.text_unshaped",
                        format!("code node '{}' could not be shaped: {}", code.id, e.message),
                        code.source_span,
                        Some(code.id.clone()),
                    ));
                    continue;
                }
                Ok(run) => {
                    if measured_line_height == 0.0 {
                        measured_line_height = run.line_height as f64;
                    }
                    let baseline_y =
                        code_y + run.ascent as f64 + (i as f64) * run.line_height as f64;
                    let glyphs = run_to_scene_glyphs(&run);

                    commands.push(SceneCommand::DrawGlyphRun {
                        x: code_x,
                        y: baseline_y,
                        font_id: run.font_id,
                        font_size: run.font_size,
                        color,
                        stroke_color: None,
                        stroke_width: None,
                        link: None,
                        selectable: true,
                        source_node_id: Some(code.id.clone()),
                        glyphs,
                    });
                }
            }
        }
    }

    if clip_box.is_some() {
        commands.push(SceneCommand::PopClip);
    }

    if code_rot.is_some() {
        commands.push(SceneCommand::PopTransform);
    }

    // Laid-out content height: number of lines spanned (index of the last
    // ink-bearing line + 1, so trailing blank lines do not inflate the box)
    // times the shared per-line height. Zero when nothing was shaped.
    match last_inked_line {
        Some(last) => (last + 1) as f64 * measured_line_height,
        None => 0.0,
    }
}
