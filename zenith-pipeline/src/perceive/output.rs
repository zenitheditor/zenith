//! The `perceive page` JSON envelope, schema 1.
//!
//! Field order is the declaration order of these structs, so the bytes are
//! stable. Every non-integer number is rounded to [`PERCEIVE_DECIMALS`]
//! decimal places by [`Fixed`]. The envelope carries no tool version, so a
//! release bump never changes its bytes.

use serde::{Serialize, Serializer};
use zenith_perception::{PerceptionDiagnostic, PerceptionSeverity};

use crate::error::PipelineError;

/// The envelope schema version. A breaking change to the shape bumps it.
pub const PERCEIVE_SCHEMA: &str = "zenith-perceive-page-v1";

/// Decimal places of every non-integer number in the envelope.
pub const PERCEIVE_DECIMALS: u32 = 4;

/// The scale [`PERCEIVE_DECIMALS`] rounds at: `10^PERCEIVE_DECIMALS`.
const DECIMAL_SCALE: f64 = 10_000.0;

/// The statement every envelope carries.
pub const PERCEIVE_NOTE: &str = "Deterministic signals measured from the render. \
They are not a judgement of design quality.";

/// A number rounded half away from zero to [`PERCEIVE_DECIMALS`] places.
///
/// Rounding uses only IEEE-754 multiply, round, and divide, so every target
/// gives the same value. Negative zero prints as `0.0`. A non-finite value
/// prints as `null`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fixed(pub f64);

impl Fixed {
    /// The rounded value, or `None` when it is not finite.
    #[must_use]
    pub fn rounded(self) -> Option<f64> {
        if !self.0.is_finite() {
            return None;
        }
        let value = (self.0 * DECIMAL_SCALE).round() / DECIMAL_SCALE;
        if value == 0.0 {
            Some(0.0)
        } else if value.is_finite() {
            Some(value)
        } else {
            None
        }
    }
}

impl From<f32> for Fixed {
    fn from(value: f32) -> Self {
        Self(f64::from(value))
    }
}

impl From<f64> for Fixed {
    fn from(value: f64) -> Self {
        Self(value)
    }
}

impl Serialize for Fixed {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.rounded() {
            Some(value) => serializer.serialize_f64(value),
            None => serializer.serialize_none(),
        }
    }
}

/// The whole `perceive page` result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PerceiveReport {
    /// Always [`PERCEIVE_SCHEMA`].
    pub schema: &'static str,
    /// Always [`PERCEIVE_NOTE`].
    pub note: &'static str,
    /// Always [`PERCEIVE_DECIMALS`].
    pub decimals: u32,
    /// The number of pages in the document.
    pub page_count: usize,
    /// One entry per perceived page, in page order.
    pub pages: Vec<PagePerception>,
}

/// The signals of one page.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PagePerception {
    /// The 1-based page number.
    pub page: usize,
    /// The page id.
    pub id: String,
    /// Width in pixels of the measured area: the trim box at scale 1.
    pub width: u32,
    /// Height in pixels of the measured area.
    pub height: u32,
    /// The ink of the page: pixels that differ from the most common pixel.
    pub ink: InkOutput,
    /// One entry per metric.
    pub metrics: PageMetrics,
}

/// Ink of a page.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InkOutput {
    /// The most common pixel, `#rrggbbaa`. A tie takes the lowest value.
    pub background: String,
    /// The number of pixels that differ from `background`.
    pub pixel_count: u64,
    /// The bounds of those pixels in pixel edges, `null` when there are none.
    pub bounds: Option<BoundsOutput>,
}

/// An axis-aligned box in page pixels. `max_*` are exclusive edges.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BoundsOutput {
    pub min_x: Fixed,
    pub min_y: Fixed,
    pub max_x: Fixed,
    pub max_y: Fixed,
}

/// Every metric of a page, in a fixed order.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PageMetrics {
    pub histogram: Metric<HistogramOutput>,
    pub value_zones: Metric<ValueZonesOutput>,
    pub density_map: Metric<DensityOutput>,
    pub edge_map: Metric<EdgeOutput>,
    pub optical_balance: Metric<OpticalBalanceOutput>,
    pub clearspace: Metric<ClearspaceOutput>,
    pub grid_conformance: Metric<GridConformanceOutput>,
}

/// One metric: measured values, or why it does not apply to the page.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Metric<T> {
    /// The metric ran.
    Measured(T),
    /// The metric needs an input the page does not declare.
    NotApplicable {
        /// What is missing, and how to declare it.
        reason: &'static str,
    },
}

/// One perception diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosticOutput {
    pub code: &'static str,
    /// `info` or `warning`.
    pub severity: &'static str,
    pub message: &'static str,
}

impl From<&PerceptionDiagnostic> for DiagnosticOutput {
    fn from(diagnostic: &PerceptionDiagnostic) -> Self {
        Self {
            code: diagnostic.code,
            severity: severity_name(diagnostic.severity),
            message: diagnostic.message,
        }
    }
}

/// The lowercase name of `severity`.
#[must_use]
pub fn severity_name(severity: PerceptionSeverity) -> &'static str {
    match severity {
        PerceptionSeverity::Info => "info",
        PerceptionSeverity::Warning => "warning",
    }
}

/// Convert every diagnostic of a metric report.
#[must_use]
pub fn diagnostics_output(diagnostics: &[PerceptionDiagnostic]) -> Vec<DiagnosticOutput> {
    diagnostics.iter().map(DiagnosticOutput::from).collect()
}

/// Ten-bin luminance histogram.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HistogramOutput {
    pub bins: Vec<u64>,
    pub total_pixels: u64,
    pub min: Fixed,
    pub max: Fixed,
    pub mean: Fixed,
    pub diagnostics: Vec<DiagnosticOutput>,
}

/// Shadow, midtone, and highlight luminance zones.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ValueZonesOutput {
    pub zones: Vec<ZoneOutput>,
    pub diagnostics: Vec<DiagnosticOutput>,
}

/// One luminance zone.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ZoneOutput {
    /// `shadow`, `midtone`, or `highlight`.
    pub zone: &'static str,
    pub count: u64,
    pub min: Fixed,
    pub max: Fixed,
    pub mean: Fixed,
    pub spread: Fixed,
}

/// Luminance contrast energy over a grid of at most 4 × 4 cells.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DensityOutput {
    pub columns: u32,
    pub rows: u32,
    pub total_energy: Fixed,
    pub focal_x: Fixed,
    pub focal_y: Fixed,
    pub strongest_cell: Option<DensityCellOutput>,
    pub ratios: DensityRatiosOutput,
    pub cells: Vec<DensityCellOutput>,
    pub diagnostics: Vec<DiagnosticOutput>,
}

/// One density cell. `*_end` are exclusive pixel edges.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DensityCellOutput {
    pub row: u32,
    pub column: u32,
    pub x_start: u32,
    pub y_start: u32,
    pub x_end: u32,
    pub y_end: u32,
    pub pixel_count: u64,
    pub energy: Fixed,
}

/// Share of energy in the strongest 70 %, next 20 %, and last 10 % of cells.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DensityRatiosOutput {
    pub top_70: Fixed,
    pub next_20: Fixed,
    pub remaining_10: Fixed,
}

/// Luminance steps between neighbouring pixels.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EdgeOutput {
    pub count: u64,
    pub max: Fixed,
    pub mean: Fixed,
    pub hard_count: u64,
    pub lost_count: u64,
    pub diagnostics: Vec<DiagnosticOutput>,
}

/// Offset of the ink centre from the page centre.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OpticalBalanceOutput {
    pub subject: BoundsOutput,
    pub container: BoundsOutput,
    pub center_dx: Fixed,
    pub center_dy: Fixed,
    pub normalized_dx: Fixed,
    pub normalized_dy: Fixed,
    pub offset_magnitude: Fixed,
    pub diagnostics: Vec<DiagnosticOutput>,
}

/// Distance from the ink to each page edge against the declared margins.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClearspaceOutput {
    pub subject: BoundsOutput,
    pub container: BoundsOutput,
    pub left: Fixed,
    pub right: Fixed,
    pub top: Fixed,
    pub bottom: Fixed,
    pub minimum: Fixed,
    /// The smallest of the four declared page margins, in pixels.
    pub required: Fixed,
    pub passes: bool,
    pub diagnostics: Vec<DiagnosticOutput>,
}

/// Distance from top-level node box key points to the declared guides.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GridConformanceOutput {
    pub guide_count: usize,
    pub node_count: usize,
    pub evaluated_key_point_count: usize,
    pub maximum_guide_distance: Fixed,
    pub normalized_distance: Fixed,
    pub diagnostics: Vec<DiagnosticOutput>,
}

/// `report` as pretty JSON (two-space indent, no trailing newline).
///
/// # Errors
///
/// `perceive.encode_failed` (exit 2) when serialisation fails.
pub fn report_json(report: &PerceiveReport) -> Result<String, PipelineError> {
    serde_json::to_string_pretty(report).map_err(|e| {
        PipelineError::new(
            "perceive.encode_failed",
            format!("cannot encode the perceive report: {e}"),
            2,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_rounds_half_away_from_zero_at_four_places() {
        assert_eq!(Fixed(0.123_45).rounded(), Some(0.1235));
        assert_eq!(Fixed(-0.123_45).rounded(), Some(-0.1235));
        assert_eq!(Fixed(2.0).rounded(), Some(2.0));
    }

    #[test]
    fn fixed_prints_negative_zero_as_zero_and_non_finite_as_null() {
        assert_eq!(serde_json::to_string(&Fixed(-0.000_01)).unwrap(), "0.0");
        assert_eq!(serde_json::to_string(&Fixed(f64::NAN)).unwrap(), "null");
        assert_eq!(
            serde_json::to_string(&Fixed(f64::INFINITY)).unwrap(),
            "null"
        );
    }

    #[test]
    fn fixed_from_f32_widens_before_rounding() {
        assert_eq!(Fixed::from(0.5_f32).rounded(), Some(0.5));
    }

    #[test]
    fn metric_status_tag_comes_first() {
        let measured: Metric<EdgeOutput> = Metric::Measured(EdgeOutput {
            count: 1,
            max: Fixed(0.5),
            mean: Fixed(0.5),
            hard_count: 1,
            lost_count: 0,
            diagnostics: Vec::new(),
        });
        let json = serde_json::to_string(&measured).unwrap();
        assert!(
            json.starts_with(r#"{"status":"measured","count":1"#),
            "{json}"
        );
        let skipped: Metric<EdgeOutput> = Metric::NotApplicable { reason: "why" };
        assert_eq!(
            serde_json::to_string(&skipped).unwrap(),
            r#"{"status":"not_applicable","reason":"why"}"#
        );
    }

    #[test]
    fn severity_names_are_lowercase() {
        assert_eq!(severity_name(PerceptionSeverity::Info), "info");
        assert_eq!(severity_name(PerceptionSeverity::Warning), "warning");
    }
}
