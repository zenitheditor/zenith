use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryError {
    NonFinitePoint,
    NonFiniteParameter,
    ParameterOutOfRange,
    NonFiniteTolerance,
    NonPositiveTolerance,
    NonPositiveCount,
    CountOutOfRange,
    DegenerateLine,
    InvalidContour,
    NonFiniteTransform,
    SingularTransform,
}

impl fmt::Display for GeometryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GeometryError::NonFinitePoint => f.write_str("point coordinates must be finite"),
            GeometryError::NonFiniteParameter => f.write_str("parameter must be finite"),
            GeometryError::ParameterOutOfRange => {
                f.write_str("parameter must be in the inclusive range [0.0, 1.0]")
            }
            GeometryError::NonFiniteTolerance => f.write_str("tolerance must be finite"),
            GeometryError::NonPositiveTolerance => f.write_str("tolerance must be positive"),
            GeometryError::NonPositiveCount => f.write_str("count must be positive"),
            GeometryError::CountOutOfRange => f.write_str("count is outside the supported range"),
            GeometryError::DegenerateLine => f.write_str("line endpoints must be distinct"),
            GeometryError::InvalidContour => {
                f.write_str("closed contour must be simple and have non-zero area")
            }
            GeometryError::NonFiniteTransform => {
                f.write_str("transform coefficients must be finite")
            }
            GeometryError::SingularTransform => f.write_str("transform must be invertible"),
        }
    }
}

impl std::error::Error for GeometryError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_parameter_errors() {
        assert_eq!(
            GeometryError::NonFiniteParameter.to_string(),
            "parameter must be finite"
        );
        assert_eq!(
            GeometryError::ParameterOutOfRange.to_string(),
            "parameter must be in the inclusive range [0.0, 1.0]"
        );
    }

    #[test]
    fn displays_transform_errors() {
        assert_eq!(
            GeometryError::NonPositiveCount.to_string(),
            "count must be positive"
        );
        assert_eq!(
            GeometryError::CountOutOfRange.to_string(),
            "count is outside the supported range"
        );
        assert_eq!(
            GeometryError::DegenerateLine.to_string(),
            "line endpoints must be distinct"
        );
        assert_eq!(
            GeometryError::InvalidContour.to_string(),
            "closed contour must be simple and have non-zero area"
        );
        assert_eq!(
            GeometryError::NonFiniteTransform.to_string(),
            "transform coefficients must be finite"
        );
        assert_eq!(
            GeometryError::SingularTransform.to_string(),
            "transform must be invertible"
        );
    }
}
