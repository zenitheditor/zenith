use crate::{PerceptionDiagnostic, PerceptionSeverity};
use zenith_geometry::RectBounds;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClearspaceInput {
    pub subject: RectBounds,
    pub container: RectBounds,
    pub required: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClearspaceReport {
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
    pub minimum: f64,
    pub required: f64,
    pub passes: bool,
    pub score: f32,
    pub diagnostics: Vec<PerceptionDiagnostic>,
}

pub fn clearspace(input: ClearspaceInput) -> ClearspaceReport {
    let left = input.subject.min_x - input.container.min_x;
    let right = input.container.max_x - input.subject.max_x;
    let top = input.subject.min_y - input.container.min_y;
    let bottom = input.container.max_y - input.subject.max_y;
    let minimum = left.min(right).min(top).min(bottom);

    let mut diagnostics = Vec::new();
    if !valid_bounds(input.subject) || !valid_bounds(input.container) {
        diagnostics.push(PerceptionDiagnostic::new(
            "clearspace.invalid_bounds",
            PerceptionSeverity::Warning,
            "clearspace bounds must be finite and ordered",
        ));
    }
    if !input.required.is_finite() || input.required <= 0.0 {
        diagnostics.push(PerceptionDiagnostic::new(
            "clearspace.invalid_required",
            PerceptionSeverity::Warning,
            "required clearspace must be a positive finite distance",
        ));
    }

    let valid = diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity != PerceptionSeverity::Warning);
    let passes = valid && minimum >= input.required;
    if valid && !passes {
        diagnostics.push(PerceptionDiagnostic::new(
            "clearspace.insufficient",
            PerceptionSeverity::Info,
            "minimum clearspace is below the required distance",
        ));
    }

    ClearspaceReport {
        left,
        right,
        top,
        bottom,
        minimum,
        required: input.required,
        passes,
        score: clearspace_score(valid, minimum, input.required),
        diagnostics,
    }
}

fn valid_bounds(bounds: RectBounds) -> bool {
    bounds.min_x.is_finite()
        && bounds.min_y.is_finite()
        && bounds.max_x.is_finite()
        && bounds.max_y.is_finite()
        && bounds.max_x >= bounds.min_x
        && bounds.max_y >= bounds.min_y
}

fn clearspace_score(valid: bool, minimum: f64, required: f64) -> f32 {
    if !valid || minimum <= 0.0 {
        0.0
    } else {
        (minimum / required).clamp(0.0, 1.0) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_passing_clearspace_distances() {
        let report = clearspace(ClearspaceInput {
            subject: bounds(25.0, 20.0, 75.0, 80.0),
            container: bounds(0.0, 0.0, 100.0, 100.0),
            required: 20.0,
        });

        assert_eq!(report.left, 25.0);
        assert_eq!(report.right, 25.0);
        assert_eq!(report.top, 20.0);
        assert_eq!(report.bottom, 20.0);
        assert_eq!(report.minimum, 20.0);
        assert!(report.passes);
        assert_eq!(report.score, 1.0);
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn diagnoses_insufficient_clearspace() {
        let report = clearspace(ClearspaceInput {
            subject: bounds(10.0, 20.0, 95.0, 80.0),
            container: bounds(0.0, 0.0, 100.0, 100.0),
            required: 20.0,
        });

        assert_eq!(report.minimum, 5.0);
        assert!(!report.passes);
        assert_eq!(report.score, 0.25);
        assert_eq!(
            report.diagnostics,
            vec![PerceptionDiagnostic::new(
                "clearspace.insufficient",
                PerceptionSeverity::Info,
                "minimum clearspace is below the required distance",
            )]
        );
    }

    #[test]
    fn invalid_inputs_score_zero() {
        let report = clearspace(ClearspaceInput {
            subject: bounds(10.0, 0.0, 0.0, 10.0),
            container: bounds(0.0, 0.0, 100.0, 100.0),
            required: 0.0,
        });

        assert!(!report.passes);
        assert_eq!(report.score, 0.0);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "clearspace.invalid_bounds")
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "clearspace.invalid_required")
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
