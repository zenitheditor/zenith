use crate::GeometryError;

pub(crate) fn validate_tolerance(tolerance: f64) -> Result<(), GeometryError> {
    if !tolerance.is_finite() {
        Err(GeometryError::NonFiniteTolerance)
    } else if tolerance <= 0.0 {
        Err(GeometryError::NonPositiveTolerance)
    } else {
        Ok(())
    }
}
