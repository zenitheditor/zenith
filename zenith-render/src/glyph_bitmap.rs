//! Shared embedded-bitmap preference and outline availability.

use zenith_core::FontProvider;
use zenith_scene::SceneCommand;

pub(crate) enum GlyphRepresentation {
    Png,
    Outline,
    Empty,
    UnsupportedBitmap,
    UnsupportedGlyph,
}

pub(crate) fn representation(
    face: &ttf_parser::Face<'_>,
    glyph: ttf_parser::GlyphId,
    size: f32,
) -> GlyphRepresentation {
    let raster = face.glyph_raster_image(glyph, size as u16);
    if decodable_png(raster) {
        return GlyphRepresentation::Png;
    }
    if face.outline_glyph(glyph, &mut OutlinePresence).is_some() {
        return GlyphRepresentation::Outline;
    }
    if raster.is_some() {
        return GlyphRepresentation::UnsupportedBitmap;
    }
    if face.glyph_bounding_box(glyph).is_some() || face.glyph_svg_image(glyph).is_some() {
        return GlyphRepresentation::UnsupportedGlyph;
    }
    GlyphRepresentation::Empty
}

pub(crate) fn preferred_png_glyph(
    face: &ttf_parser::Face<'_>,
    glyph: ttf_parser::GlyphId,
    size: f32,
) -> bool {
    decodable_png(face.glyph_raster_image(glyph, size as u16))
}

fn decodable_png(image: Option<ttf_parser::RasterGlyphImage<'_>>) -> bool {
    image.is_some_and(|image| {
        image.format == ttf_parser::RasterImageFormat::PNG
            && image.pixels_per_em > 0
            && tiny_skia::Pixmap::decode_png(image.data).is_ok()
    })
}

pub(crate) fn preferred_png(command: &SceneCommand, fonts: &dyn FontProvider) -> bool {
    let SceneCommand::DrawGlyphRun {
        font_id,
        font_size,
        glyphs,
        ..
    } = command
    else {
        return false;
    };
    let Some(font) = fonts.by_id(font_id) else {
        return false;
    };
    let Ok(face) = ttf_parser::Face::parse(&font.bytes, font.index) else {
        return false;
    };
    glyphs
        .iter()
        .any(|glyph| preferred_png_glyph(&face, ttf_parser::GlyphId(glyph.glyph_id), *font_size))
}

struct OutlinePresence;
impl ttf_parser::OutlineBuilder for OutlinePresence {
    fn move_to(&mut self, _x: f32, _y: f32) {}
    fn line_to(&mut self, _x: f32, _y: f32) {}
    fn quad_to(&mut self, _x1: f32, _y1: f32, _x: f32, _y: f32) {}
    fn curve_to(&mut self, _x1: f32, _y1: f32, _x2: f32, _y2: f32, _x: f32, _y: f32) {}
    fn close(&mut self) {}
}
