//! Color translation for the PDF backend.
//!
//! A scene [`Color`] carries straight-alpha sRGB channels plus an optional
//! original CMYK quad. CMYK-origin colors emit native DeviceCMYK (`k` / `K`)
//! so the press receives the authored separations untouched; sRGB-origin colors
//! emit DeviceRGB (`rg` / `RG`). Alpha below opaque is honored via a named
//! ExtGState carrying `ca` (fill) / `CA` (stroke) — the content translator
//! interns the alpha byte and the document writer materializes the matching
//! `/ExtGState` resource.

use pdf_writer::Content;
use zenith_scene::Color;

/// Convert linear-range sRGB channels (`0.0..=1.0`) to DeviceCMYK (`0.0..=1.0`)
/// with clean black generation (pure grays stay K-only).
pub(super) fn rgb_to_cmyk(r: f32, g: f32, b: f32) -> [f32; 4] {
    let r = r.clamp(0.0, 1.0);
    let g = g.clamp(0.0, 1.0);
    let b = b.clamp(0.0, 1.0);
    let k = 1.0 - r.max(g).max(b);
    if k >= 1.0 - 1e-6 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let inv = 1.0 - k;
    let c = ((1.0 - r - k) / inv).clamp(0.0, 1.0);
    let m = ((1.0 - g - k) / inv).clamp(0.0, 1.0);
    let y = ((1.0 - b - k) / inv).clamp(0.0, 1.0);
    [c, m, y, k.clamp(0.0, 1.0)]
}

/// Set the non-stroking (fill) color from `color`, choosing DeviceCMYK for a
/// CMYK-origin color or when `cmyk` output mode is active, and DeviceRGB otherwise.
pub(super) fn set_fill(content: &mut Content, color: &Color, cmyk: bool) {
    match color.cmyk {
        Some([c, m, y, k]) => {
            content.set_fill_cmyk(c / 100.0, m / 100.0, y / 100.0, k / 100.0);
        }
        None if cmyk => {
            let [c, m, y, k] = rgb_to_cmyk(
                f32::from(color.r) / 255.0,
                f32::from(color.g) / 255.0,
                f32::from(color.b) / 255.0,
            );
            content.set_fill_cmyk(c, m, y, k);
        }
        None => {
            content.set_fill_rgb(
                f32::from(color.r) / 255.0,
                f32::from(color.g) / 255.0,
                f32::from(color.b) / 255.0,
            );
        }
    }
}

/// Set the stroking color from `color` (see [`set_fill`]).
pub(super) fn set_stroke(content: &mut Content, color: &Color, cmyk: bool) {
    match color.cmyk {
        Some([c, m, y, k]) => {
            content.set_stroke_cmyk(c / 100.0, m / 100.0, y / 100.0, k / 100.0);
        }
        None if cmyk => {
            let [c, m, y, k] = rgb_to_cmyk(
                f32::from(color.r) / 255.0,
                f32::from(color.g) / 255.0,
                f32::from(color.b) / 255.0,
            );
            content.set_stroke_cmyk(c, m, y, k);
        }
        None => {
            content.set_stroke_rgb(
                f32::from(color.r) / 255.0,
                f32::from(color.g) / 255.0,
                f32::from(color.b) / 255.0,
            );
        }
    }
}
