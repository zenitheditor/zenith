//! Span preparation for a sized node: footnote expansion, per-span style
//! resolution, and per-span shaping.

use std::collections::{BTreeMap, BTreeSet};

use zenith_core::{Diagnostic, FontStyle, TextNode, TextSpan};
use zenith_layout::{FontFeature, ShapeRequest, TextLayoutEngine, ZenithGlyphRun};

use crate::compile::paint::resolve_property_color;
use crate::compile::text::shape::{
    CODE_MONO_FAMILY, LINK_COLOR, ResolvedSpan, emit_glyph_missing, resolve_font_weight,
    resolve_letter_spacing, resolve_span_font_feature_set, resolve_vertical_align,
};
use crate::ir::Color;

use super::style::NodeStyle;

/// Expand the node's spans into the EFFECTIVE span list.
///
/// A span carrying a `footnote_ref` keeps its text, then is immediately
/// followed by a synthetic superscript marker span (the footnote's marker
/// string). The marker reuses the `vertical-align="super"` path and inherits
/// the ref span's fill. A ref that names no footnote on the page pushes
/// `footnote.unresolved_ref` and adds no marker. With no ref the list equals
/// `text.spans`.
pub(super) fn effective_spans(
    text: &TextNode,
    footnote_markers: &BTreeMap<String, String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<TextSpan> {
    if !text.spans.iter().any(|s| s.footnote_ref.is_some()) {
        return text.spans.clone();
    }
    let mut out: Vec<TextSpan> = Vec::with_capacity(text.spans.len());
    for span in &text.spans {
        out.push(span.clone());
        let Some(fref) = &span.footnote_ref else {
            continue;
        };
        match footnote_markers.get(fref) {
            Some(marker) => out.push(marker_span(span, marker)),
            None => diagnostics.push(Diagnostic::warning(
                "footnote.unresolved_ref",
                format!(
                    "text node '{}': span footnote-ref '{}' matches no footnote \
                     on this page; no marker emitted",
                    text.id, fref
                ),
                text.source_span,
                Some(text.id.clone()),
            )),
        }
    }
    out
}

/// The superscript marker span that follows a footnote-ref span.
fn marker_span(ref_span: &TextSpan, marker: &str) -> TextSpan {
    TextSpan {
        text: marker.to_owned(),
        fill: ref_span.fill.clone(),
        font_weight: None,
        font_features: ref_span.font_features.clone(),
        font_alternates: ref_span.font_alternates.clone(),
        letter_spacing: ref_span.letter_spacing.clone(),
        italic: None,
        underline: None,
        strikethrough: None,
        vertical_align: Some("super".to_owned()),
        footnote_ref: None,
        data_ref: None,
        data_format: None,
        highlight: None,
        code: None,
        link: None,
    }
}

/// The resolved look of one span, shared by every sub-run it shapes into.
///
/// `font_size` is the span's own size (reduced for super/subscript) and
/// `baseline_dy` the baseline shift in px (negative = up). `vertical_align`
/// flags a super/subscript span so the fast path positions it against the
/// shared full-size baseline.
#[derive(Clone)]
pub(super) struct SpanLook {
    pub(super) color: Color,
    pub(super) underline: bool,
    pub(super) strikethrough: bool,
    /// Highlight background color (`None` = no highlight).
    pub(super) highlight: Option<Color>,
    /// `true` when the span was authored with `code=#true`.
    pub(super) code: bool,
    /// Hyperlink URL from `link="…"`.
    pub(super) link: Option<String>,
    pub(super) weight: u16,
    pub(super) style: FontStyle,
    pub(super) font_size: f32,
    pub(super) baseline_dy: f64,
    pub(super) letter_spacing_px: f32,
    pub(super) features: Vec<FontFeature>,
    pub(super) vertical_align: bool,
}

/// One shaped sub-run plus its span look.
///
/// `text` is the whole span text on the FIRST sub-run of a span and empty on
/// the rest, so the wrap path re-tokenizes whole words once. The fast path
/// ignores it.
pub(super) struct ShapedSpan {
    pub(super) run: ZenithGlyphRun,
    pub(super) text: String,
    pub(super) look: SpanLook,
}

impl ShapedSpan {
    /// The carrier the wrap path re-shapes from.
    pub(super) fn to_resolved(&self) -> ResolvedSpan {
        let look = &self.look;
        ResolvedSpan {
            text: self.text.clone(),
            color: look.color,
            underline: look.underline,
            strikethrough: look.strikethrough,
            highlight: look.highlight,
            code: look.code,
            link: look.link.clone(),
            weight: look.weight,
            style: look.style,
            font_size: look.font_size,
            baseline_dy: look.baseline_dy,
            letter_spacing_px: look.letter_spacing_px,
            features: look.features.clone(),
        }
    }
}

/// Every shaped sub-run of a node, in logical order.
pub(super) struct ShapedSet {
    pub(super) spans: Vec<ShapedSpan>,
    /// Sum of every run's advance width, in px.
    pub(super) total_advance: f64,
    /// Ascent of the first full-size run. Super/subscript spans sit on this
    /// shared baseline. `None` until a full-size span is shaped.
    pub(super) node_ascent: Option<f64>,
}

impl ShapedSet {
    /// Line height of the first shaped run (shared: font and size are fixed).
    pub(super) fn first_line_height(&self) -> f64 {
        self.spans
            .first()
            .map(|s| s.run.line_height as f64)
            .unwrap_or(0.0)
    }
}

/// Shape each non-empty span as its own run (per-glyph font fallback).
///
/// A span covered by the primary face yields one run. A mixed-script span
/// yields one run per same-face stretch, all with the span's look. Chars with
/// no glyph in any face push one `font.glyph_missing` warning for the node.
pub(super) fn shape_spans(
    style: &NodeStyle,
    spans: &[TextSpan],
    diagnostics: &mut Vec<Diagnostic>,
) -> ShapedSet {
    let text = style.text;
    let mut set = ShapedSet {
        spans: Vec::new(),
        total_advance: 0.0,
        node_ascent: None,
    };
    let mut missing: BTreeSet<char> = BTreeSet::new();

    for span in spans.iter().filter(|s| !s.text.is_empty()) {
        let look = resolve_span_look(style, span, diagnostics);
        // `code` spans use the bundled mono family instead of the node family.
        let mono_families;
        let families: &[String] = if look.code {
            mono_families = [CODE_MONO_FAMILY.to_owned()];
            &mono_families
        } else {
            style.families
        };
        let req = ShapeRequest {
            text: &span.text,
            families,
            weight: look.weight,
            style: look.style,
            font_size: look.font_size,
            direction: style.direction,
            features: &look.features,
            kerning_pairs: style.kerning_pairs,
            letter_spacing_px: look.letter_spacing_px,
        };
        match style.env.engine.shape_with_fallback(&req, style.env.fonts) {
            // Skip this span: the cursor does not advance.
            Err(e) => diagnostics.push(Diagnostic::advisory(
                "scene.text_unshaped",
                format!("text node '{}' could not be shaped: {}", text.id, e.message),
                text.source_span,
                Some(text.id.clone()),
            )),
            Ok(result) => {
                missing.extend(result.missing_chars);
                for (i, run) in result.runs.into_iter().enumerate() {
                    set.total_advance += run.advance_width as f64;
                    if !look.vertical_align && set.node_ascent.is_none() {
                        set.node_ascent = Some(run.ascent as f64);
                    }
                    let run_text = if i == 0 {
                        span.text.clone()
                    } else {
                        String::new()
                    };
                    set.spans.push(ShapedSpan {
                        run,
                        text: run_text,
                        look: look.clone(),
                    });
                }
            }
        }
    }

    emit_glyph_missing(diagnostics, &text.id, text.source_span, &missing);
    set
}

/// Resolve one span's look from the span, then the node cascade.
fn resolve_span_look(
    style: &NodeStyle,
    span: &TextSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> SpanLook {
    let text = style.text;
    let resolved = style.env.resolved;

    // Fill precedence: span fill, then the link color for a link span, then the
    // node fill, then black.
    let is_link = span.link.is_some();
    let mut color = span
        .fill
        .as_ref()
        .and_then(|fp| resolve_property_color(fp, resolved, diagnostics, &text.id))
        .or(is_link.then_some(LINK_COLOR))
        .or_else(|| {
            style
                .fill_prop
                .and_then(|fp| resolve_property_color(fp, resolved, diagnostics, &text.id))
        })
        .unwrap_or(Color::srgb(0, 0, 0, 255));
    color.a = (color.a as f64 * style.color_opacity).round() as u8;

    let highlight = span
        .highlight
        .as_ref()
        .and_then(|hp| resolve_property_color(hp, resolved, diagnostics, &text.id));

    let weight_prop = span.font_weight.as_ref().or(style.weight_prop);
    let weight = resolve_font_weight(weight_prop, resolved, 400);
    let font_style = if span.italic == Some(true) {
        FontStyle::Italic
    } else {
        FontStyle::Normal
    };

    // Super/subscript: a reduced size and a baseline shift against the node size.
    let (font_size, baseline_dy) =
        resolve_vertical_align(span.vertical_align.as_deref(), style.font_size);
    let features = match (
        span.font_features.as_deref(),
        span.font_alternates.as_deref(),
    ) {
        (None, None) => style.features.to_vec(),
        (span_features, span_alternates) => resolve_span_font_feature_set(
            style.features,
            span_features,
            span_alternates,
            diagnostics,
            &text.id,
            text.source_span,
        ),
    };
    let letter_spacing_px = resolve_letter_spacing(
        span.letter_spacing.as_ref().or(style.letter_spacing_prop),
        resolved,
    );

    SpanLook {
        color,
        // A link span is underlined by default, on top of an explicit underline.
        underline: span.underline == Some(true) || is_link,
        strikethrough: span.strikethrough == Some(true),
        highlight,
        code: span.code == Some(true),
        link: span.link.clone(),
        weight,
        style: font_style,
        font_size,
        baseline_dy,
        letter_spacing_px,
        features,
        vertical_align: baseline_dy != 0.0,
    }
}
