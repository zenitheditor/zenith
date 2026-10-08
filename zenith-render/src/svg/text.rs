use super::{
    geometry::finite,
    paint::{Stroke, color},
    writer::Writer,
};
use crate::RenderError;
use zenith_core::FontProvider;
use zenith_scene::SceneCommand;

pub(super) fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\"', "&quot;")
        .replace('\'', "&apos;")
}

fn safe_link(link: &str) -> Result<(), RenderError> {
    if link.chars().any(char::is_control) {
        return Err(RenderError::new(
            "SVG hyperlink contains control characters",
        ));
    }
    if let Some((scheme, _)) = link.split_once(':')
        && !["http", "https", "mailto", "tel"]
            .iter()
            .any(|allowed| scheme.eq_ignore_ascii_case(allowed))
    {
        return Err(RenderError::new(format!(
            "unsupported SVG hyperlink scheme {scheme}"
        )));
    }
    Ok(())
}

pub(super) fn bitmap_glyphs(
    command: &SceneCommand,
    fonts: &dyn FontProvider,
) -> Result<bool, RenderError> {
    let SceneCommand::DrawGlyphRun {
        font_id,
        font_size,
        glyphs,
        link,
        ..
    } = command
    else {
        return Ok(false);
    };
    if let Some(link) = link {
        safe_link(link)?;
    }
    finite(&[f64::from(*font_size)])?;
    if *font_size <= 0.0 {
        return Err(RenderError::new("SVG font size must be positive"));
    }
    let font = fonts
        .by_id(font_id)
        .ok_or_else(|| RenderError::new(format!("unresolved SVG font {font_id}")))?;
    let face = ttf_parser::Face::parse(&font.bytes, font.index)
        .map_err(|error| RenderError::new(format!("invalid SVG font {font_id}: {error:?}")))?;
    let mut bitmap = false;
    for glyph in glyphs {
        let id = ttf_parser::GlyphId(glyph.glyph_id);
        if glyph.glyph_id >= face.number_of_glyphs() {
            return Err(RenderError::new(format!(
                "invalid SVG glyph {} in font {font_id}",
                glyph.glyph_id
            )));
        }
        finite(&[f64::from(glyph.dx), f64::from(glyph.dy)])?;
        match crate::glyph_bitmap::representation(&face, id, *font_size) {
            crate::glyph_bitmap::GlyphRepresentation::Png => bitmap = true,
            crate::glyph_bitmap::GlyphRepresentation::Outline
            | crate::glyph_bitmap::GlyphRepresentation::Empty => {}
            crate::glyph_bitmap::GlyphRepresentation::UnsupportedBitmap => {
                return Err(RenderError::new(format!(
                    "unsupported SVG bitmap glyph {} in font {font_id}; use PNG bitmap glyphs or an outline font",
                    glyph.glyph_id
                )));
            }
            crate::glyph_bitmap::GlyphRepresentation::UnsupportedGlyph => {
                return Err(RenderError::new(format!(
                    "unsupported SVG glyph {} in font {font_id}; use an outline font",
                    glyph.glyph_id
                )));
            }
        }
    }
    Ok(bitmap)
}

impl Writer {
    pub(super) fn text(
        &mut self,
        command: &SceneCommand,
        fonts: &dyn FontProvider,
    ) -> Result<(), RenderError> {
        let SceneCommand::DrawGlyphRun {
            x,
            y,
            font_id,
            font_size,
            color: c,
            stroke_color,
            stroke_width,
            link,
            glyphs,
            ..
        } = command
        else {
            return Ok(());
        };
        finite(&[*x, *y, stroke_width.unwrap_or(0.0)])?;
        bitmap_glyphs(command, fonts)?;
        let font = fonts
            .by_id(font_id)
            .ok_or_else(|| RenderError::new(format!("unresolved SVG font {font_id}")))?;
        let face = ttf_parser::Face::parse(&font.bytes, font.index)
            .map_err(|error| RenderError::new(format!("invalid SVG font {font_id}: {error:?}")))?;
        let scale = f64::from(*font_size) / f64::from(face.units_per_em());
        if let Some(link) = link {
            self.body
                .push_str(&format!("<a xlink:href=\"{}\">", escape(link)));
        }
        for glyph in glyphs {
            let mut pen = Pen {
                x: *x + f64::from(glyph.dx),
                y: *y + f64::from(glyph.dy),
                scale,
                d: String::new(),
            };
            face.outline_glyph(ttf_parser::GlyphId(glyph.glyph_id), &mut pen);
            let d = pen.d;
            self.body
                .push_str(&format!("<path d=\"{d}\" {}/>", color(*c, "fill")));
            if let Some(stroke) = stroke_color
                && stroke_width.unwrap_or(0.0) > 0.0
            {
                self.body.push_str(&format!(
                    "<path d=\"{d}\" {}/>",
                    Stroke::new(*stroke, stroke_width.unwrap_or(0.0)).attributes()?
                ));
            }
        }
        if link.is_some() {
            self.body.push_str("</a>");
        }
        Ok(())
    }
}

struct Pen {
    x: f64,
    y: f64,
    scale: f64,
    d: String,
}
impl Pen {
    fn point(&self, x: f32, y: f32) -> (f64, f64) {
        (
            self.x + f64::from(x) * self.scale,
            self.y - f64::from(y) * self.scale,
        )
    }
}
impl ttf_parser::OutlineBuilder for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.d.push_str(&format!("M{x} {y}"));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.d.push_str(&format!("L{x} {y}"));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (a, b) = self.point(x1, y1);
        let (x, y) = self.point(x, y);
        self.d.push_str(&format!("Q{a} {b} {x} {y}"));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (a, b) = self.point(x1, y1);
        let (c, d) = self.point(x2, y2);
        let (x, y) = self.point(x, y);
        self.d.push_str(&format!("C{a} {b} {c} {d} {x} {y}"));
    }
    fn close(&mut self) {
        self.d.push('Z');
    }
}
