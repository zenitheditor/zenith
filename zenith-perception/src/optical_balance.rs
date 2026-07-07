use crate::{PerceptionDiagnostic, PerceptionSeverity};
use zenith_geometry::RectBounds;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpticalBalanceInput {
    pub subject: RectBounds,
    pub container: RectBounds,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpticalBalanceReport {
    pub center_dx: f64,
    pub center_dy: f64,
    pub normalized_dx: f32,
    pub normalized_dy: f32,
    pub offset_magnitude: f32,
    pub score: f32,
    pub diagnostics: Vec<PerceptionDiagnostic>,
}

pub fn optical_balance(input: OpticalBalanceInput) -> OpticalBalanceReport {
    let center_dx = input.subject.center_x() - input.container.center_x();
    let center_dy = input.subject.center_y() - input.container.center_y();
    let mut diagnostics = Vec::new();

    if !input.subject.is_valid() {
        diagnostics.push(PerceptionDiagnostic::new(
            "optical_balance.invalid_subject",
            PerceptionSeverity::Warning,
            "subject bounds must be finite and ordered",
        ));
    }
    if !input.container.is_valid()
        || input.container.width() <= 0.0
        || input.container.height() <= 0.0
    {
        diagnostics.push(PerceptionDiagnostic::new(
            "optical_balance.invalid_container",
            PerceptionSeverity::Warning,
            "container bounds must be finite, ordered, and non-degenerate",
        ));
    }

    let valid = diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity != PerceptionSeverity::Warning);
    let normalized_dx = if valid {
        (center_dx / input.container.width()) as f32
    } else {
        0.0
    };
    let normalized_dy = if valid {
        (center_dy / input.container.height()) as f32
    } else {
        0.0
    };
    let offset_magnitude = normalized_dx.hypot(normalized_dy);

    OpticalBalanceReport {
        center_dx,
        center_dy,
        normalized_dx,
        normalized_dy,
        offset_magnitude,
        score: optical_balance_score(valid, offset_magnitude),
        diagnostics,
    }
}

fn optical_balance_score(valid: bool, offset_magnitude: f32) -> f32 {
    if valid {
        (1.0 - offset_magnitude).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_subject_scores_one() {
        let report = optical_balance(OpticalBalanceInput {
            subject: bounds(25.0, 25.0, 75.0, 75.0),
            container: bounds(0.0, 0.0, 100.0, 100.0),
        });

        assert_eq!(report.center_dx, 0.0);
        assert_eq!(report.center_dy, 0.0);
        assert_eq!(report.normalized_dx, 0.0);
        assert_eq!(report.normalized_dy, 0.0);
        assert_eq!(report.offset_magnitude, 0.0);
        assert_eq!(report.score, 1.0);
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn offset_subject_reports_signed_container_normalized_offsets() {
        let report = optical_balance(OpticalBalanceInput {
            subject: bounds(40.0, 10.0, 80.0, 50.0),
            container: bounds(0.0, 0.0, 100.0, 200.0),
        });

        assert_eq!(report.center_dx, 10.0);
        assert_eq!(report.center_dy, -70.0);
        assert_eq!(report.normalized_dx, 0.1);
        assert_eq!(report.normalized_dy, -0.35);
        assert!((report.offset_magnitude - 0.364_005_5).abs() < 0.000_001);
        assert!((report.score - 0.635_994_5).abs() < 0.000_001);
    }

    #[test]
    fn invalid_inputs_score_zero() {
        let report = optical_balance(OpticalBalanceInput {
            subject: bounds(10.0, 0.0, 0.0, 10.0),
            container: bounds(0.0, 0.0, 0.0, 100.0),
        });

        assert_eq!(report.normalized_dx, 0.0);
        assert_eq!(report.normalized_dy, 0.0);
        assert_eq!(report.offset_magnitude, 0.0);
        assert_eq!(report.score, 0.0);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "optical_balance.invalid_subject")
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "optical_balance.invalid_container")
        );
    }

    fn bounds(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> RectBounds {
        RectBounds {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }
}
