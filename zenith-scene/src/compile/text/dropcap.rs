//! Drop-cap initial: lift the first body character, size it so its cap-height
//! spans the requested line count, and shape the oversized glyph for emission.

use zenith_core::{FontProvider, FontStyle};
use zenith_layout::{
    RustybuzzEngine, ShapeRequest, TextDirection, TextLayoutEngine, ZenithGlyphRun,
};

use crate::ir::Color;

use super::shape::ResolvedSpan;

/// Horizontal gap between the drop-cap glyph's right edge and the wrapped body
/// text, as a fraction of the BODY font size. `0.25 ×` body size is a compact,
/// deterministic default.
pub(in crate::compile) const DROPCAP_GAP_FACTOR: f64 = 0.25;

/// Cap-height as a fraction of em (font size) used to SIZE the drop cap so its
/// cap-height — not its full ascent — spans the requested lines. Latin cap
/// height is ≈ `0.7 × em` (Noto Sans `sCapHeight` is 714/1000); the value
/// cancels in the cap-top alignment (both the cap and the body use it), so the
/// exact figure only affects the cap's optical size, not its alignment.
const CAP_HEIGHT_RATIO: f64 = 0.714;

/// A shaped drop-cap initial ready for emission.
pub(in crate::compile) struct DropCap {
    /// The oversized shaped glyph run for the initial.
    pub(in crate::compile) run: ZenithGlyphRun,
    /// Pen advance of the oversized run (used as the body indent base).
    pub(in crate::compile) advance: f64,
    /// Resolved node color the cap paints with.
    pub(in crate::compile) color: Color,
    /// Number of body lines the cap spans (the narrow-line count).
    pub(in crate::compile) lines: usize,
}

/// The initial lifted out of the body for a drop cap, plus the donor span's
/// visual attributes, BEFORE the oversized glyph is shaped.
pub(in crate::compile) struct DropCapInitial {
    ch: char,
    color: Color,
    style: FontStyle,
}

/// Lift the first character out of the body spans for a drop cap.
///
/// The first NON-EMPTY resolved span donates its leading `char` (the v0 grapheme
/// unit — combining sequences are a documented follow-up); that span's text is
/// rewritten WITHOUT the initial so the body wrap re-tokenizes only the
/// remainder. Returns `None` (leaving the spans untouched) when no span carries
/// a character, so an empty-text node with the attribute never panics and draws
/// no cap. The donor color/style are captured for the cap glyph.
pub(in crate::compile) fn take_drop_cap_initial(
    spans: &mut [ResolvedSpan],
) -> Option<DropCapInitial> {
    let donor = spans.iter_mut().find(|s| !s.text.is_empty())?;
    let first = donor.text.chars().next()?;
    // Strip the initial from the body (it is now drawn by the cap).
    donor.text = donor.text.chars().skip(1).collect();
    Some(DropCapInitial {
        ch: first,
        color: donor.color,
        style: donor.style,
    })
}

/// Compute the drop-cap glyph SIZE so its cap-height spans `(n-1)` body lines
/// plus the body's own cap-height: `body_size + (n-1) * line_height /
/// CAP_HEIGHT_RATIO`. Paired with a baseline on line `n`'s baseline (emit site),
/// this aligns the cap's cap-top with line 1's cap-top and its baseline with
/// line `n`'s baseline — the standard drop-cap geometry.
pub(in crate::compile) fn drop_cap_font_size(body_font_size: f64, line_height: f64, n: u32) -> f32 {
    (body_font_size + (n as f64 - 1.0) * line_height / CAP_HEIGHT_RATIO).max(1.0) as f32
}

/// Shape a lifted [`DropCapInitial`] as an oversized glyph at `cap_size` (see
/// [`drop_cap_font_size`]), spanning `n` lines. It paints in the donor span's
/// color and the node family. `None` on a shaping failure → no cap, body
/// unchanged.
pub(in crate::compile) fn shape_drop_cap(
    initial: &DropCapInitial,
    families: &[String],
    weight: u16,
    cap_size: f32,
    n: u32,
    engine: &RustybuzzEngine,
    fonts: &dyn FontProvider,
) -> Option<DropCap> {
    let glyph = initial.ch.to_string();
    let req = ShapeRequest {
        text: &glyph,
        families,
        weight,
        style: initial.style,
        font_size: cap_size,
        // Drop caps are a single glyph; RTL drop caps are a documented v0
        // follow-up, so the cap always shapes LTR.
        direction: TextDirection::Ltr,
        features: &[],
    };
    let run = engine
        .shape_with_fallback(&req, fonts)
        .ok()?
        .runs
        .into_iter()
        .next()?;
    let advance = run.advance_width as f64;
    Some(DropCap {
        run,
        advance,
        color: initial.color,
        lines: n as usize,
    })
}
