//! Contrast of `text` nodes: sampling the backdrop under a text box and
//! judging the text fill against it with APCA.

use crate::ast::node::TextNode;
use crate::color::apca_lc;
use crate::diagnostics::Diagnostic;

use super::geometry::{RectPx, text_box};
use super::props::{
    resolve_color_property, resolve_font_size, resolve_font_weight, style_property,
};
use super::types::{
    BLACK, BackdropCandidate, BackdropPaint, ContrastEnv, ContrastSample, INVISIBLE_LC_FLOOR,
    PaintColor, PaintCtx, SampledBackdrop,
};

/// The minimum `Lc` for text of `size_px` and `weight`: 45 for large text,
/// else 60.
pub(super) fn lc_threshold(size_px: f64, weight: u32) -> f64 {
    let is_large = size_px >= 24.0 || (size_px >= 18.66 && weight >= 700);
    if is_large { 45.0 } else { 60.0 }
}

pub(super) fn check_text_node(
    text: &TextNode,
    ctx: PaintCtx<'_>,
    candidates: &[BackdropCandidate],
    env: ContrastEnv<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // A table header paints its `header_style` on a text with no style.
    let style = text.style.as_deref().or(ctx.header_style);
    // A text with no fill paints the engine default, black.
    let fg_rgb = match text
        .fill
        .as_ref()
        .or_else(|| style_property(style, "fill", env.style_map))
    {
        None => BLACK,
        Some(fill) => match resolve_color_property(Some(fill), env.resolved_tokens) {
            Some(rgb) => rgb,
            None => return,
        },
    };

    // A fit transform scales the drawn glyphs; the smaller axis scale rules.
    let size_px =
        resolve_font_size(text, style, env.style_map, env.resolved_tokens) * ctx.sx.min(ctx.sy);
    let weight = resolve_font_weight(text, style, env.style_map, env.resolved_tokens);
    let threshold = lc_threshold(size_px, weight);

    let hint_rgb = resolve_color_property(text.contrast_bg.as_ref(), env.resolved_tokens);
    let mut backdrop_samples = Vec::new();
    if hint_rgb.is_none() {
        // The text sample box must live in ABSOLUTE page space, mapped by the
        // accumulated ancestor offset and scale, so it lands on the same
        // coordinates as the (already-absolute) backdrop candidates and frame
        // clip.
        let Some(text_bbox) =
            text_box(text, ctx.page_size, env.resolved_tokens).map(|b| ctx.place().rect(b))
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
                    "text '{}': its backdrop includes an unsampled paint (image, or a rotated/masked/blurred/blended fill) and cannot be sampled during validation; add a contrast-bg hint",
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

pub(super) fn collect_backdrop_samples(
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

pub(super) fn select_contrast_sample(
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
