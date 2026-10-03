//! Drawn glyph ink: the geometry the compile stage measures for each `text`
//! node, and the backdrop sampling over it.
//!
//! A text node draws its glyphs line by line. Each [`InkLine`] is the ink
//! box of one line in the space its glyph runs draw in, with the affine map
//! from that space to page px. Sampling maps the box's sample points through
//! the map, so a rotated, scaled, or translated line is sampled where its
//! glyphs land.

use std::collections::BTreeMap;

use super::geometry::RectPx;
use super::label::LabelInk;
use super::text::{collect_backdrop_samples, push_unique_sample};
use super::types::{BackdropCandidate, SampledBackdrop};

/// The ink box of one drawn line of text, before its transform.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InkLine {
    /// Ink box left edge, in run space.
    pub x: f64,
    /// Ink box top edge, in run space.
    pub y: f64,
    /// Ink box width.
    pub w: f64,
    /// Ink box height.
    pub h: f64,
    /// The map `[a, b, c, d, e, f]` from run space to page px (trim-box
    /// origin): `x' = a·x + c·y + e`, `y' = b·x + d·y + f`.
    pub matrix: [f64; 6],
}

impl InkLine {
    /// The page position of the run-space point `(x, y)`.
    fn map(&self, x: f64, y: f64) -> (f64, f64) {
        let [a, b, c, d, e, f] = self.matrix;
        (a * x + c * y + e, b * x + d * y + f)
    }
}

/// The measured glyph ink of one drawn `text` node: one entry per line.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GlyphInk {
    pub lines: Vec<InkLine>,
}

/// The drawn ink the compile stage measured on one page, by node id.
#[derive(Debug, Clone, Copy)]
pub struct ContrastInks<'a> {
    /// `shape` / `connector` label ink, by owner id.
    pub labels: &'a BTreeMap<String, LabelInk>,
    /// `text` glyph ink, by text id. A text with no entry draws no glyph.
    pub texts: &'a BTreeMap<String, GlyphInk>,
}

/// The backdrops sampled under `points` (page px), and whether any point
/// sits on an unsampled paint. A point outside `clip` is not drawn and adds
/// no sample. The third value is `true` when no point is drawn.
pub(super) fn sample_points(
    points: impl IntoIterator<Item = (f64, f64)>,
    clip: Option<RectPx>,
    candidates: &[BackdropCandidate],
    page_bg_rgb: Option<(u8, u8, u8)>,
) -> (Vec<SampledBackdrop>, bool, bool) {
    let mut all = Vec::new();
    let mut indeterminate = false;
    let mut hidden = true;
    for (x, y) in points {
        let point = RectPx {
            x,
            y,
            w: 0.0,
            h: 0.0,
        };
        if clip.is_some_and(|c| !c.contains_rect(point)) {
            continue;
        }
        hidden = false;
        let (samples, point_indeterminate) =
            collect_backdrop_samples(point, None, candidates, page_bg_rgb);
        indeterminate |= point_indeterminate;
        for sample in samples {
            push_unique_sample(&mut all, sample);
        }
    }
    (all, indeterminate, hidden)
}

/// The page-px sample points of every line of `ink`: each line box's center
/// and inset corners, mapped to the page.
pub(super) fn ink_points(ink: &GlyphInk) -> impl Iterator<Item = (f64, f64)> + '_ {
    ink.lines.iter().flat_map(|line| {
        let local = RectPx {
            x: line.x,
            y: line.y,
            w: line.w,
            h: line.h,
        };
        local
            .sample_points()
            .into_iter()
            .map(move |(x, y)| line.map(x, y))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_points_follow_the_matrix() {
        let ink = GlyphInk {
            lines: vec![InkLine {
                x: 0.0,
                y: 0.0,
                w: 10.0,
                h: 4.0,
                matrix: [2.0, 0.0, 0.0, 2.0, 100.0, 50.0],
            }],
        };
        let points: Vec<(f64, f64)> = ink_points(&ink).collect();
        assert_eq!(points.len(), 5);
        assert_eq!(points[0], (110.0, 54.0));
        assert_eq!(points[1], (101.0, 51.0));
    }

    #[test]
    fn clipped_points_add_no_sample() {
        let clip = Some(RectPx {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
        });
        let (samples, indeterminate, hidden) =
            sample_points([(50.0, 50.0)], clip, &[], Some((255, 255, 255)));
        assert!(samples.is_empty());
        assert!(!indeterminate);
        assert!(hidden);
        let (samples, _, hidden) = sample_points([(5.0, 5.0)], clip, &[], Some((1, 2, 3)));
        assert_eq!(samples.len(), 1);
        assert!(!hidden);
    }
}
