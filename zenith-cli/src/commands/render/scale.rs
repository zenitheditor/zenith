//! `--scale` / `--raster-scale` flag parsing over the pipeline's scale range.

use zenith_pipeline::render::check_render_scale;

/// Parse a scale flag value: a finite number with `0 < F <= 4`.
///
/// `flag` names the flag in the error, which reads `error: <flag>: <reason>`.
pub(crate) fn parse_scale_flag(raw: &str, flag: &str) -> Result<f64, String> {
    let parsed = raw.trim().parse::<f64>().unwrap_or(f64::NAN);
    check_render_scale(parsed, raw).map_err(|message| format!("error: {flag}: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_names_the_flag_and_rejects_out_of_range() {
        assert_eq!(parse_scale_flag(" 0.5 ", "--scale"), Ok(0.5));
        let err = parse_scale_flag("5", "--scale").expect_err("too big");
        assert!(
            err.starts_with("error: --scale: invalid scale '5'"),
            "{err}"
        );
        assert!(parse_scale_flag("abc", "--raster-scale").is_err());
    }
}
