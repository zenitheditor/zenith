//! Backend-neutral glyph outline extraction for editable text-to-path conversion.

use rustybuzz::ttf_parser;
use zenith_core::FontProvider;
use zenith_geometry::Point2;

use crate::{LayoutError, ZenithGlyphRun};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphOutlineRequest<'a> {
    pub font_bytes: &'a [u8],
    pub face_index: u32,
    pub glyph_id: u16,
    pub font_size: f32,
    pub origin: Point2,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlyphOutline {
    pub segments: Vec<GlyphOutlineSegment>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphRunOutlineRequest<'a> {
    pub run: &'a ZenithGlyphRun,
    pub origin: Point2,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlyphRunOutline {
    pub glyphs: Vec<OutlinedGlyph>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutlinedGlyph {
    pub glyph_index: usize,
    pub glyph_id: u16,
    pub text: String,
    pub origin: Point2,
    pub outline: GlyphOutline,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GlyphOutlineSegment {
    MoveTo(Point2),
    LineTo(Point2),
    CubicTo {
        ctrl1: Point2,
        ctrl2: Point2,
        to: Point2,
    },
    Close,
}

pub fn glyph_outline(
    request: GlyphOutlineRequest<'_>,
) -> Result<Option<GlyphOutline>, LayoutError> {
    validate_request(request)?;
    let face = parse_face(request.font_bytes, request.face_index)?;
    glyph_outline_with_face(&face, request.glyph_id, request.font_size, request.origin)
}

pub fn glyph_run_outline(
    request: GlyphRunOutlineRequest<'_>,
    provider: &dyn FontProvider,
) -> Result<Option<GlyphRunOutline>, LayoutError> {
    request
        .origin
        .validate()
        .map_err(|_| LayoutError::new("glyph run outline requires finite origin coordinates"))?;
    let font_data = provider.by_id(&request.run.font_id).ok_or_else(|| {
        LayoutError::new(format!(
            "no font resolved for glyph run id '{}'",
            request.run.font_id
        ))
    })?;
    let face = parse_face(&font_data.bytes, font_data.index)?;
    let mut glyphs = Vec::with_capacity(request.run.glyphs.len());

    for (glyph_index, glyph) in request.run.glyphs.iter().enumerate() {
        let origin = Point2::new(
            request.origin.x + f64::from(glyph.x),
            request.origin.y + f64::from(glyph.y),
        )
        .map_err(|_| LayoutError::new("glyph run outline requires finite origin coordinates"))?;
        let Some(outline) =
            glyph_outline_with_face(&face, glyph.glyph_id, request.run.font_size, origin)?
        else {
            continue;
        };
        glyphs.push(OutlinedGlyph {
            glyph_index,
            glyph_id: glyph.glyph_id,
            text: glyph.text.clone(),
            origin,
            outline,
        });
    }

    if glyphs.is_empty() {
        Ok(None)
    } else {
        Ok(Some(GlyphRunOutline { glyphs }))
    }
}

fn glyph_outline_with_face(
    face: &ttf_parser::Face<'_>,
    glyph_id: u16,
    font_size: f32,
    origin: Point2,
) -> Result<Option<GlyphOutline>, LayoutError> {
    validate_outline_geometry(font_size, origin)?;
    let units_per_em = face.units_per_em();
    if units_per_em == 0 {
        return Err(LayoutError::new(
            "font face reports units_per_em = 0 for glyph outline",
        ));
    }

    let scale = f64::from(font_size) / f64::from(units_per_em);
    let mut pen = GlyphOutlinePen::new(origin, scale);
    let glyph_id = ttf_parser::GlyphId(glyph_id);

    if face.outline_glyph(glyph_id, &mut pen).is_none() {
        return Ok(None);
    }
    Ok(Some(GlyphOutline {
        segments: pen.segments,
    }))
}

fn parse_face(bytes: &[u8], index: u32) -> Result<ttf_parser::Face<'_>, LayoutError> {
    ttf_parser::Face::parse(bytes, index)
        .map_err(|_| LayoutError::new("failed to parse font face for glyph outline"))
}

fn validate_request(request: GlyphOutlineRequest<'_>) -> Result<(), LayoutError> {
    if request.font_bytes.is_empty() {
        return Err(LayoutError::new(
            "glyph outline requires non-empty font bytes",
        ));
    }
    validate_outline_geometry(request.font_size, request.origin)
}

fn validate_outline_geometry(font_size: f32, origin: Point2) -> Result<(), LayoutError> {
    if !font_size.is_finite() || font_size <= 0.0 {
        return Err(LayoutError::new(
            "glyph outline requires a positive finite font size",
        ));
    }
    origin
        .validate()
        .map_err(|_| LayoutError::new("glyph outline requires finite origin coordinates"))?;
    Ok(())
}

struct GlyphOutlinePen {
    segments: Vec<GlyphOutlineSegment>,
    origin: Point2,
    scale: f64,
    current: Point2,
}

impl GlyphOutlinePen {
    fn new(origin: Point2, scale: f64) -> Self {
        Self {
            segments: Vec::new(),
            origin,
            scale,
            current: origin,
        }
    }

    fn map(&self, x: f32, y: f32) -> Point2 {
        Point2::new_unchecked(
            self.origin.x + f64::from(x) * self.scale,
            self.origin.y - f64::from(y) * self.scale,
        )
    }
}

impl ttf_parser::OutlineBuilder for GlyphOutlinePen {
    fn move_to(&mut self, x: f32, y: f32) {
        let point = self.map(x, y);
        self.segments.push(GlyphOutlineSegment::MoveTo(point));
        self.current = point;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let point = self.map(x, y);
        self.segments.push(GlyphOutlineSegment::LineTo(point));
        self.current = point;
    }

    fn quad_to(&mut self, ctrl_x: f32, ctrl_y: f32, x: f32, y: f32) {
        let ctrl = self.map(ctrl_x, ctrl_y);
        let to = self.map(x, y);
        let ctrl1 = Point2::new_unchecked(
            self.current.x + 2.0 / 3.0 * (ctrl.x - self.current.x),
            self.current.y + 2.0 / 3.0 * (ctrl.y - self.current.y),
        );
        let ctrl2 = Point2::new_unchecked(
            to.x + 2.0 / 3.0 * (ctrl.x - to.x),
            to.y + 2.0 / 3.0 * (ctrl.y - to.y),
        );
        self.segments
            .push(GlyphOutlineSegment::CubicTo { ctrl1, ctrl2, to });
        self.current = to;
    }

    fn curve_to(&mut self, ctrl1_x: f32, ctrl1_y: f32, ctrl2_x: f32, ctrl2_y: f32, x: f32, y: f32) {
        let ctrl1 = self.map(ctrl1_x, ctrl1_y);
        let ctrl2 = self.map(ctrl2_x, ctrl2_y);
        let to = self.map(x, y);
        self.segments
            .push(GlyphOutlineSegment::CubicTo { ctrl1, ctrl2, to });
        self.current = to;
    }

    fn close(&mut self) {
        self.segments.push(GlyphOutlineSegment::Close);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{FontData, FontProvider, FontStyle, default_provider};

    fn default_noto_sans_font() -> Result<FontData, LayoutError> {
        let provider = default_provider();
        provider
            .resolve(&["Noto Sans".to_owned()], 400, FontStyle::Normal)
            .ok_or_else(|| LayoutError::new("default font should resolve"))
    }

    #[test]
    fn outlines_shaped_glyph_as_backend_neutral_segments() -> Result<(), LayoutError> {
        let font = default_noto_sans_font()?;
        let outline = glyph_outline(GlyphOutlineRequest {
            font_bytes: &font.bytes,
            face_index: font.index,
            glyph_id: glyph_id_for("A"),
            font_size: 40.0,
            origin: Point2::new_unchecked(10.0, 50.0),
        })?
        .ok_or_else(|| LayoutError::new("latin glyph should have an outline"))?;

        assert!(!outline.segments.is_empty());
        assert!(
            outline
                .segments
                .iter()
                .any(|segment| matches!(segment, GlyphOutlineSegment::MoveTo(_)))
        );
        assert!(
            outline
                .segments
                .iter()
                .any(|segment| matches!(segment, GlyphOutlineSegment::CubicTo { .. }))
        );
        assert!(
            outline
                .segments
                .iter()
                .any(|segment| matches!(segment, GlyphOutlineSegment::Close))
        );
        Ok(())
    }

    #[test]
    fn outline_points_are_scaled_and_positioned() -> Result<(), LayoutError> {
        let font = default_noto_sans_font()?;
        let origin_outline = glyph_outline(GlyphOutlineRequest {
            font_bytes: &font.bytes,
            face_index: font.index,
            glyph_id: glyph_id_for("A"),
            font_size: 20.0,
            origin: Point2::new_unchecked(0.0, 0.0),
        })?
        .ok_or_else(|| LayoutError::new("latin glyph should have an outline"))?;
        let shifted_outline = glyph_outline(GlyphOutlineRequest {
            font_bytes: &font.bytes,
            face_index: font.index,
            glyph_id: glyph_id_for("A"),
            font_size: 20.0,
            origin: Point2::new_unchecked(7.0, 11.0),
        })?
        .ok_or_else(|| LayoutError::new("latin glyph should have an outline"))?;

        let origin = first_point(&origin_outline)
            .ok_or_else(|| LayoutError::new("outline should expose at least one point"))?;
        let shifted = first_point(&shifted_outline)
            .ok_or_else(|| LayoutError::new("outline should expose at least one point"))?;
        assert_close(shifted.x - origin.x, 7.0);
        assert_close(shifted.y - origin.y, 11.0);
        Ok(())
    }

    #[test]
    fn space_glyph_without_outline_returns_none() -> Result<(), LayoutError> {
        let font = default_noto_sans_font()?;

        let outline = glyph_outline(GlyphOutlineRequest {
            font_bytes: &font.bytes,
            face_index: font.index,
            glyph_id: glyph_id_for(" "),
            font_size: 20.0,
            origin: Point2::new_unchecked(0.0, 0.0),
        })?;

        assert_eq!(outline, None);
        Ok(())
    }

    #[test]
    fn outlines_shaped_run_with_glyph_offsets() -> Result<(), LayoutError> {
        let provider = default_provider();
        let engine = crate::RustybuzzEngine::new();
        let run = crate::TextLayoutEngine::shape(
            &engine,
            &crate::ShapeRequest {
                text: "A B",
                families: &["Noto Sans".to_owned()],
                weight: 400,
                style: FontStyle::Normal,
                font_size: 24.0,
                direction: crate::TextDirection::Ltr,
                features: &[],
            },
            &provider,
        )?;

        let outlined = glyph_run_outline(
            GlyphRunOutlineRequest {
                run: &run,
                origin: Point2::new_unchecked(100.0, 40.0),
            },
            &provider,
        )?
        .ok_or_else(|| LayoutError::new("mixed run should include drawable glyph outlines"))?;

        assert_eq!(outlined.glyphs.len(), 2);
        assert_eq!(outlined.glyphs[0].glyph_index, 0);
        assert_eq!(outlined.glyphs[0].glyph_id, run.glyphs[0].glyph_id);
        assert_eq!(outlined.glyphs[0].text, "A");
        assert_eq!(
            outlined.glyphs[0].origin,
            Point2::new_unchecked(100.0, 40.0)
        );
        assert_eq!(outlined.glyphs[1].glyph_index, 2);
        assert_eq!(outlined.glyphs[1].glyph_id, run.glyphs[2].glyph_id);
        assert_eq!(
            outlined.glyphs[1].origin,
            Point2::new_unchecked(100.0 + f64::from(run.glyphs[2].x), 40.0)
        );
        assert!(
            outlined
                .glyphs
                .iter()
                .all(|glyph| !glyph.outline.segments.is_empty())
        );
        Ok(())
    }

    #[test]
    fn glyph_run_outline_returns_none_for_space_only_run() -> Result<(), LayoutError> {
        let provider = default_provider();
        let engine = crate::RustybuzzEngine::new();
        let run = crate::TextLayoutEngine::shape(
            &engine,
            &crate::ShapeRequest {
                text: " ",
                families: &["Noto Sans".to_owned()],
                weight: 400,
                style: FontStyle::Normal,
                font_size: 24.0,
                direction: crate::TextDirection::Ltr,
                features: &[],
            },
            &provider,
        )?;

        let outlined = glyph_run_outline(
            GlyphRunOutlineRequest {
                run: &run,
                origin: Point2::new_unchecked(0.0, 0.0),
            },
            &provider,
        )?;

        assert_eq!(outlined, None);
        Ok(())
    }

    #[test]
    fn glyph_run_outline_reports_missing_font_id() {
        let provider = default_provider();
        let run = ZenithGlyphRun {
            font_id: "missing-font".to_owned(),
            font_size: 12.0,
            ascent: 0.0,
            descent: 0.0,
            line_height: 0.0,
            advance_width: 0.0,
            glyphs: Vec::new(),
        };

        let result = glyph_run_outline(
            GlyphRunOutlineRequest {
                run: &run,
                origin: Point2::new_unchecked(0.0, 0.0),
            },
            &provider,
        );

        assert!(result.is_err());
    }

    #[test]
    fn invalid_font_size_reports_error() {
        let result = glyph_outline(GlyphOutlineRequest {
            font_bytes: &[0],
            face_index: 0,
            glyph_id: 1,
            font_size: 0.0,
            origin: Point2::new_unchecked(0.0, 0.0),
        });

        assert!(result.is_err());
    }

    fn glyph_id_for(text: &str) -> u16 {
        let provider = default_provider();
        let engine = crate::RustybuzzEngine::new();
        let run = crate::TextLayoutEngine::shape(
            &engine,
            &crate::ShapeRequest {
                text,
                families: &["Noto Sans".to_owned()],
                weight: 400,
                style: FontStyle::Normal,
                font_size: 20.0,
                direction: crate::TextDirection::Ltr,
                features: &[],
            },
            &provider,
        )
        .expect("default font should shape");
        run.glyphs
            .first()
            .map(|glyph| glyph.glyph_id)
            .expect("shaped text should produce a glyph")
    }

    fn first_point(outline: &GlyphOutline) -> Option<Point2> {
        outline.segments.iter().find_map(|segment| match *segment {
            GlyphOutlineSegment::MoveTo(point)
            | GlyphOutlineSegment::LineTo(point)
            | GlyphOutlineSegment::CubicTo { to: point, .. } => Some(point),
            GlyphOutlineSegment::Close => None,
        })
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 1.0e-9,
            "expected {actual} to be within tolerance of {expected}"
        );
    }
}
