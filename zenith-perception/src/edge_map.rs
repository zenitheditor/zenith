use zenith_raster::Surface;

use crate::diagnostic::{PerceptionDiagnostic, PerceptionSeverity};
use crate::scalar::{luminance, pixel_count};

const HARD_EDGE_THRESHOLD: f32 = 0.25;
const LOST_EDGE_THRESHOLD: f32 = 0.03;

#[derive(Debug, Clone, PartialEq)]
pub struct EdgeReport {
    pub count: u64,
    pub max: f32,
    pub mean: f32,
    pub hard_count: u64,
    pub lost_count: u64,
    pub diagnostics: Vec<PerceptionDiagnostic>,
}

pub fn edge_map(surface: &Surface) -> EdgeReport {
    let mut count = 0_u64;
    let mut total = 0.0_f32;
    let mut max = 0.0_f32;
    let mut hard_count = 0_u64;
    let mut lost_count = 0_u64;

    for y in 0..surface.height() {
        for x in 0..surface.width() {
            let Some(pixel) = surface.get(x, y) else {
                continue;
            };
            let value = luminance(pixel);

            if let Some(right) = surface.get(x.saturating_add(1), y) {
                collect_edge(
                    (value - luminance(right)).abs(),
                    &mut count,
                    &mut total,
                    &mut max,
                    &mut hard_count,
                    &mut lost_count,
                );
            }
            if let Some(down) = surface.get(x, y.saturating_add(1)) {
                collect_edge(
                    (value - luminance(down)).abs(),
                    &mut count,
                    &mut total,
                    &mut max,
                    &mut hard_count,
                    &mut lost_count,
                );
            }
        }
    }

    let diagnostics = if pixel_count(surface) > 1 && max == 0.0 {
        vec![PerceptionDiagnostic::new(
            "edge.low_signal",
            PerceptionSeverity::Info,
            "no luminance edge signal detected on a multi-pixel surface",
        )]
    } else {
        Vec::new()
    };

    EdgeReport {
        count,
        max,
        mean: if count == 0 {
            0.0
        } else {
            total / count as f32
        },
        hard_count,
        lost_count,
        diagnostics,
    }
}

fn collect_edge(
    delta: f32,
    count: &mut u64,
    total: &mut f32,
    max: &mut f32,
    hard_count: &mut u64,
    lost_count: &mut u64,
) {
    *count += 1;
    *total += delta;
    *max = max.max(delta);
    if delta >= HARD_EDGE_THRESHOLD {
        *hard_count += 1;
    } else if delta > 0.0 && delta < LOST_EDGE_THRESHOLD {
        *lost_count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_raster::{LinearRgba, Surface};

    #[test]
    fn edge_map_handles_borders_and_thresholds() {
        let surface =
            Surface::from_pixels(2, 2, vec![gray(0.0), gray(0.01), gray(0.25), gray(0.5)]).unwrap();

        let report = edge_map(&surface);

        assert_eq!(report.count, 4);
        assert_eq!(report.hard_count, 3);
        assert_eq!(report.lost_count, 1);
        assert!((report.max - 0.49).abs() < 0.000_001);
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn flat_multi_pixel_surfaces_emit_low_signal_diagnostic() {
        let surface = Surface::filled(2, 1, gray(0.5)).unwrap();

        let report = edge_map(&surface);

        assert_eq!(report.count, 1);
        assert_eq!(report.max, 0.0);
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].code, "edge.low_signal");
    }

    fn gray(value: f32) -> LinearRgba {
        LinearRgba::straight(value, value, value, 1.0).unwrap()
    }
}
