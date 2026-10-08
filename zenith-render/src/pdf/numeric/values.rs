//! Numeric errors retain page, command, field, and rejected value.

use super::transform::TransformStack;
use crate::RenderError;
use zenith_scene::ir::PathSegment;
use zenith_scene::{Color, Paint};

#[derive(Clone, Copy)]
pub(super) struct Check {
    pub page: usize,
    pub command: usize,
}
impl Check {
    pub fn finite(self, field: &str, value: f64) -> Result<(), RenderError> {
        if value.is_finite() {
            Ok(())
        } else {
            Err(self.error(field, value))
        }
    }
    pub fn coordinate(self, field: &str, value: f64) -> Result<(), RenderError> {
        if value.is_finite() && (value as f32).is_finite() {
            Ok(())
        } else {
            Err(self.error(field, value))
        }
    }
    fn error(self, field: &str, value: f64) -> RenderError {
        RenderError::new(format!(
            "PDF page {} command {} numeric error: {field}={value}. Supply finite supported coordinates",
            self.page, self.command
        ))
    }
    pub fn option(self, field: &str, value: Option<f64>) -> Result<(), RenderError> {
        match value {
            Some(value) => self.coordinate(field, value),
            None => Ok(()),
        }
    }
    pub fn color(self, field: &str, color: &Color) -> Result<(), RenderError> {
        if let Some(channels) = color.cmyk {
            for (index, value) in channels.into_iter().enumerate() {
                self.coordinate(&format!("{field}.cmyk[{index}]"), f64::from(value))?;
            }
        }
        Ok(())
    }
    pub fn point(
        self,
        transform: &TransformStack,
        field: &str,
        x: f64,
        y: f64,
    ) -> Result<(), RenderError> {
        self.coordinate(&format!("{field}.x"), x)?;
        self.coordinate(&format!("{field}.y"), y)?;
        transform.point(self, field, x as f32, y as f32)
    }
    pub fn rect(
        self,
        transform: &TransformStack,
        field: &str,
        rect: (f64, f64, f64, f64),
    ) -> Result<(), RenderError> {
        let (x, y, w, h) = rect;
        self.coordinate(&format!("{field}.w"), w)?;
        self.coordinate(&format!("{field}.h"), h)?;
        self.point(transform, field, x, y)?;
        self.point(transform, &format!("{field}.end"), x + w, y + h)?;
        self.point(transform, &format!("{field}.corner"), x + w, y)?;
        self.point(transform, &format!("{field}.corner"), x, y + h)?;
        self.coordinate(
            &format!("{field}.end.x.f32"),
            f64::from(x as f32 + w as f32),
        )?;
        self.coordinate(
            &format!("{field}.end.y.f32"),
            f64::from(y as f32 + h as f32),
        )
    }
    pub fn stroke(
        self,
        transform: &TransformStack,
        width: f64,
        doubled: bool,
    ) -> Result<(), RenderError> {
        self.coordinate("stroke_width", width)?;
        let width = if doubled { width * 2.0 } else { width };
        self.coordinate("aligned_stroke_width", width)?;
        transform.stroke(self, width)
    }
    pub fn path(
        self,
        transform: &TransformStack,
        segments: &[PathSegment],
    ) -> Result<(), RenderError> {
        for (index, segment) in segments.iter().enumerate() {
            let field = format!("segments[{index}]");
            match segment {
                PathSegment::MoveTo { x, y } | PathSegment::LineTo { x, y } => {
                    self.point(transform, &field, *x, *y)?
                }
                PathSegment::CubicTo {
                    x1,
                    y1,
                    x2,
                    y2,
                    x,
                    y,
                } => {
                    self.point(transform, &format!("{field}.control1"), *x1, *y1)?;
                    self.point(transform, &format!("{field}.control2"), *x2, *y2)?;
                    self.point(transform, &field, *x, *y)?;
                }
                PathSegment::Close => {}
            }
        }
        Ok(())
    }
    pub fn paint(
        self,
        transform: &TransformStack,
        paint: &Paint,
        bbox: Option<(f64, f64, f64, f64)>,
    ) -> Result<(), RenderError> {
        match paint {
            Paint::Solid { color } => self.color("paint", color),
            Paint::Gradient(gradient) => {
                if gradient.radial {
                    for (field, value) in [
                        ("gradient.center_x", gradient.center_x),
                        ("gradient.center_y", gradient.center_y),
                        ("gradient.radius_frac", gradient.radius_frac),
                    ] {
                        if let Some(value) = value {
                            self.finite(field, value)?;
                        }
                    }
                } else {
                    self.finite("gradient.angle_deg", gradient.angle_deg)?;
                }
                for (index, stop) in gradient.stops.iter().enumerate() {
                    self.finite(&format!("gradient.stops[{index}].offset"), stop.offset)?;
                    self.color(&format!("gradient.stops[{index}].color"), &stop.color)?;
                }
                if let Some((x, y, w, h)) = bbox {
                    match crate::pdf::gradient::geometry_coordinates(x, y, w, h, gradient) {
                        crate::pdf::gradient::GradientGeometry::Axial(values) => {
                            for (index, value) in values.into_iter().enumerate() {
                                self.coordinate(
                                    &format!("gradient.coordinates[{index}]"),
                                    f64::from(value),
                                )?;
                            }
                            for &[x, y] in values.as_chunks::<2>().0 {
                                self.point(
                                    transform,
                                    "gradient.endpoint",
                                    f64::from(x),
                                    f64::from(y),
                                )?;
                            }
                        }
                        crate::pdf::gradient::GradientGeometry::Radial(values) => {
                            for (index, value) in values.into_iter().enumerate() {
                                self.coordinate(
                                    &format!("gradient.coordinates[{index}]"),
                                    f64::from(value),
                                )?;
                            }
                            for &[cx, cy, radius] in values.as_chunks::<3>().0 {
                                self.point(
                                    transform,
                                    "gradient.circle",
                                    f64::from(cx),
                                    f64::from(cy),
                                )?;
                                self.coordinate("gradient.radius", f64::from(radius))?;
                            }
                        }
                    }
                }
                Ok(())
            }
        }
    }
}
