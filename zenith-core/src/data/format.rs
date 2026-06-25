//! Deterministic, pure data-value formatter.
//!
//! All formatting is done by hand — no external locale or number-format crate.
//! Same bytes in → same bytes out on any machine, making this safe on the
//! render path.

/// The display format to apply to a resolved data field value.
#[derive(Debug, Clone, PartialEq)]
pub enum DataFormat {
    /// Currency: `"$1,234.56"`. Negative values render as `"-$1,234.56"`.
    /// `locale` is reserved for future locale codes (currently unused; en-US
    /// thousands/decimal separators are always used). `precision` defaults to 2.
    Currency {
        locale: Option<String>,
        precision: Option<u8>,
    },
    /// Percentage: value × 100 + `"%"`. `precision` defaults to 1.
    Percent { precision: Option<u8> },
    /// Plain number with thousands separators. `precision` defaults to 0.
    Number { precision: Option<u8> },
}

/// Format `raw` according to `fmt`.
///
/// If `raw` does not parse as an `f64` it is returned unchanged (passthrough).
/// All arithmetic and string construction is deterministic and allocation-only
/// (no `f64::to_string` locale dependencies — we drive the digits ourselves).
pub fn format_data_value(raw: &str, fmt: &DataFormat) -> String {
    let value: f64 = match raw.parse() {
        Ok(v) => v,
        Err(_) => return raw.to_owned(),
    };

    match fmt {
        DataFormat::Currency { precision, .. } => {
            let prec = precision.unwrap_or(2) as usize;
            let negative = value < 0.0;
            let abs_val = value.abs();
            let formatted = format_number_parts(abs_val, prec);
            if negative {
                format!("-${formatted}")
            } else {
                format!("${formatted}")
            }
        }
        DataFormat::Percent { precision } => {
            let prec = precision.unwrap_or(1) as usize;
            let pct = value * 100.0;
            let formatted = format_fixed(pct, prec);
            format!("{formatted}%")
        }
        DataFormat::Number { precision } => {
            let prec = precision.unwrap_or(0) as usize;
            let negative = value < 0.0;
            let abs_val = value.abs();
            let formatted = format_number_parts(abs_val, prec);
            if negative {
                format!("-{formatted}")
            } else {
                formatted
            }
        }
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Format `value` with `prec` decimal places AND thousands separators (`,`).
/// Always uses en-US conventions: comma thousands, period decimal.
fn format_number_parts(value: f64, prec: usize) -> String {
    let fixed = format_fixed(value, prec);
    // Split on the decimal point (if any).
    let (integer_part, decimal_part) = if let Some(dot) = fixed.find('.') {
        (&fixed[..dot], Some(&fixed[dot..]))
    } else {
        (fixed.as_str(), None)
    };

    let with_thousands = insert_thousands(integer_part);
    match decimal_part {
        Some(dec) => format!("{with_thousands}{dec}"),
        None => with_thousands,
    }
}

/// Format `value` with exactly `prec` decimal places, no thousands separators.
///
/// Uses `f64`'s built-in `format!("{:.prec$}")` which is deterministic (IEEE
/// 754 round-to-nearest-even) but does NOT apply locale — the decimal separator
/// is always `.`, matching our target en-US output.
fn format_fixed(value: f64, prec: usize) -> String {
    format!("{value:.prec$}")
}

/// Insert a `,` every three digits from the right into `integer_str`.
///
/// `integer_str` must contain only ASCII digits (no sign, no decimal). This is
/// a pure string manipulation — no arithmetic — so it is branch-free and
/// byte-stable across all platforms.
fn insert_thousands(integer_str: &str) -> String {
    if integer_str.len() <= 3 {
        return integer_str.to_owned();
    }
    let chars: Vec<char> = integer_str.chars().collect();
    let len = chars.len();
    // Number of commas to insert.
    let comma_count = (len - 1) / 3;
    let mut result = String::with_capacity(len + comma_count);
    for (i, ch) in chars.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(',');
        }
        result.push(*ch);
    }
    result
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Currency ─────────────────────────────────────────────────────────────────

    #[test]
    fn currency_default_precision() {
        assert_eq!(
            format_data_value(
                "1234.5",
                &DataFormat::Currency {
                    locale: None,
                    precision: None
                }
            ),
            "$1,234.50"
        );
    }

    #[test]
    fn currency_zero_precision() {
        assert_eq!(
            format_data_value(
                "9999.99",
                &DataFormat::Currency {
                    locale: None,
                    precision: Some(0)
                }
            ),
            "$10,000"
        );
    }

    #[test]
    fn currency_negative() {
        assert_eq!(
            format_data_value(
                "-42.5",
                &DataFormat::Currency {
                    locale: None,
                    precision: None
                }
            ),
            "-$42.50"
        );
    }

    #[test]
    fn currency_thousands() {
        assert_eq!(
            format_data_value(
                "1000000.0",
                &DataFormat::Currency {
                    locale: None,
                    precision: Some(2)
                }
            ),
            "$1,000,000.00"
        );
    }

    #[test]
    fn currency_small() {
        assert_eq!(
            format_data_value(
                "5.0",
                &DataFormat::Currency {
                    locale: None,
                    precision: Some(2)
                }
            ),
            "$5.00"
        );
    }

    // Percent ──────────────────────────────────────────────────────────────────

    #[test]
    fn percent_default_precision() {
        assert_eq!(
            format_data_value("0.1234", &DataFormat::Percent { precision: None }),
            "12.3%"
        );
    }

    #[test]
    fn percent_zero_precision() {
        assert_eq!(
            format_data_value("0.5", &DataFormat::Percent { precision: Some(0) }),
            "50%"
        );
    }

    #[test]
    fn percent_negative() {
        assert_eq!(
            format_data_value("-0.05", &DataFormat::Percent { precision: Some(1) }),
            "-5.0%"
        );
    }

    #[test]
    fn percent_high_precision() {
        assert_eq!(
            format_data_value("0.12345", &DataFormat::Percent { precision: Some(3) }),
            "12.345%"
        );
    }

    // Number ───────────────────────────────────────────────────────────────────

    #[test]
    fn number_default_precision() {
        assert_eq!(
            format_data_value("1234567.8", &DataFormat::Number { precision: None }),
            "1,234,568"
        );
    }

    #[test]
    fn number_with_precision() {
        assert_eq!(
            format_data_value("1234.5", &DataFormat::Number { precision: Some(2) }),
            "1,234.50"
        );
    }

    #[test]
    fn number_negative() {
        assert_eq!(
            format_data_value("-9876.0", &DataFormat::Number { precision: Some(0) }),
            "-9,876"
        );
    }

    #[test]
    fn number_small_no_thousands() {
        assert_eq!(
            format_data_value("42.0", &DataFormat::Number { precision: Some(0) }),
            "42"
        );
    }

    // Non-numeric passthrough ──────────────────────────────────────────────────

    #[test]
    fn passthrough_non_numeric() {
        assert_eq!(
            format_data_value(
                "N/A",
                &DataFormat::Currency {
                    locale: None,
                    precision: None
                }
            ),
            "N/A"
        );
    }

    #[test]
    fn passthrough_empty() {
        assert_eq!(
            format_data_value("", &DataFormat::Number { precision: None }),
            ""
        );
    }

    #[test]
    fn passthrough_string_with_letters() {
        assert_eq!(
            format_data_value("twelve", &DataFormat::Percent { precision: None }),
            "twelve"
        );
    }
}
