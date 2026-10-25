//! Number formatting for the print messages.

/// `value` with at most two decimals and no trailing zeros: `8`, `5.33`.
pub(super) fn fmt_num(value: f64) -> String {
    let text = format!("{value:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

/// A CMYK color as authored: `cmyk(60,40,40,100)`.
pub(super) fn fmt_cmyk(cmyk: [f32; 4]) -> String {
    let [c, m, y, k] = cmyk.map(|v| fmt_num(f64::from(v)));
    format!("cmyk({c},{m},{y},{k})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_drop_trailing_zeros() {
        assert_eq!(fmt_num(8.0), "8");
        assert_eq!(fmt_num(5.333), "5.33");
        assert_eq!(fmt_num(2.5), "2.5");
        assert_eq!(fmt_num(-0.001), "0");
    }

    #[test]
    fn cmyk_reads_as_authored() {
        assert_eq!(fmt_cmyk([60.0, 40.0, 40.0, 100.0]), "cmyk(60,40,40,100)");
        assert_eq!(fmt_cmyk([12.5, 0.0, 0.0, 0.0]), "cmyk(12.5,0,0,0)");
    }
}
