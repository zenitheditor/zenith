//! Small helpers shared by `theme new` and `theme apply`.

/// Format a scalar without a trailing `.0` (so `16.0` → `16`, `1.5` → `1.5`).
///
/// Shared by `new::px` (dimension shape values) and `apply::encode_literal`
/// (plain `number` token values).
pub(super) fn format_scalar(v: f64) -> String {
    zenith_core::format_number(v)
}
