//! Plot-area inset computation.
//!
//! Reserves margins around the chart bounding box for axis labels, title, and
//! caption, returning a `PlotArea` describing the drawable data region in
//! device-space pixels.

// ── Margin constants ───────────────────────────────────────────────────────────

/// Left margin: reserved for Y-axis tick labels.
const LEFT: f64 = 44.0;
/// Bottom margin: reserved for the X axis line and (future) category labels.
const BOTTOM: f64 = 28.0;
/// Extra top inset when a title is present.
const TOP_TITLE: f64 = 24.0;
/// Extra bottom inset when a caption is present.
const CAPTION: f64 = 20.0;
/// Right margin: breathing room for the rightmost tick label.
const RIGHT: f64 = 12.0;
/// Minimum top inset even without a title (axis top breathing room).
const TOP_MIN: f64 = 10.0;

// ── PlotArea ──────────────────────────────────────────────────────────────────

/// The drawable data region of a chart in device-space pixels, after margins
/// have been reserved for labels, title, and caption.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PlotArea {
    /// Left edge of the plot area (x origin of the Y axis line).
    pub(super) x: f64,
    /// Top edge of the plot area.
    pub(super) y: f64,
    /// Width of the plot area (exclusive of axis labels).
    pub(super) w: f64,
    /// Height of the plot area (exclusive of caption/title).
    pub(super) h: f64,
}

/// Inset the chart bounding box by standard margins, returning the drawable
/// `PlotArea`.
///
/// - Left: `LEFT` px for Y tick labels.
/// - Bottom: `BOTTOM` px for the X axis line.
/// - Top: `TOP_TITLE` px when a title is present, `TOP_MIN` otherwise.
/// - Bottom extra: `CAPTION` px when a caption is present.
/// - Right: `RIGHT` px breathing room.
///
/// The returned `w`/`h` are clamped to `0.0` — no negative dimensions.
pub(super) fn plot_area(
    chart_x: f64,
    chart_y: f64,
    chart_w: f64,
    chart_h: f64,
    has_title: bool,
    has_caption: bool,
) -> PlotArea {
    let top = if has_title { TOP_TITLE } else { TOP_MIN };
    let bottom = BOTTOM + if has_caption { CAPTION } else { 0.0 };

    let x = chart_x + LEFT;
    let y = chart_y + top;
    let w = (chart_w - LEFT - RIGHT).max(0.0);
    let h = (chart_h - top - bottom).max(0.0);

    PlotArea { x, y, w, h }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plot_area_no_title_no_caption() {
        let pa = plot_area(0.0, 0.0, 400.0, 300.0, false, false);
        assert_eq!(pa.x, LEFT);
        assert_eq!(pa.y, TOP_MIN);
        assert!((pa.w - (400.0 - LEFT - RIGHT)).abs() < 1e-10);
        assert!((pa.h - (300.0 - TOP_MIN - BOTTOM)).abs() < 1e-10);
    }

    #[test]
    fn plot_area_with_title() {
        let pa = plot_area(0.0, 0.0, 400.0, 300.0, true, false);
        assert_eq!(pa.y, TOP_TITLE);
        assert!((pa.h - (300.0 - TOP_TITLE - BOTTOM)).abs() < 1e-10);
    }

    #[test]
    fn plot_area_with_caption() {
        let pa = plot_area(0.0, 0.0, 400.0, 300.0, false, true);
        assert!((pa.h - (300.0 - TOP_MIN - BOTTOM - CAPTION)).abs() < 1e-10);
    }

    #[test]
    fn plot_area_clamps_negative() {
        // A 10×10 chart is too small for margins; result must not be negative.
        let pa = plot_area(0.0, 0.0, 10.0, 10.0, true, true);
        assert_eq!(pa.w, 0.0);
        assert_eq!(pa.h, 0.0);
    }

    #[test]
    fn plot_area_translates_origin() {
        let pa = plot_area(50.0, 30.0, 400.0, 300.0, false, false);
        assert!((pa.x - (50.0 + LEFT)).abs() < 1e-10);
        assert!((pa.y - (30.0 + TOP_MIN)).abs() < 1e-10);
    }
}
