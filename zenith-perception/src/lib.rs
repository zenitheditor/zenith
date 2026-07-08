//! Read-only deterministic perception metrics for Zenith raster surfaces and vector inputs.

pub mod anchor_economy;
pub mod clearspace;
pub mod density_map;
pub mod diagnostic;
pub mod edge_map;
pub mod grid_conformance;
pub mod histogram;
pub mod optical_balance;
pub mod path_collision;
mod path_geometry;
pub mod path_outline;
pub mod path_tangent_quality;
pub mod report;
pub mod scalar;
pub mod value_zones;
pub mod vector_mark;
pub mod vector_report;

pub use anchor_economy::{AnchorEconomyInput, AnchorEconomyReport, anchor_economy};
pub use clearspace::{ClearspaceInput, ClearspaceReport, clearspace};
pub use density_map::{DensityCell, DensityRatioSummary, DensityReport, density_map};
pub use diagnostic::{PerceptionDiagnostic, PerceptionSeverity};
pub use edge_map::{EdgeReport, edge_map};
pub use grid_conformance::{
    CompoundGridConformanceInput, GridConformanceInput, GridConformanceReport,
    compound_grid_conformance, grid_conformance,
};
pub use histogram::{Histogram, histogram};
pub use optical_balance::{OpticalBalanceInput, OpticalBalanceReport, optical_balance};
pub use path_collision::{
    CompoundPathCollisionInput, CompoundPathCollisionNearestPoints, CompoundPathCollisionReport,
    PathCollisionInput, PathCollisionNearestPoints, PathCollisionReport, compound_path_collision,
    path_collision,
};
pub use path_outline::{
    CompoundPathOutlinePerceptionInput, CompoundPathOutlinePerceptionReport, PathOutlineKind,
    PathOutlinePerceptionInput, PathOutlinePerceptionReport, compound_path_outline, path_outline,
};
pub use path_tangent_quality::{
    PathTangentQualityInput, PathTangentQualityReport, path_tangent_quality,
};
pub use report::{PerceptionReport, analyze};
pub use value_zones::{ValueZone, ZoneMetrics, ZoneReport, value_zones};
pub use vector_mark::{
    VectorMarkCollisionReport, VectorMarkPathInput, VectorMarkPerceptionInput,
    VectorMarkPerceptionReport, analyze_vector_mark,
};
pub use vector_report::{
    CompoundVectorPathPerceptionInput, CompoundVectorPathPerceptionReport, VectorPathContourInput,
    VectorPathPerceptionInput, VectorPathPerceptionReport, analyze_compound_vector_path,
    analyze_vector_path,
};
