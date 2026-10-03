//! Glyph ink bounds: the tight box of a glyph's outline, in px.
//!
//! Line metrics (ascent, descent, line gap) describe the line BOX. The ink box
//! is what the glyph actually paints, so callers use it to decide whether
//! drawn text leaves a container. No third-party type escapes this module.

use rustybuzz::ttf_parser;

/// The ink box of one glyph, in px, relative to its pen origin on the
/// baseline. Positive x is rightward; positive y is downward (so `y_min` is
/// negative for ink above the baseline).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphInkBox {
    /// Leftmost ink x.
    pub x_min: f32,
    /// Topmost ink y (negative above the baseline).
    pub y_min: f32,
    /// Rightmost ink x.
    pub x_max: f32,
    /// Bottommost ink y (positive below the baseline).
    pub y_max: f32,
}

/// The ink box of `glyph_id` in the face held by `font_bytes`/`face_index`, at
/// `font_size` px. `None` when the face does not parse, reports
/// `units_per_em = 0`, or the glyph paints nothing (a space).
#[must_use]
pub fn glyph_ink_box(
    font_bytes: &[u8],
    face_index: u32,
    glyph_id: u16,
    font_size: f32,
) -> Option<GlyphInkBox> {
    let face = ttf_parser::Face::parse(font_bytes, face_index).ok()?;
    ink_box_with_face(&face, glyph_id, font_size)
}

/// [`glyph_ink_box`] over an already parsed face.
pub(crate) fn ink_box_with_face(
    face: &ttf_parser::Face<'_>,
    glyph_id: u16,
    font_size: f32,
) -> Option<GlyphInkBox> {
    let units_per_em = face.units_per_em();
    if units_per_em == 0 || !font_size.is_finite() {
        return None;
    }
    let scale = font_size / f32::from(units_per_em);
    let rect = face.glyph_bounding_box(ttf_parser::GlyphId(glyph_id))?;
    Some(GlyphInkBox {
        x_min: f32::from(rect.x_min) * scale,
        y_min: -f32::from(rect.y_max) * scale,
        x_max: f32::from(rect.x_max) * scale,
        y_max: -f32::from(rect.y_min) * scale,
    })
}

#[cfg(test)]
mod tests {
    use zenith_core::{FontProvider, FontStyle, default_provider};

    use super::*;
    use crate::engine::{ShapeRequest, TextDirection, TextLayoutEngine};
    use crate::rustybuzz_engine::{FontFaceStore, RustybuzzEngine};

    fn shape(text: &str, font_size: f32) -> crate::ZenithGlyphRun {
        let provider = default_provider();
        let store = FontFaceStore::new(&provider);
        let families = vec!["Noto Sans".to_owned()];
        RustybuzzEngine::new(&store)
            .shape(
                &ShapeRequest {
                    text,
                    families: &families,
                    weight: 400,
                    style: FontStyle::Normal,
                    font_size,
                    direction: TextDirection::Ltr,
                    features: &[],
                    kerning_pairs: &[],
                    letter_spacing_px: 0.0,
                },
                &provider,
            )
            .expect("bundled font shapes")
    }

    fn ink(run: &crate::ZenithGlyphRun, i: usize) -> Option<GlyphInkBox> {
        let font = default_provider().by_id(&run.font_id).expect("font id");
        glyph_ink_box(
            &font.bytes,
            font.index,
            run.glyphs[i].glyph_id,
            run.font_size,
        )
    }

    #[test]
    fn ink_sits_inside_the_line_box() {
        let run = shape("Hg", 64.0);
        let h = ink(&run, 0).expect("H paints");
        let g = ink(&run, 1).expect("g paints");
        assert!(
            h.y_min < 0.0 && h.y_max <= 0.5,
            "H sits on the baseline: {h:?}"
        );
        assert!(
            -h.y_min < run.ascent,
            "cap height is below the ascent: {h:?}"
        );
        assert!(g.y_max > 0.0, "g descends: {g:?}");
        assert!(
            g.y_max <= run.descent,
            "descender within the descent: {g:?}"
        );
    }

    #[test]
    fn ink_scales_with_font_size() {
        let a = ink(&shape("H", 10.0), 0).expect("H paints");
        let b = ink(&shape("H", 20.0), 0).expect("H paints");
        assert!((b.y_min - 2.0 * a.y_min).abs() < 1e-3);
    }

    #[test]
    fn space_paints_nothing() {
        assert!(ink(&shape(" ", 16.0), 0).is_none());
    }

    #[test]
    fn cached_engine_matches_the_free_function() {
        let provider = default_provider();
        let store = FontFaceStore::new(&provider);
        let engine = RustybuzzEngine::new(&store);
        let run = shape("Ag", 37.5);
        for (i, g) in run.glyphs.iter().enumerate() {
            let cached = engine.glyph_ink_box(&run.font_id, g.glyph_id, run.font_size, &provider);
            assert_eq!(cached, ink(&run, i));
        }
        assert!(
            engine
                .glyph_ink_box("no-such-font", 1, 16.0, &provider)
                .is_none()
        );
    }

    #[test]
    fn bad_bytes_yield_none() {
        assert!(glyph_ink_box(&[0u8; 8], 0, 1, 16.0).is_none());
    }
}
