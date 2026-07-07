use zenith_raster::Surface;

use crate::density_map::{DensityReport, density_map};
use crate::edge_map::{EdgeReport, edge_map};
use crate::histogram::{Histogram, histogram};
use crate::value_zones::{ZoneReport, value_zones};

#[derive(Debug, Clone, PartialEq)]
pub struct PerceptionReport {
    pub histogram: Histogram,
    pub value_zones: ZoneReport,
    pub density_map: DensityReport,
    pub edge_map: EdgeReport,
}

pub fn analyze(surface: &Surface) -> PerceptionReport {
    PerceptionReport {
        histogram: histogram(surface),
        value_zones: value_zones(surface),
        density_map: density_map(surface),
        edge_map: edge_map(surface),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_raster::{LinearRgba, Surface};

    #[test]
    fn aggregate_report_contains_all_metric_outputs() {
        let surface = Surface::from_pixels(2, 1, vec![gray(0.0), gray(1.0)]).unwrap();

        let report = analyze(&surface);

        assert_eq!(report.histogram.total_pixels, 2);
        assert_eq!(report.value_zones.zones.len(), 3);
        assert_eq!(report.density_map.cells.len(), 2);
        assert_eq!(report.edge_map.count, 1);
    }

    fn gray(value: f32) -> LinearRgba {
        LinearRgba::straight(value, value, value, 1.0).unwrap()
    }
}
