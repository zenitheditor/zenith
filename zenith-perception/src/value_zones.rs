use zenith_raster::Surface;

use crate::diagnostic::{PerceptionDiagnostic, PerceptionSeverity};
use crate::scalar::{luminance, pixel_count};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueZone {
    Shadow,
    Midtone,
    Highlight,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ZoneMetrics {
    pub zone: ValueZone,
    pub count: u64,
    pub min: f32,
    pub max: f32,
    pub mean: f32,
    pub spread: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ZoneReport {
    pub zones: Vec<ZoneMetrics>,
    pub diagnostics: Vec<PerceptionDiagnostic>,
}

#[derive(Debug, Clone, Copy)]
struct ZoneAccumulator {
    zone: ValueZone,
    count: u64,
    min: f32,
    max: f32,
    total: f32,
}

impl ZoneAccumulator {
    const fn new(zone: ValueZone) -> Self {
        Self {
            zone,
            count: 0,
            min: 1.0,
            max: 0.0,
            total: 0.0,
        }
    }

    fn add(&mut self, value: f32) {
        self.count += 1;
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        self.total += value;
    }

    fn metrics(self) -> ZoneMetrics {
        if self.count == 0 {
            return ZoneMetrics {
                zone: self.zone,
                count: 0,
                min: 0.0,
                max: 0.0,
                mean: 0.0,
                spread: 0.0,
            };
        }

        ZoneMetrics {
            zone: self.zone,
            count: self.count,
            min: self.min,
            max: self.max,
            mean: self.total / self.count as f32,
            spread: self.max - self.min,
        }
    }
}

pub fn value_zones(surface: &Surface) -> ZoneReport {
    let mut accumulators = [
        ZoneAccumulator::new(ValueZone::Shadow),
        ZoneAccumulator::new(ValueZone::Midtone),
        ZoneAccumulator::new(ValueZone::Highlight),
    ];

    for pixel in surface.pixels() {
        let value = luminance(*pixel);
        let zone = zone_for(value);
        for accumulator in &mut accumulators {
            if accumulator.zone == zone {
                accumulator.add(value);
            }
        }
    }

    let zones = accumulators
        .into_iter()
        .map(ZoneAccumulator::metrics)
        .collect::<Vec<_>>();
    let occupied = zones.iter().filter(|zone| zone.count > 0).count();
    let diagnostics = if pixel_count(surface) > 0 && occupied == 1 {
        vec![PerceptionDiagnostic::new(
            "value.zone_uncompressed",
            PerceptionSeverity::Warning,
            "all pixels occupy a single luminance value zone",
        )]
    } else {
        Vec::new()
    };

    ZoneReport { zones, diagnostics }
}

fn zone_for(value: f32) -> ValueZone {
    if value < (1.0 / 3.0) {
        return ValueZone::Shadow;
    }
    if value < (2.0 / 3.0) {
        return ValueZone::Midtone;
    }
    ValueZone::Highlight
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_raster::{LinearRgba, Surface};

    #[test]
    fn single_zone_surfaces_emit_compression_diagnostic() {
        let surface = Surface::filled(2, 2, gray(0.25)).unwrap();

        let report = value_zones(&surface);

        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].code, "value.zone_uncompressed");
        assert_eq!(report.zones[0].count, 4);
        assert_eq!(report.zones[0].spread, 0.0);
    }

    #[test]
    fn zone_boundaries_are_shadow_midtone_highlight() {
        let surface = Surface::from_pixels(
            3,
            1,
            vec![gray((1.0 / 3.0) - 0.01), gray(1.0 / 3.0), gray(2.0 / 3.0)],
        )
        .unwrap();

        let report = value_zones(&surface);

        assert_eq!(report.zones[0].count, 1);
        assert_eq!(report.zones[1].count, 1);
        assert_eq!(report.zones[2].count, 1);
        assert!(report.diagnostics.is_empty());
    }

    fn gray(value: f32) -> LinearRgba {
        LinearRgba::straight(value, value, value, 1.0).unwrap()
    }
}
