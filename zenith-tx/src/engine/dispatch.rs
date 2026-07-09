//! Per-op dispatch: [`apply_op`] routes each [`Op`] to its handler.

use zenith_core::{Diagnostic, Document};

use super::asset::{AddAssetSpec, apply_add_asset, apply_set_asset};
use super::fill_rule::apply_set_fill_rule;
use super::flags::{apply_set_locked, apply_set_points, apply_set_visible};
use super::geometry::{
    GeometryDelta, apply_align_nodes, apply_align_to_edge, apply_distribute_nodes,
    apply_set_geometry,
};
use super::path::{
    MakePathSymmetricArgs, MovePathAnchorArgs, MovePathHandleArgs, PathBooleanArgs,
    apply_insert_path_anchor, apply_insert_path_anchor_at_point, apply_make_path_symmetric,
    apply_move_path_anchor, apply_move_path_handle, apply_path_boolean, apply_remove_path_anchor,
    apply_set_path_anchor_kind, apply_set_path_anchors, apply_simplify_path_anchors,
    apply_snap_path_anchors, apply_transform_path_anchors,
};
use super::pattern::apply_detach_pattern;
use super::recipe::{RecipeScalars, apply_create_recipe, apply_delete_recipe, apply_update_recipe};
use super::structure;
use super::structure::{
    AddPathSpec, ReorderKind, apply_add_node, apply_add_page, apply_add_path, apply_create_master,
    apply_delete_master, apply_delete_page, apply_duplicate_node, apply_duplicate_page,
    apply_group, apply_remove_node, apply_reorder, apply_reorder_pages, apply_reparent,
    apply_set_page_master, apply_set_page_size, apply_ungroup,
};
use super::style::{
    apply_create_style, apply_delete_style, apply_find_replace_text, apply_replace_text,
    apply_set_fill, apply_set_opacity, apply_set_stroke, apply_set_stroke_width,
    apply_set_style_property, apply_set_text_align, apply_set_text_direction,
    apply_set_text_overflow,
};
use super::token::{
    CreateTokenBody, CreateTokenScalars, apply_create_token, apply_update_token_value,
};
use crate::op::Op;

pub(super) fn apply_op(
    op: &Op,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    match op {
        Op::SetTextAlign {
            node: node_id,
            align,
        } => {
            apply_set_text_align(node_id, align, doc, diagnostics, affected);
        }
        Op::MoveForward { node: node_id } => {
            apply_reorder(node_id, ReorderKind::Forward, doc, diagnostics, affected);
        }
        Op::MoveBackward { node: node_id } => {
            apply_reorder(node_id, ReorderKind::Backward, doc, diagnostics, affected);
        }
        Op::MoveToFront { node: node_id } => {
            apply_reorder(node_id, ReorderKind::ToFront, doc, diagnostics, affected);
        }
        Op::MoveToBack { node: node_id } => {
            apply_reorder(node_id, ReorderKind::ToBack, doc, diagnostics, affected);
        }
        Op::SetFill {
            node: node_id,
            fill,
        } => {
            apply_set_fill(node_id, fill, doc, diagnostics, affected);
        }
        Op::SetFillRule {
            node: node_id,
            fill_rule,
        } => {
            apply_set_fill_rule(node_id, fill_rule, doc, diagnostics, affected);
        }
        Op::SetStroke {
            node: node_id,
            stroke,
        } => {
            apply_set_stroke(node_id, stroke, doc, diagnostics, affected);
        }
        Op::SetStrokeWidth {
            node: node_id,
            stroke_width,
        } => {
            apply_set_stroke_width(node_id, stroke_width, doc, diagnostics, affected);
        }
        Op::SetVisible {
            node: node_id,
            visible,
        } => {
            apply_set_visible(node_id, *visible, doc, diagnostics, affected);
        }
        Op::SetLocked {
            node: node_id,
            locked,
        } => {
            apply_set_locked(node_id, *locked, doc, diagnostics, affected);
        }
        Op::SetGeometry {
            node: node_id,
            x,
            y,
            w,
            h,
            rotate,
        } => {
            apply_set_geometry(
                node_id,
                GeometryDelta {
                    x: *x,
                    y: *y,
                    w: *w,
                    h: *h,
                    rotate: *rotate,
                },
                doc,
                diagnostics,
                affected,
            );
        }
        Op::SetPoints {
            node: node_id,
            points,
        } => {
            apply_set_points(node_id, points, doc, diagnostics, affected);
        }
        Op::SetPathAnchors {
            node: node_id,
            subpath_index,
            anchors,
        } => {
            apply_set_path_anchors(node_id, *subpath_index, anchors, doc, diagnostics, affected);
        }
        Op::SetPathAnchorKind {
            node: node_id,
            subpath_index,
            anchor_index,
            kind,
        } => {
            apply_set_path_anchor_kind(
                node_id,
                *subpath_index,
                *anchor_index,
                kind.as_deref(),
                doc,
                diagnostics,
                affected,
            );
        }
        Op::RemovePathAnchor {
            node: node_id,
            subpath_index,
            anchor_index,
        } => {
            apply_remove_path_anchor(
                node_id,
                *subpath_index,
                *anchor_index,
                doc,
                diagnostics,
                affected,
            );
        }
        Op::InsertPathAnchor {
            node: node_id,
            subpath_index,
            segment_index,
            t,
        } => {
            apply_insert_path_anchor(
                node_id,
                *subpath_index,
                *segment_index,
                *t,
                doc,
                diagnostics,
                affected,
            );
        }
        Op::InsertPathAnchorAtPoint {
            node: node_id,
            x,
            y,
            tolerance,
        } => {
            apply_insert_path_anchor_at_point(
                node_id,
                *x,
                *y,
                *tolerance,
                doc,
                diagnostics,
                affected,
            );
        }
        Op::MovePathAnchor {
            node: node_id,
            subpath_index,
            anchor_index,
            dx,
            dy,
        } => {
            apply_move_path_anchor(
                MovePathAnchorArgs {
                    node_id,
                    subpath_index: *subpath_index,
                    anchor_index: *anchor_index,
                    dx: *dx,
                    dy: *dy,
                },
                doc,
                diagnostics,
                affected,
            );
        }
        Op::MovePathHandle {
            node: node_id,
            subpath_index,
            anchor_index,
            handle,
            dx,
            dy,
        } => {
            apply_move_path_handle(
                MovePathHandleArgs {
                    node_id,
                    subpath_index: *subpath_index,
                    anchor_index: *anchor_index,
                    handle: *handle,
                    dx: *dx,
                    dy: *dy,
                },
                doc,
                diagnostics,
                affected,
            );
        }
        Op::SimplifyPathAnchors {
            node: node_id,
            subpath_index,
            tolerance,
        } => {
            apply_simplify_path_anchors(
                node_id,
                *subpath_index,
                *tolerance,
                doc,
                diagnostics,
                affected,
            );
        }
        Op::TransformPathAnchors {
            node: node_id,
            transform,
        } => {
            apply_transform_path_anchors(node_id, transform, doc, diagnostics, affected);
        }
        Op::SnapPathAnchors {
            node: node_id,
            target,
            tolerance,
        } => {
            apply_snap_path_anchors(node_id, target, *tolerance, doc, diagnostics, affected);
        }
        Op::MakePathSymmetric {
            node: node_id,
            id_prefix,
            count,
            cx,
            cy,
            start_angle_degrees,
            mirror,
        } => {
            apply_make_path_symmetric(
                MakePathSymmetricArgs {
                    node_id,
                    id_prefix,
                    count: *count,
                    cx: *cx,
                    cy: *cy,
                    start_angle_degrees: *start_angle_degrees,
                    mirror: *mirror,
                },
                doc,
                diagnostics,
                affected,
            );
        }
        Op::PathBoolean {
            node: node_id,
            target,
            new_id,
            operation,
            tolerance,
        } => {
            apply_path_boolean(
                PathBooleanArgs {
                    node_id,
                    target_id: target,
                    new_id,
                    operation: *operation,
                    tolerance: *tolerance,
                },
                doc,
                diagnostics,
                affected,
            );
        }
        Op::AddNode {
            parent,
            position,
            source,
        } => {
            apply_add_node(parent, position, source, doc, diagnostics, affected);
        }
        Op::AddPath {
            parent,
            id,
            position,
            closed,
            anchors,
            subpaths,
        } => {
            apply_add_path(
                AddPathSpec {
                    parent,
                    id,
                    position,
                    closed: *closed,
                    anchors,
                    subpaths,
                },
                doc,
                diagnostics,
                affected,
            );
        }
        Op::RemoveNode { node: node_id } => {
            apply_remove_node(node_id, doc, diagnostics, affected);
        }
        Op::SetOpacity {
            node: node_id,
            opacity,
        } => {
            apply_set_opacity(node_id, *opacity, doc, diagnostics, affected);
        }
        Op::ReplaceText {
            node: node_id,
            spans,
        } => {
            apply_replace_text(node_id, spans, doc, diagnostics, affected);
        }
        Op::DuplicateNode {
            node: node_id,
            new_id,
        } => {
            apply_duplicate_node(node_id, new_id, doc, diagnostics, affected);
        }
        Op::DuplicatePage {
            page,
            new_id,
            id_suffix,
        } => {
            apply_duplicate_page(page, new_id, id_suffix, doc, diagnostics, affected);
        }
        Op::Group { node_ids, group_id } => {
            apply_group(node_ids, group_id, doc, diagnostics, affected);
        }
        Op::Ungroup { group_id } => {
            apply_ungroup(group_id, doc, diagnostics, affected);
        }
        Op::Reparent {
            node: node_id,
            new_parent,
            position,
        } => {
            apply_reparent(node_id, new_parent, position, doc, diagnostics, affected);
        }
        Op::AlignNodes {
            node_ids,
            align,
            anchor,
        } => {
            apply_align_nodes(node_ids, align, anchor, doc, diagnostics, affected);
        }
        Op::SetTextOverflow { node_id, overflow } => {
            apply_set_text_overflow(node_id, overflow, doc, diagnostics, affected);
        }
        Op::DistributeNodes { node_ids, axis } => {
            apply_distribute_nodes(node_ids, axis, doc, diagnostics, affected);
        }
        Op::AddPage {
            id,
            w,
            h,
            background,
            index,
        } => {
            let spec = structure::AddPageSpec {
                id,
                w,
                h,
                background: background.as_deref(),
                index: *index,
            };
            apply_add_page(&spec, doc, diagnostics, affected);
        }
        Op::DeletePage { page } => {
            apply_delete_page(page, doc, diagnostics, affected);
        }
        Op::ReorderPages { order } => {
            apply_reorder_pages(order, doc, diagnostics, affected);
        }
        Op::AddAsset {
            id,
            kind,
            src,
            sha256,
            metadata,
        } => {
            let spec = AddAssetSpec {
                id,
                kind,
                src,
                sha256: sha256.as_deref(),
                producer_kind: metadata.producer_kind.as_deref(),
                producer_source: metadata.producer_source.as_deref(),
                ai_prompt: metadata.ai_prompt.as_deref(),
                ai_model: metadata.ai_model.as_deref(),
                ai_provider: metadata.ai_provider.as_deref(),
                ai_seed: metadata.ai_seed,
                ai_generation_date: metadata.ai_generation_date.as_deref(),
                ai_license: metadata.ai_license.as_deref(),
                ai_source_rights: metadata.ai_source_rights.as_deref(),
                ai_safety_status: metadata.ai_safety_status.as_deref(),
                ai_reuse_policy: metadata.ai_reuse_policy.as_deref(),
            };
            apply_add_asset(&spec, doc, diagnostics, affected);
        }
        Op::SetAsset { node_id, asset_id } => {
            apply_set_asset(node_id, asset_id, doc, diagnostics, affected);
        }
        Op::CreateToken {
            id,
            token_type,
            value,
            set,
            layers,
            filter_ops,
            stops,
            angle,
            radial,
            center_x,
            center_y,
            radius,
            shape,
            feather,
            invert,
        } => {
            apply_create_token(
                &CreateTokenScalars {
                    id,
                    token_type,
                    value,
                    set: set.as_deref(),
                },
                &CreateTokenBody {
                    layers,
                    filter_ops,
                    stops,
                    angle: *angle,
                    radial: *radial,
                    center_x: *center_x,
                    center_y: *center_y,
                    radius: *radius,
                    shape: shape.as_deref(),
                    feather: *feather,
                    invert: *invert,
                },
                doc,
                diagnostics,
                affected,
            );
        }
        Op::UpdateTokenValue { id, value, set } => {
            apply_update_token_value(id, value, set.as_deref(), doc, diagnostics, affected);
        }
        Op::SetStyleProperty {
            style_id,
            property,
            value,
        } => {
            apply_set_style_property(style_id, property, value, doc, diagnostics, affected);
        }
        Op::CreateStyle { id, properties } => {
            apply_create_style(id, properties, doc, diagnostics, affected);
        }
        Op::DeleteStyle { id } => {
            apply_delete_style(id, doc, diagnostics, affected);
        }
        Op::CreateMaster { id } => {
            apply_create_master(id, doc, diagnostics, affected);
        }
        Op::DeleteMaster { id } => {
            apply_delete_master(id, doc, diagnostics, affected);
        }
        Op::SetPageMaster { page, master } => {
            apply_set_page_master(page, master.as_deref(), doc, diagnostics, affected);
        }
        Op::SetTextDirection { node, direction } => {
            apply_set_text_direction(node, direction, doc, diagnostics, affected);
        }
        Op::FindReplaceText {
            find,
            replace,
            node,
        } => {
            apply_find_replace_text(find, replace, node.as_deref(), doc, diagnostics, affected);
        }
        Op::SetPageSize { page, w, h } => {
            apply_set_page_size(page, w, h, doc, diagnostics, affected);
        }
        Op::AlignToEdge { node, edge, margin } => {
            apply_align_to_edge(node, edge, *margin, doc, diagnostics, affected);
        }
        Op::CreateRecipe {
            id,
            kind,
            seed,
            generator,
            bounds,
            detached,
        } => {
            apply_create_recipe(
                RecipeScalars {
                    id,
                    kind,
                    seed: *seed,
                    generator: generator.as_deref(),
                    bounds: bounds.as_deref(),
                    detached: *detached,
                },
                doc,
                diagnostics,
                affected,
            );
        }
        Op::UpdateRecipe {
            id,
            kind,
            seed,
            generator,
            bounds,
            detached,
        } => {
            apply_update_recipe(
                RecipeScalars {
                    id,
                    kind,
                    seed: *seed,
                    generator: generator.as_deref(),
                    bounds: bounds.as_deref(),
                    detached: *detached,
                },
                doc,
                diagnostics,
                affected,
            );
        }
        Op::DeleteRecipe { id } => {
            apply_delete_recipe(id, doc, diagnostics, affected);
        }
        Op::DetachPattern { node: node_id } => {
            apply_detach_pattern(node_id, doc, diagnostics, affected);
        }
    }
}
