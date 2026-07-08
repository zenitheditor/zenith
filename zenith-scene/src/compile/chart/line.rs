//! Line, area, and sparkline series rendering for `kind="line"`, `kind="area"`,
//! and `kind="sparkline"`.
//!
//! Pure geometry helpers (`line_points`) are free of engine/I-O dependencies
//! and are directly unit-testable. Emit helpers push `SceneCommand`s into an
//! existing command list — no allocations beyond the points vec.

use crate::ir::{Color, FillRule, Paint, SceneCommand, StrokeAlign};

use super::frame::PlotArea;
use super::scale::LinearScale;

// ── line_points ───────────────────────────────────────────────────────────────

/// Compute pixel (x, y) pairs for one series across the plot width.
///
/// `y = y_scale.map(value)`. The X placement depends on `slot_center`:
/// - `slot_center = true` (category line/area): each point sits at the center
///   of its category band, `x(c) = plot.x + (c + 0.5) * (plot.w / n)`. This
///   aligns the vertices with bar slots and the category labels beneath them.
/// - `slot_center = false` (sparkline / continuous): edge-to-edge, first point
///   at `plot.x` and last at `plot.x + plot.w`.
///
/// A single point is centered at `plot.x + plot.w / 2` in either mode.
///
/// Returns an empty vec when `values` is empty, `plot.w <= 0`, or
/// `plot.h <= 0`.
pub(super) fn line_points(
    values: &[f64],
    plot: &PlotArea,
    y_scale: &LinearScale,
    slot_center: bool,
) -> Vec<(f64, f64)> {
    let n = values.len();
    if n == 0 || plot.w <= 0.0 || plot.h <= 0.0 {
        return Vec::new();
    }

    values
        .iter()
        .enumerate()
        .map(|(c, &v)| {
            let x = if n == 1 {
                plot.x + plot.w / 2.0
            } else if slot_center {
                // Category band centers: aligns with bars and category labels.
                plot.x + (c as f64 + 0.5) * (plot.w / n as f64)
            } else {
                // Edge-to-edge: first point at plot.x, last at plot.x + plot.w.
                plot.x + c as f64 * (plot.w / (n - 1) as f64)
            };
            let y = y_scale.map(v);
            (x, y)
        })
        .collect()
}

// ── emit_line_series ──────────────────────────────────────────────────────────

/// Push a `StrokePolyline` for one series.
///
/// No-op when `points.len() < 2` (a single point does not form a stroke).
pub(super) fn emit_line_series(
    points: &[(f64, f64)],
    color: Color,
    stroke_width: f64,
    commands: &mut Vec<SceneCommand>,
) {
    if points.len() < 2 {
        return;
    }

    let mut flat: Vec<f64> = Vec::with_capacity(points.len() * 2);
    for &(x, y) in points {
        flat.push(x);
        flat.push(y);
    }

    commands.push(SceneCommand::StrokePolyline {
        points: flat,
        color,
        stroke_width,
        closed: false,
        align: StrokeAlign::Center,
        clip_fill_rule: FillRule::NonZero,
    });
}

// ── emit_area_fill ────────────────────────────────────────────────────────────

/// Push a `FillPolygon` for the area under the line, closed down to the plot
/// baseline (`plot.y + plot.h`).
///
/// No-op when `points.len() < 2`.
///
/// The polygon is built left-to-right along the line points, then the two
/// baseline corners close the shape:
/// `[line pts …, (last_x, baseline), (first_x, baseline)]`.
pub(super) fn emit_area_fill(
    points: &[(f64, f64)],
    plot: &PlotArea,
    area_color: Color,
    commands: &mut Vec<SceneCommand>,
) {
    if points.len() < 2 {
        return;
    }

    let baseline = plot.y + plot.h;

    // Capacity: n line points + 2 baseline corners, each with x and y.
    let mut flat: Vec<f64> = Vec::with_capacity((points.len() + 2) * 2);

    for &(x, y) in points {
        flat.push(x);
        flat.push(y);
    }

    // Close down to baseline: last point's x → first point's x.
    if let Some(&(last_x, _)) = points.last() {
        flat.push(last_x);
        flat.push(baseline);
    }
    if let Some(&(first_x, _)) = points.first() {
        flat.push(first_x);
        flat.push(baseline);
    }

    commands.push(SceneCommand::FillPolygon {
        points: flat,
        paint: Paint::solid(area_color),
        fill_rule: FillRule::NonZero,
    });
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Color, SceneCommand};

    fn test_plot() -> PlotArea {
        PlotArea {
            x: 44.0,
            y: 10.0,
            w: 300.0,
            h: 200.0,
        }
    }

    fn test_scale() -> LinearScale {
        LinearScale {
            data_min: 0.0,
            data_max: 100.0,
            pixel_min: 210.0, // plot.y + plot.h = 10+200
            pixel_max: 10.0,  // plot.y
        }
    }

    // ── line_points ───────────────────────────────────────────────────────────

    #[test]
    fn line_points_empty_values_returns_empty() {
        let plot = test_plot();
        let scale = test_scale();
        let pts = line_points(&[], &plot, &scale, false);
        assert!(pts.is_empty());
    }

    #[test]
    fn line_points_zero_width_returns_empty() {
        let plot = PlotArea {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 200.0,
        };
        let scale = test_scale();
        let pts = line_points(&[50.0, 75.0], &plot, &scale, false);
        assert!(pts.is_empty());
    }

    #[test]
    fn line_points_zero_height_returns_empty() {
        let plot = PlotArea {
            x: 0.0,
            y: 0.0,
            w: 300.0,
            h: 0.0,
        };
        let scale = test_scale();
        let pts = line_points(&[50.0, 75.0], &plot, &scale, false);
        assert!(pts.is_empty());
    }

    #[test]
    fn line_points_single_value_centered() {
        let plot = test_plot();
        let scale = test_scale();
        let pts = line_points(&[50.0], &plot, &scale, false);
        assert_eq!(pts.len(), 1);
        let expected_x = plot.x + plot.w / 2.0;
        assert!(
            (pts[0].0 - expected_x).abs() < 1e-9,
            "single point x should be centered: got {} expected {}",
            pts[0].0,
            expected_x
        );
    }

    #[test]
    fn line_points_three_values_edge_to_edge() {
        let plot = test_plot();
        let scale = test_scale();
        let pts = line_points(&[10.0, 50.0, 90.0], &plot, &scale, false);
        assert_eq!(pts.len(), 3);

        let eps = 1e-9;

        // First x at left edge.
        assert!(
            (pts[0].0 - plot.x).abs() < eps,
            "first point x should equal plot.x: got {} expected {}",
            pts[0].0,
            plot.x
        );

        // Last x at right edge.
        let right = plot.x + plot.w;
        assert!(
            (pts[2].0 - right).abs() < eps,
            "last point x should equal plot.x+plot.w: got {} expected {}",
            pts[2].0,
            right
        );

        // Middle at center.
        let center = plot.x + plot.w / 2.0;
        assert!(
            (pts[1].0 - center).abs() < eps,
            "middle point x should be centered: got {} expected {}",
            pts[1].0,
            center
        );
    }

    #[test]
    fn line_points_larger_value_smaller_y() {
        // Inverted Y: larger data value → smaller pixel y (higher on screen).
        let plot = test_plot();
        let scale = test_scale();
        let pts = line_points(&[25.0, 75.0], &plot, &scale, false);
        assert_eq!(pts.len(), 2);
        assert!(
            pts[1].1 < pts[0].1,
            "larger value (75) should have smaller y than smaller value (25)"
        );
    }

    #[test]
    fn line_points_slot_center_within_bands() {
        // Slot-center: point c sits at the center of its category band.
        let plot = test_plot();
        let scale = test_scale();
        let pts = line_points(&[10.0, 50.0, 90.0], &plot, &scale, true);
        assert_eq!(pts.len(), 3);
        let eps = 1e-9;
        let slot_w = plot.w / 3.0;
        for (c, p) in pts.iter().enumerate() {
            let expected = plot.x + (c as f64 + 0.5) * slot_w;
            assert!(
                (p.0 - expected).abs() < eps,
                "slot-center point {c} x: got {} expected {}",
                p.0,
                expected
            );
        }
        // First point is inset from the left edge (not at plot.x).
        assert!(pts[0].0 > plot.x + eps);
    }

    // ── emit_line_series ──────────────────────────────────────────────────────

    #[test]
    fn emit_line_series_one_point_no_command() {
        let pts = vec![(10.0, 20.0)];
        let mut cmds: Vec<SceneCommand> = Vec::new();
        emit_line_series(&pts, Color::srgb(0, 0, 255, 255), 2.0, &mut cmds);
        assert!(
            cmds.is_empty(),
            "one point should produce no StrokePolyline"
        );
    }

    #[test]
    fn emit_line_series_three_points_six_coords() {
        let pts = vec![(10.0, 20.0), (30.0, 40.0), (50.0, 60.0)];
        let mut cmds: Vec<SceneCommand> = Vec::new();
        emit_line_series(&pts, Color::srgb(0, 0, 255, 255), 2.0, &mut cmds);
        assert_eq!(cmds.len(), 1, "should emit exactly one command");
        match &cmds[0] {
            SceneCommand::StrokePolyline { points, .. } => {
                assert_eq!(
                    points.len(),
                    6,
                    "3 points → 6 flat coords, got {}",
                    points.len()
                );
            }
            other => panic!("expected StrokePolyline, got {:?}", other),
        }
    }

    // ── emit_area_fill ────────────────────────────────────────────────────────

    #[test]
    fn emit_area_fill_one_point_no_command() {
        let plot = test_plot();
        let pts = vec![(10.0, 20.0)];
        let mut cmds: Vec<SceneCommand> = Vec::new();
        emit_area_fill(&pts, &plot, Color::srgb(0, 0, 255, 64), &mut cmds);
        assert!(cmds.is_empty(), "one point should produce no FillPolygon");
    }

    #[test]
    fn emit_area_fill_three_points_polygon_coord_count() {
        let plot = test_plot();
        // 3 line pts + 2 baseline corners = 5 pairs = 10 flat coords.
        let pts = vec![(44.0, 50.0), (194.0, 100.0), (344.0, 80.0)];
        let mut cmds: Vec<SceneCommand> = Vec::new();
        emit_area_fill(&pts, &plot, Color::srgb(0, 0, 255, 64), &mut cmds);
        assert_eq!(cmds.len(), 1, "should emit exactly one command");
        match &cmds[0] {
            SceneCommand::FillPolygon { points, .. } => {
                assert_eq!(
                    points.len(),
                    10,
                    "3 pts + 2 baseline corners = 10 flat coords, got {}",
                    points.len()
                );
            }
            other => panic!("expected FillPolygon, got {:?}", other),
        }
    }
}
