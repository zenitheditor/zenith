//! Baseline-grid snapping: align a text node's first-line baseline and inter-line
//! advance onto the page baseline grid so corresponding lines line up across
//! columns/chain members.

use zenith_core::Diagnostic;

/// Snap a text node's first-line baseline and inter-line advance onto the page
/// baseline grid of pitch `g`.
///
/// Given the natural (post-`ctx.dy`) `text_y`, the resolved `ascent`, and the
/// resolved `line_height`, returns `(snapped_text_y, effective_line_height)`:
/// the first baseline moves DOWN to the next grid line at/below its natural
/// position, and the advance inflates to the smallest multiple of `g` that is
/// ≥ `line_height`, so corresponding lines align horizontally across columns.
/// Because the emit computes `baseline_y = text_y + ascent + i*line_height`,
/// substituting these two values places every baseline on the grid with no
/// change to the emit code. Caller must ensure `g.is_finite() && g > 0.0`.
pub(in crate::compile) fn snap_to_baseline_grid(
    text_y: f64,
    ascent: f64,
    line_height: f64,
    g: f64,
) -> (f64, f64) {
    let natural_baseline = text_y + ascent;
    let snapped_baseline = (natural_baseline / g).ceil() * g;
    let effective_line_height = (line_height / g).ceil() * g;
    let snapped_text_y = snapped_baseline - ascent;
    (snapped_text_y, effective_line_height)
}

/// Build the `baseline-grid.snap_failed` advisory for a text node whose resolved
/// line-height exceeds the grid pitch (a single line cannot fit one grid cell,
/// so the effective advance inflates to a multiple of `g` and leading grows).
/// Emitted ONCE per affected node; the caller only calls this when
/// `line_height > g`.
pub(in crate::compile) fn baseline_grid_snap_failed_diag(
    node_id: &str,
    line_height: f64,
    g: f64,
    span: Option<zenith_core::Span>,
) -> Diagnostic {
    let multiple = (line_height / g).ceil();
    let effective = multiple * g;
    Diagnostic::warning(
        "baseline-grid.snap_failed",
        format!(
            "text node '{node_id}' line-height {line_height}px exceeds baseline-grid \
             pitch {g}px; lines snap to {effective}px ({multiple}× grid)"
        ),
        span,
        Some(node_id.to_owned()),
    )
}

#[cfg(test)]
mod baseline_grid_unit_tests {
    use super::*;

    #[test]
    fn snaps_first_baseline_down_to_next_grid_line() {
        // g=14, ascent=12, text_y chosen so natural baseline = 350.0 (a multiple
        // of 14 is 350=25*14) → snapped baseline stays 350; snapped_text_y=338.
        let (ty, lh) = snap_to_baseline_grid(/* text_y */ 338.0, 12.0, 18.0, 14.0);
        assert_eq!(ty + 12.0, 350.0, "baseline already on the grid stays put");
        // 18 → ceil(18/14)=2 → 28.
        assert_eq!(lh, 28.0);
    }

    #[test]
    fn natural_baseline_355_snaps_to_364() {
        // natural baseline = text_y(343) + ascent(12) = 355; next grid line ≥ 355
        // is 364 (26*14). snapped_text_y = 364 - 12 = 352.
        let (ty, lh) = snap_to_baseline_grid(343.0, 12.0, 14.0, 14.0);
        assert_eq!(ty + 12.0, 364.0);
        // line_height 14 == g → effective stays 14 (ceil(14/14)=1).
        assert_eq!(lh, 14.0);
    }

    #[test]
    fn effective_advance_is_smallest_multiple_ge_line_height() {
        // line_height just under one cell → 1 cell; just over → 2 cells.
        let (_, lh1) = snap_to_baseline_grid(0.0, 10.0, 13.9, 14.0);
        assert_eq!(lh1, 14.0);
        let (_, lh2) = snap_to_baseline_grid(0.0, 10.0, 14.1, 14.0);
        assert_eq!(lh2, 28.0);
    }

    #[test]
    fn snap_failed_diag_names_node_and_pitch() {
        let d = baseline_grid_snap_failed_diag("col1", 18.0, 14.0, None);
        assert_eq!(d.code, "baseline-grid.snap_failed");
        assert!(d.message.contains("col1"), "message names the node id");
        assert!(d.message.contains("18"), "message names line-height");
        assert!(d.message.contains("14"), "message names the grid pitch");
        assert!(
            d.message.contains("28"),
            "message names the snapped advance"
        );
    }
}
