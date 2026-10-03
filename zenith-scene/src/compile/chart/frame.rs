//! Plot-area and band arithmetic.
//!
//! Every margin is a fixed multiple of the chart base size `b`, so the frame
//! grows with the text it holds. The title band is cut from the top of the
//! chart box and the caption band from the bottom before the legend and the
//! plot share the rest.

/// Title band at the top of the chart box: `2.4 b`.
const TITLE_BAND: f64 = 2.4;
/// Caption band at the bottom of the chart box: `1.75 b`.
const CAPTION_BAND: f64 = 1.75;
/// Top inset of the plot, room for the top tick label: `0.75 b`.
const TOP_MIN: f64 = 0.75;
/// Gap between an axis and its labels: `0.5 b`.
const LABEL_GAP: f64 = 0.5;
/// Band under the plot for the axis labels: `2.1 b`.
const LABEL_BAND: f64 = 2.1;
/// Outer padding left of the leftmost labels: `0.5 b`.
const EDGE_PAD: f64 = 0.5;
/// Right inset of the plot: `1.0 b`.
const RIGHT_PAD: f64 = 1.0;

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

/// A box `(x, y, w, h)` in device px.
pub(super) type BoxPx = (f64, f64, f64, f64);

/// The chart band measures for base size `b`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Bands {
    pub(super) base: f64,
}

impl Bands {
    /// The chart box minus the title band (when `has_title`) and the caption
    /// band (when `has_caption`). Width and height clamp to `0`.
    pub(super) fn content(self, chart: BoxPx, has_title: bool, has_caption: bool) -> BoxPx {
        let (x, y, w, h) = chart;
        let top = if has_title {
            self.base * TITLE_BAND
        } else {
            0.0
        };
        let bottom = if has_caption {
            self.base * CAPTION_BAND
        } else {
            0.0
        };
        (x, y + top, w, (h - top - bottom).max(0.0))
    }

    /// Gap between an axis and its labels.
    pub(super) fn label_gap(self) -> f64 {
        self.base * LABEL_GAP
    }

    /// Outer padding left of the leftmost labels.
    pub(super) fn edge_pad(self) -> f64 {
        self.base * EDGE_PAD
    }

    /// Top inset of the plot.
    pub(super) fn top(self) -> f64 {
        self.base * TOP_MIN
    }

    /// Band under the plot for the axis labels.
    pub(super) fn label_band(self) -> f64 {
        self.base * LABEL_BAND
    }

    /// Right inset of the plot.
    pub(super) fn right(self) -> f64 {
        self.base * RIGHT_PAD
    }

    /// Distance from the top of the chart box to the title's ink top.
    pub(super) fn title_inset(self) -> f64 {
        self.base * 0.25
    }

    /// Distance from the bottom of the chart box up to the caption baseline.
    pub(super) fn caption_inset(self) -> f64 {
        self.base * 0.45
    }

    /// The plot of a cartesian chart in `bbox` whose left labels need
    /// `left_labels` px: left = labels + gap + edge pad, top = [`Self::top`],
    /// bottom = [`Self::label_band`], right = [`Self::right`].
    pub(super) fn plot(self, bbox: BoxPx, left_labels: f64) -> PlotArea {
        let left = left_labels + self.label_gap() + self.edge_pad();
        inset(bbox, left, self.top(), self.right(), self.label_band())
    }
}

/// `bbox` inset by `left`, `top`, `right`, `bottom`. Width and height clamp
/// to `0`.
pub(super) fn inset(bbox: BoxPx, left: f64, top: f64, right: f64, bottom: f64) -> PlotArea {
    let (x, y, w, h) = bbox;
    PlotArea {
        x: x + left,
        y: y + top,
        w: (w - left - right).max(0.0),
        h: (h - top - bottom).max(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const B: Bands = Bands { base: 10.0 };

    #[test]
    fn content_cuts_title_and_caption_bands() {
        assert_eq!(
            B.content((0.0, 0.0, 400.0, 300.0), false, false),
            (0.0, 0.0, 400.0, 300.0)
        );
        assert_eq!(
            B.content((0.0, 0.0, 400.0, 300.0), true, true),
            (0.0, 24.0, 400.0, 300.0 - 24.0 - 17.5)
        );
    }

    #[test]
    fn plot_reserves_scaled_margins() {
        let pa = B.plot((0.0, 0.0, 400.0, 300.0), 30.0);
        assert!((pa.x - (30.0 + 5.0 + 5.0)).abs() < 1e-10);
        assert!((pa.y - 7.5).abs() < 1e-10);
        assert!((pa.w - (400.0 - 40.0 - 10.0)).abs() < 1e-10);
        assert!((pa.h - (300.0 - 7.5 - 21.0)).abs() < 1e-10);
    }

    #[test]
    fn margins_double_with_the_base() {
        let small = B.plot((0.0, 0.0, 400.0, 300.0), 0.0);
        let big = Bands { base: 20.0 }.plot((0.0, 0.0, 400.0, 300.0), 0.0);
        assert!(((big.x - 0.0) - 2.0 * (small.x - 0.0)).abs() < 1e-10);
        assert!(((300.0 - big.h) - 2.0 * (300.0 - small.h)).abs() < 1e-10);
    }

    #[test]
    fn plot_clamps_negative() {
        let pa = B.plot((0.0, 0.0, 10.0, 10.0), 50.0);
        assert_eq!(pa.w, 0.0);
        assert_eq!(pa.h, 0.0);
    }

    #[test]
    fn plot_translates_origin() {
        let pa = inset((50.0, 30.0, 400.0, 300.0), 4.0, 6.0, 0.0, 0.0);
        assert!((pa.x - 54.0).abs() < 1e-10);
        assert!((pa.y - 36.0).abs() < 1e-10);
    }
}
