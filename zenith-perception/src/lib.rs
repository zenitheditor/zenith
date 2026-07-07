//! Read-only deterministic metrics over Zenith raster surfaces.

pub mod density_map;
pub mod diagnostic;
pub mod edge_map;
pub mod histogram;
pub mod report;
pub mod scalar;
pub mod value_zones;

pub use density_map::{DensityCell, DensityRatioSummary, DensityReport, density_map};
pub use diagnostic::{PerceptionDiagnostic, PerceptionSeverity};
pub use edge_map::{EdgeReport, edge_map};
pub use histogram::{Histogram, histogram};
pub use report::{PerceptionReport, analyze};
pub use value_zones::{ValueZone, ZoneMetrics, ZoneReport, value_zones};
