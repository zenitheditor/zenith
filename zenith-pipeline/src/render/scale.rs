//! The accepted raster output scale range.

use crate::error::PipelineError;

/// Largest accepted raster output scale.
pub const MAX_RENDER_SCALE: f64 = 4.0;

/// Check a raster output scale: finite, `> 0`, and `<= 4`.
///
/// `shown` is the value as the caller received it, for the message.
///
/// # Errors
///
/// A message naming the input and the accepted range.
pub fn check_render_scale(scale: f64, shown: &str) -> Result<f64, String> {
    if scale.is_finite() && scale > 0.0 && scale <= MAX_RENDER_SCALE {
        return Ok(scale);
    }
    Err(format!(
        "invalid scale '{shown}'; pass a number greater than 0 and at most {MAX_RENDER_SCALE} (e.g. 0.5)"
    ))
}

/// [`check_render_scale`] for the vector raster fallback scale, as a
/// `cli.invalid_argument` error with exit code 2.
pub(crate) fn check_vector_raster_scale(scale: f64) -> Result<(), PipelineError> {
    check_render_scale(scale, &scale.to_string())
        .map(|_| ())
        .map_err(|message| PipelineError::new("cli.invalid_argument", message, 2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_is_open_at_zero_and_closed_at_four() {
        assert_eq!(check_render_scale(4.0, "4"), Ok(4.0));
        assert_eq!(check_render_scale(0.01, "0.01"), Ok(0.01));
        for bad in [0.0, -0.5, 4.000_001, f64::NAN, f64::INFINITY] {
            assert!(check_render_scale(bad, "x").is_err(), "{bad}");
        }
        assert!(check_vector_raster_scale(0.0).is_err());
    }
}
