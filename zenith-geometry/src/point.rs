use crate::GeometryError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    #[must_use]
    pub const fn new_unchecked(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn new(x: f64, y: f64) -> Result<Self, GeometryError> {
        let point = Self { x, y };
        point.validate()?;
        Ok(point)
    }

    pub fn validate(self) -> Result<(), GeometryError> {
        if self.is_finite() {
            Ok(())
        } else {
            Err(GeometryError::NonFinitePoint)
        }
    }

    #[must_use]
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    #[must_use]
    pub fn midpoint(self, other: Self) -> Self {
        self.lerp(other, 0.5)
    }

    #[must_use]
    pub fn lerp(self, other: Self, t: f64) -> Self {
        Self {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
        }
    }

    #[must_use]
    pub fn distance_squared_to_line(self, line_start: Self, line_end: Self) -> f64 {
        let dx = line_end.x - line_start.x;
        let dy = line_end.y - line_start.y;
        let length_squared = dx.mul_add(dx, dy * dy);

        if length_squared == 0.0 {
            return self.distance_squared(line_start);
        }

        let cross = (self.x - line_start.x) * dy - (self.y - line_start.y) * dx;
        (cross * cross) / length_squared
    }

    #[must_use]
    pub fn distance_squared_to_segment(self, segment_start: Self, segment_end: Self) -> f64 {
        let dx = segment_end.x - segment_start.x;
        let dy = segment_end.y - segment_start.y;
        let length_squared = dx.mul_add(dx, dy * dy);

        if length_squared == 0.0 {
            return self.distance_squared(segment_start);
        }

        let projection =
            ((self.x - segment_start.x) * dx + (self.y - segment_start.y) * dy) / length_squared;
        let t = projection.clamp(0.0, 1.0);
        self.distance_squared(segment_start.lerp(segment_end, t))
    }

    #[must_use]
    pub fn distance_squared(self, other: Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx.mul_add(dx, dy * dy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_finite_coordinates() {
        assert_eq!(
            Point2::new(f64::NAN, 0.0),
            Err(GeometryError::NonFinitePoint)
        );
        assert_eq!(
            Point2::new(0.0, f64::INFINITY),
            Err(GeometryError::NonFinitePoint)
        );
    }

    #[test]
    fn interpolates_between_points() {
        let a = Point2::new(2.0, 4.0).expect("valid point");
        let b = Point2::new(10.0, 20.0).expect("valid point");

        assert_eq!(a.lerp(b, 0.25), Point2::new_unchecked(4.0, 8.0));
        assert_eq!(a.midpoint(b), Point2::new_unchecked(6.0, 12.0));
    }

    #[test]
    fn measures_distance_to_bounded_segment() {
        let point = Point2::new_unchecked(2.0, 0.0);
        let segment_start = Point2::new_unchecked(0.0, 0.0);
        let segment_end = Point2::new_unchecked(1.0, 0.0);

        assert_eq!(
            point.distance_squared_to_segment(segment_start, segment_end),
            1.0
        );
        assert_eq!(
            point.distance_squared_to_line(segment_start, segment_end),
            0.0
        );
    }
}
