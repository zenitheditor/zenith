//! Contrast of chart text, judged on drawn geometry.
//!
//! The scene compiler draws every chart string (title, caption, axis and
//! category labels, legend, value labels) and measures each one like a
//! label: its ink box, run colours, and run size. This module samples the
//! backdrop under each string and reports each failing role once per chart,
//! at its lowest contrast.

use std::collections::BTreeMap;

use crate::ast::node::ChartNode;
use crate::diagnostics::Diagnostic;

use super::label::{InkVerdict, LabelInk, contrast_diagnostic, worst_ink_sample};
use super::text::lc_threshold;
use super::types::{BackdropCandidate, ContrastEnv, ContrastSample, PaintCtx};

/// The measured ink of one drawn chart string.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartTextInk {
    /// The text role (`title`, `axis`, `category`, `legend`, `value`, …).
    pub role: String,
    /// Font weight the role draws at.
    pub weight: u16,
    /// Ink box, colours, and size, in page px.
    pub ink: LabelInk,
}

/// Judge the drawn strings of `chart`, when the compile stage measured any.
pub(super) fn check_chart_text(
    chart: &ChartNode,
    ctx: PaintCtx<'_>,
    candidates: &[BackdropCandidate],
    env: ContrastEnv<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(items) = env.inks.and_then(|inks| inks.charts.get(chart.id.as_str())) else {
        return;
    };
    // The lowest failing sample per role, in role-name order.
    let mut worst: BTreeMap<&str, (ContrastSample, f64)> = BTreeMap::new();
    for item in items {
        let Some(sample) = worst_ink_sample(&item.ink, ctx, candidates) else {
            continue;
        };
        let threshold = lc_threshold(item.ink.font_size_px, u32::from(item.weight));
        if sample.lc >= threshold {
            continue;
        }
        let slot = worst
            .entry(item.role.as_str())
            .or_insert((sample, threshold));
        if sample.lc < slot.0.lc {
            *slot = (sample, threshold);
        }
    }
    for (role, (sample, threshold)) in worst {
        diagnostics.push(contrast_diagnostic(
            InkVerdict {
                subject: &format!("chart '{}' {role} text", chart.id),
                fix: "set the chart `fill`, or the chart style `fill`, to a colour with more contrast",
                sample,
                threshold,
            },
            chart.source_span,
            &chart.id,
        ));
    }
}
