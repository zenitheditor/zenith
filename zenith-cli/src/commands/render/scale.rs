//! The accepted raster output scale range, shared by the CLI and MCP.

/// Largest accepted raster output scale.
pub const MAX_RENDER_SCALE: f64 = 4.0;

/// Check a raster output scale: finite, `> 0`, and `<= 4`.
///
/// `shown` is the value as the caller received it, for the message. The error
/// names the input and the accepted range.
pub fn check_render_scale(scale: f64, shown: &str) -> Result<f64, String> {
    if scale.is_finite() && scale > 0.0 && scale <= MAX_RENDER_SCALE {
        return Ok(scale);
    }
    Err(format!(
        "invalid scale '{shown}'; pass a number greater than 0 and at most {MAX_RENDER_SCALE} (e.g. 0.5)"
    ))
}

pub(crate) fn parse_scale_flag(raw: &str, flag: &str) -> Result<f64, String> {
    let parsed = raw.trim().parse::<f64>().unwrap_or(f64::NAN);
    check_render_scale(parsed, raw).map_err(|message| format!("error: {flag}: {message}"))
}

pub(super) fn check_vector_raster_scale(scale: f64) -> Result<(), super::entry::RenderCmdErr> {
    check_render_scale(scale, &scale.to_string())
        .map(|_| ())
        .map_err(|message| super::entry::RenderCmdErr::new("cli.invalid_argument", message, 2))
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
    }
}
