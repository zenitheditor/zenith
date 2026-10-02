//! Per-kind check for the `chart` node.

use std::collections::BTreeSet;

use crate::ast::node::ChartNode;
use crate::diagnostics::Diagnostic;

use super::shared::{
    AnchorParentCtx, AnchorProps, TokenEnv, VisualProps, check_anchor, check_optional_dim,
    check_style_ref, check_visual_props,
};
use super::suggest::check_unknown_props;
use crate::schema::enums::{
    CHART_BAR_MODES, CHART_KINDS, CHART_LEGEND_ALIGNS, CHART_LEGEND_LAYOUTS,
    CHART_LEGEND_POSITIONS, CHART_ORIENTATIONS, CHART_POINT_PLACEMENTS, CHART_VALUE_LABELS,
};
use crate::suggest::invalid_value_message;
use crate::validate::check::nodes::WalkCtx;
use crate::validate::check::register_id;

pub(in crate::validate::check) fn check_chart(
    c: &ChartNode,
    ctx: WalkCtx,
    seen_ids: &mut BTreeSet<String>,
    referenced_token_ids: &mut BTreeSet<String>,
    geom_required: bool,
    parent_ctx: AnchorParentCtx,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let WalkCtx {
        resolved_tokens,
        declared_style_ids,
        zone_ids,
        ..
    } = ctx;
    // The chart's own id participates in id-uniqueness. The series children are
    // pure DATA and are intentionally NOT visited here.
    register_id(&c.id, seen_ids, diagnostics);
    check_style_ref(
        &c.id,
        c.style.as_deref(),
        declared_style_ids,
        c.source_span,
        diagnostics,
    );

    // A recognized anchor supplies both x and y; the chart IS anchor-bearing.
    let anchor_active = check_anchor(
        &c.id,
        AnchorProps {
            anchor: c.anchor.as_deref(),
            anchor_zone: c.anchor_zone.as_deref(),
            anchor_sibling: c.anchor_sibling.as_deref(),
            anchor_parent: c.anchor_parent == Some(true),
            anchor_edge: c.anchor_edge.as_deref(),
            anchor_gap: c.anchor_gap.as_ref(),
        },
        parent_ctx,
        zone_ids,
        c.source_span,
        diagnostics,
    );
    let xy_required = geom_required && !anchor_active;

    {
        let mut tokens = TokenEnv {
            referenced: referenced_token_ids,
            resolved: resolved_tokens,
        };
        check_optional_dim(
            &c.id,
            "x",
            c.x.as_ref(),
            xy_required,
            c.source_span,
            &mut tokens,
            diagnostics,
        );
        check_optional_dim(
            &c.id,
            "y",
            c.y.as_ref(),
            xy_required,
            c.source_span,
            &mut tokens,
            diagnostics,
        );
        check_optional_dim(
            &c.id,
            "w",
            c.w.as_ref(),
            geom_required,
            c.source_span,
            &mut tokens,
            diagnostics,
        );
        check_optional_dim(
            &c.id,
            "h",
            c.h.as_ref(),
            geom_required,
            c.source_span,
            &mut tokens,
            diagnostics,
        );
    }

    // Visual properties — token refs collected for token-usage checks, and the
    // shared per-corner-radius / stroke-dash guards. This mirrors the complete
    // set that check_pattern collects.
    let props = VisualProps {
        fill: c.fill.as_ref(),
        stroke: c.stroke.as_ref(),
        stroke_width: c.stroke_width.as_ref(),
        stroke_dash: c.stroke_dash.as_ref(),
        stroke_gap: c.stroke_gap.as_ref(),
        stroke_linecap: c.stroke_linecap.as_deref(),
        border_top: c.border_top.as_ref(),
        border_bottom: c.border_bottom.as_ref(),
        border_left: c.border_left.as_ref(),
        border_right: c.border_right.as_ref(),
        stroke_outer: c.stroke_outer.as_ref(),
        border_width: c.border_width.as_ref(),
        stroke_outer_width: c.stroke_outer_width.as_ref(),
        blend_mode: c.blend_mode.as_deref(),
        radius: c.radius.as_ref(),
        radius_tl: c.radius_tl.as_ref(),
        radius_tr: c.radius_tr.as_ref(),
        radius_br: c.radius_br.as_ref(),
        radius_bl: c.radius_bl.as_ref(),
        shadow: c.shadow.as_ref(),
        filter: c.filter.as_ref(),
        mask: c.mask.as_ref(),
        blur: c.blur.as_ref(),
    };
    check_visual_props(
        "chart",
        &c.id,
        c.source_span,
        props,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );

    // Chart-specific semantic checks.
    //
    // The renderer recognizes "bar", "line", "area", "sparkline", "pie", and
    // "donut"; any other kind string cannot render and is reported immediately.
    let kind_known = CHART_KINDS.contains(&c.kind.as_str());
    if !kind_known {
        diagnostics.push(Diagnostic::error(
            "chart.invalid_kind",
            invalid_value_message(&format!("chart '{}'", c.id), "kind", &c.kind, CHART_KINDS),
            c.source_span,
            Some(c.id.clone()),
        ));
    }

    // Validate bar-mode against the recognized set {"grouped", "stacked"}.
    // An unknown value is an Error: a typo would otherwise silently fall back
    // to the default at render time.
    if let Some(bar_mode) = &c.bar_mode {
        let bar_mode_known = CHART_BAR_MODES.contains(&bar_mode.as_str());
        if !bar_mode_known {
            diagnostics.push(Diagnostic::error(
                "chart.invalid_bar_mode",
                invalid_value_message(
                    &format!("chart '{}'", c.id),
                    "bar-mode",
                    bar_mode,
                    CHART_BAR_MODES,
                ),
                c.source_span,
                Some(c.id.clone()),
            ));
        }
    }

    // Validate orientation against the recognized set {"vertical", "horizontal"}.
    // An unknown value is an Error.
    if let Some(orientation) = &c.orientation {
        let orientation_known = CHART_ORIENTATIONS.contains(&orientation.as_str());
        if !orientation_known {
            diagnostics.push(Diagnostic::error(
                "chart.invalid_orientation",
                invalid_value_message(
                    &format!("chart '{}'", c.id),
                    "orientation",
                    orientation,
                    CHART_ORIENTATIONS,
                ),
                c.source_span,
                Some(c.id.clone()),
            ));
        }
    }

    // Validate point-placement against the recognized set {"edge", "center"}.
    if let Some(point_placement) = &c.point_placement {
        let point_placement_known = CHART_POINT_PLACEMENTS.contains(&point_placement.as_str());
        if !point_placement_known {
            diagnostics.push(Diagnostic::error(
                "chart.invalid_point_placement",
                invalid_value_message(
                    &format!("chart '{}'", c.id),
                    "point-placement",
                    point_placement,
                    CHART_POINT_PLACEMENTS,
                ),
                c.source_span,
                Some(c.id.clone()),
            ));
        }
    }

    // Validate value-labels against the recognized set {"auto", "none", "top", "center"}.
    if let Some(value_labels) = &c.value_labels {
        let value_labels_known = CHART_VALUE_LABELS.contains(&value_labels.as_str());
        if !value_labels_known {
            diagnostics.push(Diagnostic::error(
                "chart.invalid_value_labels",
                invalid_value_message(
                    &format!("chart '{}'", c.id),
                    "value-labels",
                    value_labels,
                    CHART_VALUE_LABELS,
                ),
                c.source_span,
                Some(c.id.clone()),
            ));
        }
    }

    // Validate legend-position against the recognized set {"right", "left", "top", "bottom"}.
    if let Some(legend_position) = &c.legend_position {
        let legend_position_known = CHART_LEGEND_POSITIONS.contains(&legend_position.as_str());
        if !legend_position_known {
            diagnostics.push(Diagnostic::error(
                "chart.invalid_legend_position",
                invalid_value_message(
                    &format!("chart '{}'", c.id),
                    "legend-position",
                    legend_position,
                    CHART_LEGEND_POSITIONS,
                ),
                c.source_span,
                Some(c.id.clone()),
            ));
        }
    }

    // Validate legend-layout against the recognized set {"wrapped", "list"}.
    if let Some(legend_layout) = &c.legend_layout {
        let legend_layout_known = CHART_LEGEND_LAYOUTS.contains(&legend_layout.as_str());
        if !legend_layout_known {
            diagnostics.push(Diagnostic::error(
                "chart.invalid_legend_layout",
                invalid_value_message(
                    &format!("chart '{}'", c.id),
                    "legend-layout",
                    legend_layout,
                    CHART_LEGEND_LAYOUTS,
                ),
                c.source_span,
                Some(c.id.clone()),
            ));
        }
    }

    // Validate legend-align against the recognized set {"center", "left", "right"}.
    if let Some(legend_align) = &c.legend_align {
        let legend_align_known = CHART_LEGEND_ALIGNS.contains(&legend_align.as_str());
        if !legend_align_known {
            diagnostics.push(Diagnostic::error(
                "chart.invalid_legend_align",
                invalid_value_message(
                    &format!("chart '{}'", c.id),
                    "legend-align",
                    legend_align,
                    CHART_LEGEND_ALIGNS,
                ),
                c.source_span,
                Some(c.id.clone()),
            ));
        }
    }

    // Validate categories count vs. series data length.
    // Emitted as Advisory (governable) when categories is non-empty and its count
    // does not match the maximum series value count.
    if !c.categories.is_empty() {
        let max_series_len = c.series.iter().map(|s| s.values.len()).max().unwrap_or(0);
        if c.categories.len() != max_series_len {
            diagnostics.push(Diagnostic::advisory(
                "chart.category_count_mismatch",
                format!(
                    "chart '{}': {} category labels but {} data points per series",
                    c.id,
                    c.categories.len(),
                    max_series_len,
                ),
                c.source_span,
                Some(c.id.clone()),
            ));
        }
    }

    // Series color token refs — series are pure data but their color props are
    // PropertyValue token refs that must be counted as used.
    for s in &c.series {
        if let Some(crate::ast::value::PropertyValue::TokenRef(token_id)) = &s.color {
            referenced_token_ids.insert(token_id.clone());
        }
        // Per-series label-color token ref.
        if let Some(crate::ast::value::PropertyValue::TokenRef(token_id)) = &s.label_color {
            referenced_token_ids.insert(token_id.clone());
        }
    }
    // value-color token ref — collect even though the series children are not walked.
    if let Some(crate::ast::value::PropertyValue::TokenRef(token_id)) = &c.value_color {
        referenced_token_ids.insert(token_id.clone());
    }
    // label-colors token refs — per-slice color refs from the label-colors child node.
    for pv in &c.label_colors {
        if let crate::ast::value::PropertyValue::TokenRef(token_id) = pv {
            referenced_token_ids.insert(token_id.clone());
        }
    }
    // slice-colors token refs — per-slice FILL color refs from the slice-colors child node.
    for pv in &c.slice_colors {
        if let crate::ast::value::PropertyValue::TokenRef(token_id) = pv {
            referenced_token_ids.insert(token_id.clone());
        }
    }

    // Unknown properties.
    check_unknown_props("chart", &c.id, &c.unknown_props, c.source_span, diagnostics);
}
