//! Resolving a node's `fill` to a backdrop paint.

use std::collections::BTreeMap;

use crate::ast::style::Style;
use crate::ast::value::PropertyValue;
use crate::color::parse_rgb;
use crate::tokens::{ResolvedToken, ResolvedValue};

use super::props::style_property;
use super::types::{BackdropPaint, MIN_PAINT_ALPHA, PaintColor};

pub(super) fn resolve_fill_paint(
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
