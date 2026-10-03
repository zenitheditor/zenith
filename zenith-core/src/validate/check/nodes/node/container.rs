//! Per-kind checks for the container nodes `frame`, `group`, and `table`.
//!
//! These functions emit each container's OWN diagnostics. The child recursion
//! (building a fresh [`super::super::nodes::WalkPos`] and descending) stays in
//! the dispatcher [`super::super::nodes::walk_node`] so traversal/emit order is
//! unchanged.

use std::collections::BTreeSet;

use crate::ast::node::{FrameNode, GroupNode, LayoutKind, TableNode};
use crate::ast::value::{Dimension, dim_to_px};
use crate::diagnostics::Diagnostic;
use crate::schema::enums::{BORDER_COLLAPSES, H_ALIGNS, SYMMETRY_MODES, V_ALIGNS};
use crate::suggest::{invalid_value_message, replace_value_fix};

use super::shared::{
    AnchorParentCtx, AnchorProps, TokenEnv, VisualProps, check_anchor, check_optional_dim,
    check_style_ref, check_visual_props,
};
use super::suggest::check_unknown_props;
use crate::validate::check::nodes::WalkCtx;
use crate::validate::check::register_id;
use crate::validate::check::visual::{VisualExpect, check_visual_prop};

pub(in crate::validate::check) fn check_frame(
    f: &FrameNode,
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
    register_id(&f.id, seen_ids, diagnostics);
    check_style_ref(
        &f.id,
        f.style.as_deref(),
        declared_style_ids,
        f.source_span,
        diagnostics,
    );

    // A recognized anchor supplies both x and y.
    let anchor_active = check_anchor(
        &f.id,
        AnchorProps {
            anchor: f.anchor.as_deref(),
            anchor_zone: f.anchor_zone.as_deref(),
            anchor_sibling: f.anchor_sibling.as_deref(),
            anchor_parent: f.anchor_parent == Some(true),
            anchor_edge: f.anchor_edge.as_deref(),
            anchor_gap: f.anchor_gap.as_ref(),
        },
        parent_ctx,
        zone_ids,
        f.source_span,
        diagnostics,
    );
    let xy_required = geom_required && !anchor_active;
    // A layout frame without w/h hugs its children; the layout check reports
    // that, so a missing w/h is not `node.missing_geometry` here.
    let wh_required = geom_required
        && !f
            .layout
            .as_ref()
            .is_some_and(LayoutKind::positions_children);

    // Frames REQUIRE all four geometry dimensions (unlike groups).
    {
        let mut tokens = TokenEnv {
            referenced: referenced_token_ids,
            resolved: resolved_tokens,
        };
        check_optional_dim(
            &f.id,
            "x",
            f.x.as_ref(),
            xy_required,
            f.source_span,
            &mut tokens,
            diagnostics,
        );
        check_optional_dim(
            &f.id,
            "y",
            f.y.as_ref(),
            xy_required,
            f.source_span,
            &mut tokens,
            diagnostics,
        );
        check_optional_dim(
            &f.id,
            "w",
            f.w.as_ref(),
            wh_required,
            f.source_span,
            &mut tokens,
            diagnostics,
        );
        check_optional_dim(
            &f.id,
            "h",
            f.h.as_ref(),
            wh_required,
            f.source_span,
            &mut tokens,
            diagnostics,
        );
    }

    check_visual_props(
        "frame",
        &f.id,
        f.source_span,
        VisualProps {
            fill: f.fill.as_ref(),
            stroke: f.stroke.as_ref(),
            stroke_width: f.stroke_width.as_ref(),
            stroke_dash: None,
            stroke_gap: None,
            stroke_linecap: None,
            border_top: None,
            border_bottom: None,
            border_left: None,
            border_right: None,
            stroke_outer: None,
            border_width: None,
            stroke_outer_width: None,
            blend_mode: f.blend_mode.as_deref(),
            radius: f.radius.as_ref(),
            radius_tl: None,
            radius_tr: None,
            radius_br: None,
            radius_bl: None,
            shadow: f.shadow.as_ref(),
            filter: f.filter.as_ref(),
            mask: f.mask.as_ref(),
            blur: f.blur.as_ref(),
        },
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );

    // Grid layout advisory: `layout="grid"` without a positive `columns`
    // defaults the scene to a single column. Non-fatal.
    if f.layout == Some(LayoutKind::Grid) && f.columns.unwrap_or(0) == 0 {
        diagnostics.push(Diagnostic::advisory(
            "grid.missing_columns",
            format!(
                "frame '{}' uses layout=\"grid\" without a positive `columns`; \
                 defaulting to 1 column",
                f.id
            ),
            f.source_span,
            Some(f.id.clone()),
        ));
    }

    // Unknown properties.
    check_unknown_props("frame", &f.id, &f.unknown_props, f.source_span, diagnostics);
}

pub(in crate::validate::check) fn check_group(
    g: &GroupNode,
    ctx: WalkCtx,
    seen_ids: &mut BTreeSet<String>,
    referenced_token_ids: &mut BTreeSet<String>,
    parent_ctx: AnchorParentCtx,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let WalkCtx {
        resolved_tokens,
        declared_style_ids,
        zone_ids,
        ..
    } = ctx;
    register_id(&g.id, seen_ids, diagnostics);
    check_style_ref(
        &g.id,
        g.style.as_deref(),
        declared_style_ids,
        g.source_span,
        diagnostics,
    );

    // Groups have NO required geometry — x/y/w/h are all advisory.
    // Still validate the anchor value if present.
    check_anchor(
        &g.id,
        AnchorProps {
            anchor: g.anchor.as_deref(),
            anchor_zone: g.anchor_zone.as_deref(),
            anchor_sibling: g.anchor_sibling.as_deref(),
            anchor_parent: g.anchor_parent == Some(true),
            anchor_edge: g.anchor_edge.as_deref(),
            anchor_gap: g.anchor_gap.as_ref(),
        },
        parent_ctx,
        zone_ids,
        g.source_span,
        diagnostics,
    );

    // Geometry is never required for a group, but a `(token)` dimension ref must
    // still be registered + type-checked (and a bad unit / non-dimension value
    // diagnosed) exactly like any other geometry property.
    {
        let mut tokens = TokenEnv {
            referenced: referenced_token_ids,
            resolved: resolved_tokens,
        };
        for (prop, value) in [
            ("x", g.x.as_ref()),
            ("y", g.y.as_ref()),
            ("w", g.w.as_ref()),
            ("h", g.h.as_ref()),
        ] {
            check_optional_dim(
                &g.id,
                prop,
                value,
                false,
                g.source_span,
                &mut tokens,
                diagnostics,
            );
        }
    }

    check_visual_props(
        "group",
        &g.id,
        g.source_span,
        VisualProps {
            fill: None,
            stroke: None,
            stroke_width: None,
            stroke_dash: None,
            stroke_gap: None,
            stroke_linecap: None,
            border_top: None,
            border_bottom: None,
            border_left: None,
            border_right: None,
            stroke_outer: None,
            border_width: None,
            stroke_outer_width: None,
            blend_mode: g.blend_mode.as_deref(),
            radius: None,
            radius_tl: None,
            radius_tr: None,
            radius_br: None,
            radius_bl: None,
            shadow: g.shadow.as_ref(),
            filter: g.filter.as_ref(),
            mask: g.mask.as_ref(),
            blur: g.blur.as_ref(),
        },
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );

    if let Some(v) = g.intensity
        && !(0.0..=1.0).contains(&v)
    {
        diagnostics.push(Diagnostic::warning(
            "group.invalid_intensity",
            format!("group '{}': intensity {v} is out of range 0.0..=1.0", g.id),
            g.source_span,
            Some(g.id.clone()),
        ));
    }

    check_group_symmetry(g, diagnostics);

    // Unknown properties.
    check_unknown_props("group", &g.id, &g.unknown_props, g.source_span, diagnostics);
}

fn check_group_symmetry(g: &GroupNode, diagnostics: &mut Vec<Diagnostic>) {
    if let Some(mode) = g.symmetry_mode.as_deref()
        && !SYMMETRY_MODES.contains(&mode)
    {
        diagnostics.push(
            Diagnostic::error(
                "group.invalid_symmetry",
                invalid_value_message(
                    &format!("group '{}'", g.id),
                    "symmetry-mode",
                    mode,
                    SYMMETRY_MODES,
                ),
                g.source_span,
                Some(g.id.clone()),
            )
            .with_fix(replace_value_fix("symmetry-mode", mode, SYMMETRY_MODES)),
        );
    }

    let Some(count) = g.symmetry_count else {
        return;
    };
    if count == 0 || count > 72 {
        diagnostics.push(Diagnostic::error(
            "group.invalid_symmetry",
            format!(
                "group '{}': symmetry-count {count} is out of range 1..=72",
                g.id
            ),
            g.source_span,
            Some(g.id.clone()),
        ));
        return;
    }
    if count <= 1 {
        return;
    }

    check_symmetry_center_dimension(g, "symmetry-cx", g.symmetry_cx.as_ref(), diagnostics);
    check_symmetry_center_dimension(g, "symmetry-cy", g.symmetry_cy.as_ref(), diagnostics);
    check_symmetry_start_angle(g, g.symmetry_start_angle.as_ref(), diagnostics);
}

fn check_symmetry_center_dimension(
    g: &GroupNode,
    field: &str,
    value: Option<&Dimension>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(value) = value else {
        diagnostics.push(Diagnostic::error(
            "group.invalid_symmetry",
            format!("group '{}': live symmetry requires {field}", g.id),
            g.source_span,
            Some(g.id.clone()),
        ));
        return;
    };

    let Some(px) = dim_to_px(value.value, &value.unit) else {
        diagnostics.push(Diagnostic::error(
            "group.invalid_symmetry",
            format!("group '{}': {field} must use a px/pt dimension", g.id),
            g.source_span,
            Some(g.id.clone()),
        ));
        return;
    };
    if !px.is_finite() {
        diagnostics.push(Diagnostic::error(
            "group.invalid_symmetry",
            format!("group '{}': {field} must resolve to a finite value", g.id),
            g.source_span,
            Some(g.id.clone()),
        ));
    }
}

fn check_symmetry_start_angle(
    g: &GroupNode,
    value: Option<&Dimension>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(value) = value else {
        return;
    };
    if !value.value.is_finite() {
        diagnostics.push(Diagnostic::error(
            "group.invalid_symmetry",
            format!("group '{}': symmetry-start-angle must be finite", g.id),
            g.source_span,
            Some(g.id.clone()),
        ));
    }
}

pub(in crate::validate::check) fn check_table(
    t: &TableNode,
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
    register_id(&t.id, seen_ids, diagnostics);
    check_style_ref(
        &t.id,
        t.style.as_deref(),
        declared_style_ids,
        t.source_span,
        diagnostics,
    );
    // `header-style` is a style ref carried for a later unit; validate it
    // against the declared styles now so authoring errors surface early.
    check_style_ref(
        &t.id,
        t.header_style.as_deref(),
        declared_style_ids,
        t.source_span,
        diagnostics,
    );

    // A recognized anchor supplies both x and y.
    let anchor_active = check_anchor(
        &t.id,
        AnchorProps {
            anchor: t.anchor.as_deref(),
            anchor_zone: t.anchor_zone.as_deref(),
            anchor_sibling: t.anchor_sibling.as_deref(),
            anchor_parent: t.anchor_parent == Some(true),
            anchor_edge: t.anchor_edge.as_deref(),
            anchor_gap: t.anchor_gap.as_ref(),
        },
        parent_ctx,
        zone_ids,
        t.source_span,
        diagnostics,
    );
    let xy_required = geom_required && !anchor_active;

    // Required geometry: x, y, w, h must all be present (mirror frame).
    {
        let mut tokens = TokenEnv {
            referenced: referenced_token_ids,
            resolved: resolved_tokens,
        };
        check_optional_dim(
            &t.id,
            "x",
            t.x.as_ref(),
            xy_required,
            t.source_span,
            &mut tokens,
            diagnostics,
        );
        check_optional_dim(
            &t.id,
            "y",
            t.y.as_ref(),
            xy_required,
            t.source_span,
            &mut tokens,
            diagnostics,
        );
        check_optional_dim(
            &t.id,
            "w",
            t.w.as_ref(),
            geom_required,
            t.source_span,
            &mut tokens,
            diagnostics,
        );
        check_optional_dim(
            &t.id,
            "h",
            t.h.as_ref(),
            geom_required,
            t.source_span,
            &mut tokens,
            diagnostics,
        );
    }

    // Token-typed visual props: colors and dimensions.
    for (prop_name, prop_val) in [
        ("fill", t.fill.as_ref()),
        ("border", t.border.as_ref()),
        ("header-fill", t.header_fill.as_ref()),
    ] {
        check_visual_prop(
            &t.id,
            prop_name,
            prop_val,
            VisualExpect::Color,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
    }
    for (prop_name, prop_val) in [
        ("border-width", t.border_width.as_ref()),
        ("gap", t.gap.as_ref()),
        ("cell-padding", t.cell_padding.as_ref()),
    ] {
        check_visual_prop(
            &t.id,
            prop_name,
            prop_val,
            VisualExpect::Dimension,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
    }

    // Enum-value checks (an unrecognized value is an Error).
    if let Some(ha) = t.h_align.as_deref()
        && !H_ALIGNS.contains(&ha)
    {
        diagnostics.push(
            Diagnostic::error(
                "table.invalid_h_align",
                invalid_value_message(&format!("table '{}'", t.id), "h-align", ha, H_ALIGNS),
                t.source_span,
                Some(t.id.clone()),
            )
            .with_fix(replace_value_fix("h-align", ha, H_ALIGNS)),
        );
    }
    if let Some(va) = t.v_align.as_deref()
        && !V_ALIGNS.contains(&va)
    {
        diagnostics.push(
            Diagnostic::error(
                "table.invalid_v_align",
                invalid_value_message(&format!("table '{}'", t.id), "v-align", va, V_ALIGNS),
                t.source_span,
                Some(t.id.clone()),
            )
            .with_fix(replace_value_fix("v-align", va, V_ALIGNS)),
        );
    }
    if let Some(bc) = t.border_collapse.as_deref()
        && !BORDER_COLLAPSES.contains(&bc)
    {
        diagnostics.push(
            Diagnostic::error(
                "table.invalid_border_collapse",
                invalid_value_message(
                    &format!("table '{}'", t.id),
                    "border-collapse",
                    bc,
                    BORDER_COLLAPSES,
                ),
                t.source_span,
                Some(t.id.clone()),
            )
            .with_fix(replace_value_fix("border-collapse", bc, BORDER_COLLAPSES)),
        );
    }

    // Per-cell enum checks, mirroring the table-level checks.
    for row in &t.rows {
        for cell in &row.cells {
            if let Some(ha) = cell.h_align.as_deref()
                && !H_ALIGNS.contains(&ha)
            {
                diagnostics.push(Diagnostic::error(
                    "table.invalid_h_align",
                    invalid_value_message(
                        &format!("table '{}'", t.id),
                        "cell h-align",
                        ha,
                        H_ALIGNS,
                    ),
                    cell.source_span,
                    Some(t.id.clone()),
                ));
            }
            if let Some(va) = cell.v_align.as_deref()
                && !V_ALIGNS.contains(&va)
            {
                diagnostics.push(Diagnostic::error(
                    "table.invalid_v_align",
                    invalid_value_message(
                        &format!("table '{}'", t.id),
                        "cell v-align",
                        va,
                        V_ALIGNS,
                    ),
                    cell.source_span,
                    Some(t.id.clone()),
                ));
            }
            // Per-cell token-typed visual props.
            check_visual_prop(
                &t.id,
                "cell fill",
                cell.fill.as_ref(),
                VisualExpect::Color,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );
            check_visual_prop(
                &t.id,
                "cell border",
                cell.border.as_ref(),
                VisualExpect::Color,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );
            check_visual_prop(
                &t.id,
                "cell border-width",
                cell.border_width.as_ref(),
                VisualExpect::Dimension,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );
        }
    }

    // Unknown properties on the table node itself.
    check_unknown_props("table", &t.id, &t.unknown_props, t.source_span, diagnostics);

    // Unknown properties on each column declaration.
    for col in &t.columns {
        check_unknown_props(
            "column",
            &t.id,
            &col.unknown_props,
            col.source_span,
            diagnostics,
        );
    }

    // Unknown properties on each row and cell.
    for row in &t.rows {
        check_unknown_props(
            "row",
            &t.id,
            &row.unknown_props,
            row.source_span,
            diagnostics,
        );
        for cell in &row.cells {
            check_unknown_props(
                "cell",
                &t.id,
                &cell.unknown_props,
                cell.source_span,
                diagnostics,
            );
        }
    }

    // ── Cell-span consistency ──────────────────────────────────────
    // HTML-table cell flow: place each cell in the next free column of
    // its row, honoring colspan/rowspan via a BTreeSet occupancy grid
    // keyed by (row, col). A colspan that would run past the column
    // count, or a rowspan past the last row, is a hard error.
    let col_count = t.columns.len().max(1);
    let row_count = t.rows.len();
    let mut occupied: BTreeSet<(usize, usize)> = BTreeSet::new();
    for (r, row) in t.rows.iter().enumerate() {
        let mut col_cursor = 0usize;
        for cell in &row.cells {
            // Advance to the next column free at this row.
            while col_cursor < col_count && occupied.contains(&(r, col_cursor)) {
                col_cursor += 1;
            }
            let cs = cell.colspan.max(1) as usize;
            let rs = cell.rowspan.max(1) as usize;
            if col_cursor + cs > col_count {
                diagnostics.push(Diagnostic::error(
                    "table.cell_overflow",
                    format!(
                        "table '{}': cell at row {r} starting column {col_cursor} with \
                         colspan {cs} exceeds the column count {col_count}",
                        t.id
                    ),
                    cell.source_span,
                    Some(t.id.clone()),
                ));
            }
            if r + rs > row_count {
                diagnostics.push(Diagnostic::error(
                    "table.cell_overflow",
                    format!(
                        "table '{}': cell at row {r} with rowspan {rs} extends past the \
                         last row (row count {row_count})",
                        t.id
                    ),
                    cell.source_span,
                    Some(t.id.clone()),
                ));
            }
            // Mark the cell's covered slots (clamped to the grid).
            for dr in 0..rs {
                for dc in 0..cs {
                    let cr = r + dr;
                    let cc = col_cursor + dc;
                    if cr < row_count && cc < col_count {
                        occupied.insert((cr, cc));
                    }
                }
            }
            col_cursor += cs;
        }
    }
}
