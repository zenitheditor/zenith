use crate::{GeometryError, Point2};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectBounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl RectBounds {
    pub fn from_xywh(x: f64, y: f64, width: f64, height: f64) -> Result<Self, GeometryError> {
        if !x.is_finite() || !y.is_finite() {
            return Err(GeometryError::NonFinitePoint);
        }
        if !width.is_finite() || !height.is_finite() {
            return Err(GeometryError::NonFiniteParameter);
        }
        if width < 0.0 || height < 0.0 {
            return Err(GeometryError::CountOutOfRange);
        }

        let bounds = Self {
            min_x: x,
            min_y: y,
            max_x: x + width,
            max_y: y + height,
        };

        if bounds.is_valid() {
            Ok(bounds)
        } else {
            Err(GeometryError::NonFiniteParameter)
        }
    }

    #[must_use]
    pub fn from_point(point: Point2) -> Self {
        Self {
            min_x: point.x,
            min_y: point.y,
            max_x: point.x,
            max_y: point.y,
        }
    }

    #[must_use]
    pub fn include_point(self, point: Point2) -> Self {
        Self {
            min_x: self.min_x.min(point.x),
            min_y: self.min_y.min(point.y),
            max_x: self.max_x.max(point.x),
            max_y: self.max_y.max(point.y),
        }
    }

    #[must_use]
    pub fn include_bounds(self, bounds: Self) -> Self {
        Self {
            min_x: self.min_x.min(bounds.min_x),
            min_y: self.min_y.min(bounds.min_y),
            max_x: self.max_x.max(bounds.max_x),
            max_y: self.max_y.max(bounds.max_y),
        }
    }

    pub fn outset(self, amount: f64) -> Result<Self, GeometryError> {
        if !self.is_valid() {
            return Err(GeometryError::CountOutOfRange);
        }
        if !amount.is_finite() {
            return Err(GeometryError::NonFiniteParameter);
        }

        let bounds = Self {
            min_x: self.min_x - amount,
            min_y: self.min_y - amount,
            max_x: self.max_x + amount,
            max_y: self.max_y + amount,
        };

        if bounds.is_valid() {
            Ok(bounds)
        } else {
            Err(GeometryError::CountOutOfRange)
        }
    }

    #[must_use]
    pub fn contains_bounds(self, bounds: Self) -> bool {
        self.is_valid()
            && bounds.is_valid()
            && self.min_x <= bounds.min_x
            && self.min_y <= bounds.min_y
            && self.max_x >= bounds.max_x
            && self.max_y >= bounds.max_y
    }

    #[must_use]
    pub fn width(self) -> f64 {
        self.max_x - self.min_x
    }

    #[must_use]
    pub fn height(self) -> f64 {
        self.max_y - self.min_y
    }

    #[must_use]
    pub fn center_x(self) -> f64 {
        (self.min_x + self.max_x) * 0.5
    }

    #[must_use]
    pub fn center_y(self) -> f64 {
        (self.min_y + self.max_y) * 0.5
    }

    #[must_use]
    pub fn is_valid(self) -> bool {
        self.min_x.is_finite()
            && self.min_y.is_finite()
            && self.max_x.is_finite()
            && self.max_y.is_finite()
            && self.max_x >= self.min_x
            && self.max_y >= self.min_y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_to_include_points() {
        let bounds = RectBounds::from_point(Point2::new_unchecked(2.0, 3.0))
            .include_point(Point2::new_unchecked(-1.0, 5.0))
            .include_point(Point2::new_unchecked(4.0, -2.0));

        assert_eq!(
            bounds,
            RectBounds {
                min_x: -1.0,
                min_y: -2.0,
                max_x: 4.0,
                max_y: 5.0
            }
        );
        assert_eq!(bounds.width(), 5.0);
        assert_eq!(bounds.height(), 7.0);
        assert_eq!(bounds.center_x(), 1.5);
        assert_eq!(bounds.center_y(), 1.5);
        assert!(bounds.is_valid());
    }

    #[test]
    fn expands_to_include_bounds() {
        let bounds =
            RectBounds::from_point(Point2::new_unchecked(2.0, 3.0)).include_bounds(RectBounds {
                min_x: -4.0,
                min_y: 1.0,
                max_x: 8.0,
                max_y: 9.0,
            });

        assert_eq!(
            bounds,
            RectBounds {
                min_x: -4.0,
                min_y: 1.0,
                max_x: 8.0,
                max_y: 9.0
            }
        );
    }

    #[test]
    fn builds_from_origin_and_size() {
        assert_eq!(
            RectBounds::from_xywh(2.0, 3.0, 5.0, 7.0),
            Ok(RectBounds {
                min_x: 2.0,
                min_y: 3.0,
                max_x: 7.0,
                max_y: 10.0
            })
        );
        assert_eq!(
            RectBounds::from_xywh(f64::NAN, 3.0, 5.0, 7.0),
            Err(GeometryError::NonFinitePoint)
        );
        assert_eq!(
            RectBounds::from_xywh(2.0, 3.0, f64::INFINITY, 7.0),
            Err(GeometryError::NonFiniteParameter)
        );
        assert_eq!(
            RectBounds::from_xywh(2.0, 3.0, -1.0, 7.0),
            Err(GeometryError::CountOutOfRange)
        );
    }

    #[test]
    fn outsets_and_contains_bounds() {
        let bounds = RectBounds::from_xywh(2.0, 3.0, 5.0, 7.0).expect("valid bounds");
        let outer = bounds.outset(2.0).expect("valid outset");

        assert_eq!(
            outer,
            RectBounds {
                min_x: 0.0,
                min_y: 1.0,
                max_x: 9.0,
                max_y: 12.0
            }
        );
        assert!(outer.contains_bounds(bounds));
        assert!(!bounds.contains_bounds(outer));
        assert_eq!(
            bounds.outset(f64::NAN),
            Err(GeometryError::NonFiniteParameter)
        );
        assert_eq!(bounds.outset(-4.0), Err(GeometryError::CountOutOfRange));
    }

    #[test]
    fn invalid_bounds_are_detected() {
        assert!(
            !RectBounds {
                min_x: 2.0,
                min_y: 0.0,
                max_x: 1.0,
                max_y: 1.0
            }
            .is_valid()
        );
        assert!(
            !RectBounds {
                min_x: 0.0,
                min_y: f64::NAN,
                max_x: 1.0,
                max_y: 1.0
            }
            .is_valid()
        );
    }
}
