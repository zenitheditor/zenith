//! Resolution of a chain source's shared render style (families/size/weight
//! and the full per-span style).

use std::collections::BTreeMap;

use zenith_core::{
    Diagnostic, FontProvider, FontStyle, PropertyValue, ResolvedToken, Style, TextNode,
};

use crate::compile::paint::resolve_property_color;
use crate::compile::style_prop;
use crate::compile::text::{
    LINK_COLOR, ResolvedSpan, resolve_family_with_fallback, resolve_font_family_name,
    resolve_font_feature_set, resolve_font_weight, resolve_letter_spacing,
    resolve_span_font_feature_set, resolve_vertical_align,
};
use crate::compile::util::resolve_property_dimension_px;
use crate::ir::Color;

/// Resolve only the chain source's shared base render style: `families`,
/// `font_size`, and the node base `weight`. Does NOT build [`ResolvedSpan`]s, so
/// it is cheap enough to call on the block path where the per-span resolution
/// would allocate a [`Vec<ResolvedSpan>`] that is immediately discarded.
pub(super) fn resolve_chain_base_style(
    source: &TextNode,
    resolved: &BTreeMap<String, ResolvedToken>,
    style_map: &BTreeMap<&str, &Style>,
    fonts: &dyn FontProvider,
    diagnostics: &mut Vec<Diagnostic>,
) -> (Vec<String>, f32, u16) {
    let font_family_prop = source
        .font_family
        .as_ref()
        .or_else(|| style_prop(&source.style, style_map, "font-family"));
    let raw_family_name = resolve_font_family_name(font_family_prop, resolved, "Noto Sans");
    let (family_name, fell_back, is_local) =
        resolve_family_with_fallback(fonts, &raw_family_name, "Noto Sans", 400, FontStyle::Normal);
    if fell_back {
        diagnostics.push(
            Diagnostic::advisory(
                "font.unresolved",
                format!(
                    "text node '{}': font family '{}' not available, falling back to 'Noto Sans'",
                    source.id, raw_family_name
                ),
                source.source_span,
                Some(source.id.clone()),
            )
            .with_cause(format!(
                "font family '{raw_family_name}' not available, falling back to 'Noto Sans'"
            )),
        );
    }
    if is_local {
        diagnostics.push(Diagnostic::advisory(
            "font.local",
            format!(
                "text node '{}': font family '{}' resolved from a local/system font; rendering is \
                 NOT guaranteed deterministic across machines — bundle the font or guarantee the \
                 target OS provides it",
                source.id, raw_family_name
            ),
            source.source_span,
            Some(source.id.clone()),
        ));
    }
    let families = vec![family_name];

    let font_size_prop = source
        .font_size
        .clone()
        .or_else(|| style_prop(&source.style, style_map, "font-size").cloned());
    let font_size: f32 =
        resolve_property_dimension_px(font_size_prop.as_ref(), resolved, 16.0) as f32;

    let node_weight_prop: Option<&PropertyValue> = source
        .font_weight
        .as_ref()
        .or_else(|| style_prop(&source.style, style_map, "font-weight"));
    let base_weight = resolve_font_weight(node_weight_prop, resolved, 400);

    (families, font_size, base_weight)
}

/// Resolve the chain source's shared render style into `families`, `font_size`,
/// the node base weight, and the per-span [`ResolvedSpan`] carriers used for
/// shaping. Mirrors `compile_text`'s resolution at opacity 1.0 (v0: no cascade).
pub(super) fn resolve_chain_style(
    source: &TextNode,
    resolved: &BTreeMap<String, ResolvedToken>,
    style_map: &BTreeMap<&str, &Style>,
    fonts: &dyn FontProvider,
    diagnostics: &mut Vec<Diagnostic>,
) -> (Vec<String>, f32, u16, f32, Vec<ResolvedSpan>) {
    let (families, font_size, base_weight) =
        resolve_chain_base_style(source, resolved, style_map, fonts, diagnostics);

    // Node-level fill/weight fallbacks (span override → node → style → default).
    let node_fill_prop: Option<&PropertyValue> = source
        .fill
        .as_ref()
        .or_else(|| style_prop(&source.style, style_map, "fill"));
    let node_weight_prop: Option<&PropertyValue> = source
        .font_weight
        .as_ref()
        .or_else(|| style_prop(&source.style, style_map, "font-weight"));
    let node_features = resolve_font_feature_set(
        source.font_features.as_deref(),
        source.font_alternates.as_deref(),
        diagnostics,
        &source.id,
        source.source_span,
    );
    let node_letter_spacing_prop = source
        .letter_spacing
        .as_ref()
        .or_else(|| style_prop(&source.style, style_map, "letter-spacing"));
    let node_letter_spacing_px = resolve_letter_spacing(node_letter_spacing_prop, resolved);

    let mut spans: Vec<ResolvedSpan> = Vec::new();
    for span in &source.spans {
        if span.text.is_empty() {
            continue;
        }
        // Per-span fill precedence: span-level `fill` > `link` color > inherited
        // node fill > black. A link's conventional color overrides an inherited
        // node fill but not a fill set directly on the span. Non-link spans keep
        // the prior `span.fill else node.fill else black` resolution (byte-identical).
        let is_link = span.link.is_some();
        let color = span
            .fill
            .as_ref()
            .and_then(|fp| resolve_property_color(fp, resolved, diagnostics, &source.id))
            .or(is_link.then_some(LINK_COLOR))
            .or_else(|| {
                node_fill_prop
                    .and_then(|fp| resolve_property_color(fp, resolved, diagnostics, &source.id))
            })
            .unwrap_or(Color::srgb(0, 0, 0, 255));
        // Per-span highlight background color (token ref or raw color string).
        // Absent → `None` (no highlight, byte-identical to a span without it).
        let highlight: Option<Color> = span
            .highlight
            .as_ref()
            .and_then(|hp| resolve_property_color(hp, resolved, diagnostics, &source.id));
        // `code` span: bool flag that drives mono-family shaping + bg rect.
        let code = span.code == Some(true);
        // `link` span: URL retained for future annotation use.
        let link = span.link.clone();
        let weight_prop = span.font_weight.as_ref().or(node_weight_prop);
        let weight = resolve_font_weight(weight_prop, resolved, 400);
        let style = if span.italic == Some(true) {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        };
        // Super/subscript: reduced size + baseline shift, shared with the
        // single-box wrap path so a chained article honors vertical-align too.
        let (span_font_size, baseline_dy) =
            resolve_vertical_align(span.vertical_align.as_deref(), font_size);
        let features = match (
            span.font_features.as_deref(),
            span.font_alternates.as_deref(),
        ) {
            (None, None) => node_features.clone(),
            (span_features, span_alternates) => resolve_span_font_feature_set(
                &node_features,
                span_features,
                span_alternates,
                diagnostics,
                &source.id,
                source.source_span,
            ),
        };
        let span_letter_spacing_px = resolve_letter_spacing(
            span.letter_spacing.as_ref().or(node_letter_spacing_prop),
            resolved,
        );
        spans.push(ResolvedSpan {
            text: span.text.clone(),
            color,
            // `link` spans are underlined by default; explicit underline OR-ed in.
            underline: span.underline == Some(true) || is_link,
            strikethrough: span.strikethrough == Some(true),
            highlight,
            code,
            link,
            weight,
            style,
            font_size: span_font_size,
            baseline_dy,
            letter_spacing_px: span_letter_spacing_px,
            features,
        });
    }

    (
        families,
        font_size,
        base_weight,
        node_letter_spacing_px,
        spans,
    )
}
