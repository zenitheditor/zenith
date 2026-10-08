//! Transform composition uses the backend's f32 affine arithmetic.

use super::values::Check;
use crate::RenderError;
use tiny_skia::{Point, Transform};

pub(super) struct TransformStack {
    current: Transform,
    previous: Vec<Transform>,
    raster: bool,
}
impl TransformStack {
    pub fn new(height: f64) -> Self {
        Self {
            current: Transform::from_row(1.0, 0.0, 0.0, -1.0, 0.0, height as f32),
            previous: Vec::new(),
            raster: false,
        }
    }
    pub fn capture(scale: f64) -> Self {
        Self {
            current: Transform::from_scale(scale as f32, scale as f32),
            previous: Vec::new(),
            raster: true,
        }
    }
    pub fn push(&mut self, check: Check, matrix: Transform) -> Result<(), RenderError> {
        check_matrix(check, "matrix", matrix)?;
        let composed = self.current.pre_concat(matrix);
        check_matrix(check, "composed_matrix", composed)?;
        self.previous.push(self.current);
        self.current = composed;
        Ok(())
    }
    pub fn rotation(
        &mut self,
        check: Check,
        angle: f64,
        cx: f64,
        cy: f64,
    ) -> Result<(), RenderError> {
        check.finite("angle_deg", angle)?;
        check.coordinate("cx", cx)?;
        check.coordinate("cy", cy)?;
        if self.raster {
            return self.push(
                check,
                Transform::from_rotate_at(angle as f32, cx as f32, cy as f32),
            );
        }
        let theta = angle.to_radians();
        check.finite("angle_radians", theta)?;
        let (s, c) = (theta.sin() as f32, theta.cos() as f32);
        let (cx, cy) = (cx as f32, cy as f32);
        self.push(
            check,
            Transform::from_row(c, s, -s, c, cx - c * cx + s * cy, cy - s * cx - c * cy),
        )
    }
    pub fn pop(&mut self) {
        if let Some(previous) = self.previous.pop() {
            self.current = previous;
        }
    }
    pub fn point(&self, check: Check, field: &str, x: f32, y: f32) -> Result<(), RenderError> {
        let mut point = Point::from_xy(x, y);
        self.current.map_point(&mut point);
        check.coordinate(&format!("{field}.transformed.x"), f64::from(point.x))?;
        check.coordinate(&format!("{field}.transformed.y"), f64::from(point.y))
    }
    pub fn stroke(&self, check: Check, width: f64) -> Result<(), RenderError> {
        let scale = f64::from(self.current.sx)
            .hypot(f64::from(self.current.ky))
            .max(f64::from(self.current.kx).hypot(f64::from(self.current.sy)));
        check.coordinate("transformed_stroke_width", width * scale)
    }
}
fn check_matrix(check: Check, field: &str, matrix: Transform) -> Result<(), RenderError> {
    for (name, value) in [
        ("a", matrix.sx),
        ("b", matrix.ky),
        ("c", matrix.kx),
        ("d", matrix.sy),
        ("e", matrix.tx),
        ("f", matrix.ty),
    ] {
        check.coordinate(&format!("{field}.{name}"), f64::from(value))?;
    }
    Ok(())
}
