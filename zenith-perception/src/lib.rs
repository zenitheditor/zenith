//! Read-only deterministic perception metrics for Zenith raster surfaces and vector inputs.

pub mod anchor_economy;
pub mod clearspace;
pub mod density_map;
pub mod diagnostic;
pub mod edge_map;
pub mod histogram;
pub mod optical_balance;
mod path_geometry;
pub mod path_tangent_quality;
pub mod report;
pub mod scalar;
pub mod value_zones;
pub mod vector_report;

pub use anchor_economy::{AnchorEconomyInput, AnchorEconomyReport, anchor_economy};
pub use clearspace::{ClearspaceInput, ClearspaceReport, clearspace};
pub use density_map::{DensityCell, DensityRatioSummary, DensityReport, density_map};
pub use diagnostic::{PerceptionDiagnostic, PerceptionSeverity};
pub use edge_map::{EdgeReport, edge_map};
pub use histogram::{Histogram, histogram};
pub use optical_balance::{OpticalBalanceInput, OpticalBalanceReport, optical_balance};
pub use path_tangent_quality::{
    PathTangentQualityInput, PathTangentQualityReport, path_tangent_quality,
};
pub use report::{PerceptionReport, analyze};
pub use value_zones::{ValueZone, ZoneMetrics, ZoneReport, value_zones};
pub use vector_report::{
    VectorPathPerceptionInput, VectorPathPerceptionReport, analyze_vector_path,
};
