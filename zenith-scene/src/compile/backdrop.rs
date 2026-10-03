//! The backdrop: the solid colour a node draws over, as the structural fills
//! around it imply.
//!
//! The page background starts the backdrop. A `frame` fill and a `table`,
//! header, or cell fill replace it for their children. A translucent fill
//! (alpha or node opacity under 1) blends over the parent backdrop. A
//! gradient counts as the plain average of its stops. A fill that does not
//! resolve keeps the parent backdrop. Free-standing shapes (a `rect` behind a
//! node) do not count: the backdrop follows structure only, like content
//! pairing in the `defaults` lowering.
//!
//! Chart text reads it to choose an engine-default ink. Resolution here emits
//! no diagnostics: the paint path that draws the fill reports them.

use std::collections::BTreeMap;

use zenith_core::{PropertyValue, ResolvedToken};

use crate::color::parse_color;
use crate::ir::Color;

use super::paint::{color_from_resolved, resolve_property_gradient};

/// Backdrop of a page with no resolvable background: white paper.
pub(in crate::compile) const PAPER: Color = Color::srgb(255, 255, 255, 255);

/// The backdrop a page's content draws over.
pub(in crate::compile) fn page_backdrop(
    background: Option<&PropertyValue>,
    resolved: &BTreeMap<String, ResolvedToken>,
) -> Color {
    fill_backdrop(background, 1.0, PAPER, resolved)
}

/// The backdrop the children of a node with `fill` (at node `opacity`) draw
/// over, given the `parent` backdrop.
pub(in crate::compile) fn fill_backdrop(
    fill: Option<&PropertyValue>,
    opacity: f64,
    parent: Color,
    resolved: &BTreeMap<String, ResolvedToken>,
) -> Color {
    let Some(fill) = fill else {
        return parent;
    };
    let Some(color) = solid_of(fill, resolved) else {
        return parent;
    };
    let alpha = f64::from(color.a) / 255.0 * opacity.clamp(0.0, 1.0);
    blend(color, parent, alpha)
}

/// The single colour a fill paints: the colour itself, or the mean of a
/// gradient's stops.
fn solid_of(fill: &PropertyValue, resolved: &BTreeMap<String, ResolvedToken>) -> Option<Color> {
    if let Some(gradient) = resolve_property_gradient(fill, resolved, "") {
        let n = gradient.stops.len() as f64;
        if n <= 0.0 {
            return None;
        }
        let mean = |f: fn(&Color) -> u8| -> u8 {
            let sum: f64 = gradient.stops.iter().map(|s| f64::from(f(&s.color))).sum();
            channel(sum / n)
        };
        return Some(Color::srgb(
            mean(|c| c.r),
            mean(|c| c.g),
            mean(|c| c.b),
            mean(|c| c.a),
        ));
    }
    match fill {
        PropertyValue::TokenRef(id) => color_from_resolved(&resolved.get(id.as_str())?.value),
        PropertyValue::Literal(literal) => parse_color(literal),
        PropertyValue::Dimension(_) | PropertyValue::DataRef(_) => None,
    }
}

/// `top` at coverage `alpha` over the opaque `bottom`. The result is opaque.
pub(in crate::compile) fn blend(top: Color, bottom: Color, alpha: f64) -> Color {
    let a = alpha.clamp(0.0, 1.0);
    let mix = |t: u8, b: u8| channel(f64::from(t) * a + f64::from(b) * (1.0 - a));
    Color::srgb(
        mix(top.r, bottom.r),
        mix(top.g, bottom.g),
        mix(top.b, bottom.b),
        255,
    )
}

/// A channel value rounded into `0..=255`.
fn channel(v: f64) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_fill_keeps_the_parent() {
        let parent = Color::srgb(10, 20, 30, 255);
        assert_eq!(fill_backdrop(None, 1.0, parent, &BTreeMap::new()), parent);
    }

    #[test]
    fn literal_fill_replaces_the_parent() {
        let fill = PropertyValue::Literal("#102030".to_owned());
        let got = fill_backdrop(Some(&fill), 1.0, PAPER, &BTreeMap::new());
        assert_eq!(got, Color::srgb(0x10, 0x20, 0x30, 255));
    }

    #[test]
    fn half_opacity_blends_over_the_parent() {
        let fill = PropertyValue::Literal("#000000".to_owned());
        let got = fill_backdrop(Some(&fill), 0.5, PAPER, &BTreeMap::new());
        assert_eq!(got, Color::srgb(128, 128, 128, 255));
    }

    #[test]
    fn unresolved_token_keeps_the_parent() {
        let fill = PropertyValue::TokenRef("color.missing".to_owned());
        assert_eq!(
            fill_backdrop(Some(&fill), 1.0, PAPER, &BTreeMap::new()),
            PAPER
        );
    }
}
