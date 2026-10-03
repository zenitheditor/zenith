//! The chart look: text typography and colour, axis and grid paint.
//!
//! Each value reads the chart attribute, then the chart `style=` key, then
//! the engine default. The `defaults` lowering has already written the
//! `defaults` chart row and the ambient content pair into the attribute or
//! a merged style, so this module sees one cascade.
//!
//! | Value | Source |
//! | --- | --- |
//! | font family | style `font-family`, else Noto Sans |
//! | base size | style `font-size`, else [`default_base`] |
//! | text ink | `fill`, else style `fill`, else [`default_ink`] |
//! | axis lines | `stroke`, else style `stroke`, else ink mixed 50% into the backdrop |
//! | gridlines | axis colour mixed 70% into the backdrop |
//! | line width | `stroke-width`, else style `stroke-width`, else 1px |

use std::collections::BTreeMap;

use zenith_core::{ChartNode, Diagnostic, PropertyValue, ResolvedToken, apca_lc, best_text_color};

use crate::ir::Color;

use super::super::NodeCtx;
use super::super::backdrop::blend;
use super::super::paint::{color_from_resolved, resolve_property_color};
use super::super::style_prop;
use super::super::text::resolve_font_family_name;
use super::super::util::resolve_property_dimension_px;

/// Fallback family, and the second family after a styled one.
const DEFAULT_FAMILY: &str = "Noto Sans";
/// Engine-default base size as a share of the shorter chart side.
const BASE_SHARE: f64 = 0.04;
/// Smallest engine-default base size, in px.
const MIN_BASE: f64 = 12.0;
/// Largest engine-default base size, in px.
const MAX_BASE: f64 = 28.0;
/// Smallest APCA |Lc| a declared `.content` token needs to win the default ink.
const MIN_TOKEN_LC: f64 = 60.0;
/// Share of the backdrop mixed into the ink for the default axis colour.
const AXIS_MIX: f64 = 0.5;
/// Share of the backdrop mixed into the axis colour for the gridlines.
const GRID_MIX: f64 = 0.7;

/// The resolved look of one chart.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ChartLook {
    /// Font families, preferred first.
    pub(super) families: Vec<String>,
    /// Base font size in px; each role scales it (see `ChartTextRole::scale`).
    pub(super) base: f64,
    /// Text colour of every role except value labels inside a mark.
    pub(super) ink: Color,
    /// Axis line colour.
    pub(super) axis: Color,
    /// Gridline colour.
    pub(super) grid: Color,
    /// Axis and gridline width in px.
    pub(super) line_w: f64,
}

/// Resolve the look of `chart`, drawn in a `w` × `h` box over `cx.backdrop`.
pub(super) fn chart_look(
    chart: &ChartNode,
    (w, h): (f64, f64),
    cx: NodeCtx,
    diagnostics: &mut Vec<Diagnostic>,
) -> ChartLook {
    let style = |key: &str| style_prop(&chart.style, cx.style_map, key);
    let own = |attr: Option<&'_ PropertyValue>, key: &str| -> Option<PropertyValue> {
        attr.or_else(|| style(key)).cloned()
    };

    let family = resolve_font_family_name(style("font-family"), cx.resolved, DEFAULT_FAMILY);
    let mut families = vec![family];
    if families.iter().all(|f| f != DEFAULT_FAMILY) {
        families.push(DEFAULT_FAMILY.to_owned());
    }

    let fallback_base = default_base(w, h);
    let base = match style("font-size") {
        Some(p) => resolve_property_dimension_px(Some(p), cx.resolved, fallback_base),
        None => fallback_base,
    };
    let base = if base.is_finite() && base > 0.0 {
        base
    } else {
        fallback_base
    };

    let backdrop = cx.backdrop;
    let ink = own(chart.fill.as_ref(), "fill")
        .and_then(|p| resolve_property_color(&p, cx.resolved, diagnostics, &chart.id))
        .unwrap_or_else(|| default_ink(backdrop, cx.resolved));
    let axis = own(chart.stroke.as_ref(), "stroke")
        .and_then(|p| resolve_property_color(&p, cx.resolved, diagnostics, &chart.id))
        .unwrap_or_else(|| blend(backdrop, ink, AXIS_MIX));
    let grid = blend(backdrop, axis, GRID_MIX);
    let line_w = own(chart.stroke_width.as_ref(), "stroke-width")
        .map(|p| resolve_property_dimension_px(Some(&p), cx.resolved, 1.0))
        .filter(|w| w.is_finite() && *w >= 0.0)
        .unwrap_or(1.0);

    ChartLook {
        families,
        base,
        ink,
        axis,
        grid,
        line_w,
    }
}

/// Engine-default base size for a `w` × `h` chart:
/// `clamp(round(min(w, h) × 0.04), 12, 28)` px.
pub(super) fn default_base(w: f64, h: f64) -> f64 {
    let side = w.min(h);
    if !side.is_finite() || side <= 0.0 {
        return MIN_BASE;
    }
    (side * BASE_SHARE).round().clamp(MIN_BASE, MAX_BASE)
}

/// Engine-default text ink over `backdrop`: the declared `.content` colour
/// token with the highest APCA |Lc| when it reaches Lc 60, else black or
/// white, whichever contrasts more. Ties keep the first token in id order.
pub(super) fn default_ink(backdrop: Color, resolved: &BTreeMap<String, ResolvedToken>) -> Color {
    let bg = (backdrop.r, backdrop.g, backdrop.b);
    let mut best: Option<(f64, Color)> = None;
    for (id, token) in resolved {
        if !id.ends_with(".content") {
            continue;
        }
        let Some(color) = color_from_resolved(&token.value) else {
            continue;
        };
        if color.a != 255 {
            continue;
        }
        let lc = apca_lc((color.r, color.g, color.b), bg).abs();
        if best.is_none_or(|(b, _)| lc > b) {
            best = Some((lc, color));
        }
    }
    match best {
        Some((lc, color)) if lc >= MIN_TOKEN_LC => color,
        Some(_) | None => black_or_white(backdrop),
    }
}

/// Black or white, whichever has the higher APCA |Lc| over `fill`. Used for
/// value labels drawn on a bar or slice.
pub(super) fn black_or_white(fill: Color) -> Color {
    let (r, g, b) = best_text_color((fill.r, fill.g, fill.b), (0, 0, 0), (255, 255, 255));
    Color::srgb(r, g, b, 255)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLACK: Color = Color::srgb(0, 0, 0, 255);
    const WHITE: Color = Color::srgb(255, 255, 255, 255);

    use zenith_core::{ResolvedValue, TokenType};

    fn color_token(id: &str, hex: &str) -> (String, ResolvedToken) {
        (
            id.to_owned(),
            ResolvedToken {
                token_type: TokenType::Color,
                value: ResolvedValue::Color(hex.to_owned()),
            },
        )
    }

    #[test]
    fn default_base_scales_with_the_shorter_side() {
        assert_eq!(default_base(400.0, 300.0), 12.0);
        assert_eq!(default_base(1200.0, 500.0), 20.0);
        assert_eq!(default_base(1600.0, 900.0), 28.0);
        assert_eq!(default_base(100.0, 80.0), 12.0);
        assert_eq!(default_base(0.0, 80.0), 12.0);
    }

    #[test]
    fn default_ink_is_black_or_white_without_tokens() {
        let none = BTreeMap::new();
        assert_eq!(default_ink(Color::srgb(255, 255, 255, 255), &none), BLACK);
        assert_eq!(default_ink(Color::srgb(12, 18, 40, 255), &none), WHITE);
    }

    #[test]
    fn default_ink_prefers_the_best_content_token() {
        let resolved: BTreeMap<String, ResolvedToken> = [
            color_token("color.base.content", "#0d1529"),
            color_token("color.primary.content", "#f5f7ff"),
            color_token("color.primary", "#000000"),
        ]
        .into_iter()
        .collect();
        let light = default_ink(Color::srgb(250, 250, 250, 255), &resolved);
        assert_eq!(light, Color::srgb(0x0d, 0x15, 0x29, 255));
        let dark = default_ink(Color::srgb(20, 20, 30, 255), &resolved);
        assert_eq!(dark, Color::srgb(0xf5, 0xf7, 0xff, 255));
    }

    #[test]
    fn on_fill_ink_contrasts_with_the_fill() {
        assert_eq!(black_or_white(Color::srgb(251, 188, 4, 255)), BLACK);
        assert_eq!(black_or_white(Color::srgb(40, 40, 120, 255)), WHITE);
    }
}
