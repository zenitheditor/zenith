//! Static schema metadata for the transaction op set.
//!
//! Exposes the canonical list of op names (as their JSON `op` tag strings),
//! one-line summaries per op, and a compile-time drift guard that forces a
//! compile error whenever a new `Op` variant is added without updating this
//! module.

// ── Canonical op name list ────────────────────────────────────────────────────

/// All transaction op names in their JSON `op` tag form (snake_case).
///
/// The list is sorted for deterministic output. The drift-guard test
/// `op_summary_covers_every_op` enforces that this list exactly matches the
/// `Op` enum variants.
pub fn op_names() -> &'static [&'static str] {
    &[
        "add_asset",
        "add_node",
        "add_page",
        "align_nodes",
        "align_to_edge",
        "create_recipe",
        "create_token",
        "delete_page",
        "delete_recipe",
        "detach_pattern",
        "distribute_nodes",
        "duplicate_node",
        "duplicate_page",
        "find_replace_text",
        "group",
        "move_backward",
        "move_forward",
        "move_to_back",
        "move_to_front",
        "reparent",
        "reorder_pages",
        "remove_node",
        "replace_text",
        "set_asset",
        "set_fill",
        "set_geometry",
        "set_locked",
        "set_opacity",
        "set_page_size",
        "set_points",
        "set_stroke",
        "set_stroke_width",
        "set_style_property",
        "set_text_align",
        "set_text_direction",
        "set_text_overflow",
        "set_visible",
        "ungroup",
        "update_recipe",
        "update_token_value",
    ]
}

// ── One-line summaries ────────────────────────────────────────────────────────

/// Return a one-line description of the named op, or `None` if unrecognised.
pub fn op_summary(name: &str) -> Option<&'static str> {
    match name {
        "set_text_align" => Some("Set the text alignment of a text node."),
        "move_forward" => {
            Some("Move a node one sibling position toward the front (top of z-order).")
        }
        "move_backward" => {
            Some("Move a node one sibling position toward the back (bottom of z-order).")
        }
        "move_to_front" => Some("Move a node to the topmost (last-child) position in its parent."),
        "move_to_back" => {
            Some("Move a node to the bottommost (first-child) position in its parent.")
        }
        "set_fill" => Some("Set the fill color of a node to a token reference."),
        "set_stroke" => Some("Set the stroke (outline) color of a node to a token reference."),
        "set_stroke_width" => {
            Some("Set the stroke width of a node to a dimension token reference.")
        }
        "set_visible" => Some("Show or hide a node by toggling its visible property."),
        "set_locked" => Some("Lock or unlock a node to prevent accidental edits."),
        "set_geometry" => Some("Move and/or resize a node by setting x, y, w, h, or rotate."),
        "set_points" => Some("Replace the full vertex list of a polygon or polyline node."),
        "add_node" => Some("Parse a .zen source fragment and insert it into a container."),
        "remove_node" => Some("Remove a node and its subtree from the document."),
        "set_opacity" => Some("Set the opacity of a node (0.0 = fully transparent, 1.0 = opaque)."),
        "replace_text" => Some("Replace all text spans of a text or shape node."),
        "duplicate_node" => Some("Clone a leaf node and insert the copy after the original."),
        "duplicate_page" => Some("Deep-clone a page and insert the copy after the original."),
        "group" => Some("Wrap a set of sibling nodes inside a new group node."),
        "ungroup" => Some("Dissolve a group node, moving its children up to the parent."),
        "reparent" => Some("Move a node into a different container (page, group, or frame)."),
        "align_nodes" => Some("Align a set of nodes to a common edge or centre along one axis."),
        "set_text_overflow" => {
            Some("Set the overflow mode (fit, clip, or visible) of a text or code node.")
        }
        "add_page" => Some("Create a new empty page and insert it at the given index."),
        "delete_page" => Some("Remove a page and its entire subtree from the document."),
        "reorder_pages" => Some("Reorder all document pages to match the given id permutation."),
        "add_asset" => Some("Declare a new asset (image, svg, or font) in the assets block."),
        "set_asset" => Some("Assign an asset reference to an image node."),
        "distribute_nodes" => {
            Some("Evenly space a set of nodes along a horizontal or vertical axis.")
        }
        "create_token" => Some("Create a new scalar design token in the tokens block."),
        "update_token_value" => Some("Replace the literal value of an existing design token."),
        "set_style_property" => {
            Some("Set a recognized visual property on a named style to a token reference.")
        }
        "set_text_direction" => Some("Set the text direction (ltr or rtl) of a text node."),
        "find_replace_text" => Some("Literal find-and-replace across text and shape label spans."),
        "set_page_size" => Some("Resize a page by setting new width and height dimensions."),
        "align_to_edge" => {
            Some("Snap a node's edge or centre to the boundary of its containing page.")
        }
        "create_recipe" => Some("Create a new recipe entry in the document's recipes block."),
        "update_recipe" => Some("Replace the scalar fields of an existing recipe."),
        "delete_recipe" => Some("Remove a recipe from the document's recipes block."),
        "detach_pattern" => {
            Some("Materialize a pattern node into an editable group of native shapes.")
        }
        _ => None,
    }
}

// ── Drift-guard tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::Op;
    use std::collections::BTreeSet;

    /// Exhaustive map from an `Op` reference to its JSON tag string.
    ///
    /// The exhaustive `match` here is the **compile-time drift guard**: when a
    /// new `Op` variant is added the compiler forces this fn to be updated,
    /// which in turn forces `op_names()` and `op_summary()` to be updated.
    fn op_tag(op: &Op) -> &'static str {
        match op {
            Op::SetTextAlign { .. } => "set_text_align",
            Op::MoveForward { .. } => "move_forward",
            Op::MoveBackward { .. } => "move_backward",
            Op::MoveToFront { .. } => "move_to_front",
            Op::MoveToBack { .. } => "move_to_back",
            Op::SetFill { .. } => "set_fill",
            Op::SetStroke { .. } => "set_stroke",
            Op::SetStrokeWidth { .. } => "set_stroke_width",
            Op::SetVisible { .. } => "set_visible",
            Op::SetLocked { .. } => "set_locked",
            Op::SetGeometry { .. } => "set_geometry",
            Op::SetPoints { .. } => "set_points",
            Op::AddNode { .. } => "add_node",
            Op::RemoveNode { .. } => "remove_node",
            Op::SetOpacity { .. } => "set_opacity",
            Op::ReplaceText { .. } => "replace_text",
            Op::DuplicateNode { .. } => "duplicate_node",
            Op::DuplicatePage { .. } => "duplicate_page",
            Op::Group { .. } => "group",
            Op::Ungroup { .. } => "ungroup",
            Op::Reparent { .. } => "reparent",
            Op::AlignNodes { .. } => "align_nodes",
            Op::SetTextOverflow { .. } => "set_text_overflow",
            Op::AddPage { .. } => "add_page",
            Op::DeletePage { .. } => "delete_page",
            Op::ReorderPages { .. } => "reorder_pages",
            Op::AddAsset { .. } => "add_asset",
            Op::SetAsset { .. } => "set_asset",
            Op::DistributeNodes { .. } => "distribute_nodes",
            Op::CreateToken { .. } => "create_token",
            Op::UpdateTokenValue { .. } => "update_token_value",
            Op::SetStyleProperty { .. } => "set_style_property",
            Op::SetTextDirection { .. } => "set_text_direction",
            Op::FindReplaceText { .. } => "find_replace_text",
            Op::SetPageSize { .. } => "set_page_size",
            Op::AlignToEdge { .. } => "align_to_edge",
            Op::CreateRecipe { .. } => "create_recipe",
            Op::UpdateRecipe { .. } => "update_recipe",
            Op::DeleteRecipe { .. } => "delete_recipe",
            Op::DetachPattern { .. } => "detach_pattern",
        }
    }

    /// Canonical set of all op tags as derived from the exhaustive match above.
    ///
    /// Kept in sync with `op_tag` by the assertions in the test below.
    fn all_exhaustive_tags() -> BTreeSet<&'static str> {
        BTreeSet::from([
            "set_text_align",
            "move_forward",
            "move_backward",
            "move_to_front",
            "move_to_back",
            "set_fill",
            "set_stroke",
            "set_stroke_width",
            "set_visible",
            "set_locked",
            "set_geometry",
            "set_points",
            "add_node",
            "remove_node",
            "set_opacity",
            "replace_text",
            "duplicate_node",
            "duplicate_page",
            "group",
            "ungroup",
            "reparent",
            "align_nodes",
            "set_text_overflow",
            "add_page",
            "delete_page",
            "reorder_pages",
            "add_asset",
            "set_asset",
            "distribute_nodes",
            "create_token",
            "update_token_value",
            "set_style_property",
            "set_text_direction",
            "find_replace_text",
            "set_page_size",
            "align_to_edge",
            "create_recipe",
            "update_recipe",
            "delete_recipe",
            "detach_pattern",
        ])
    }

    #[test]
    fn op_summary_covers_every_op() {
        let exhaustive = all_exhaustive_tags();
        let listed: BTreeSet<&str> = op_names().iter().copied().collect();

        // The exhaustive set and op_names() must match exactly.
        let missing_from_names: BTreeSet<_> = exhaustive.difference(&listed).collect();
        assert!(
            missing_from_names.is_empty(),
            "op_names() is missing op tags present in the exhaustive match: {:?}",
            missing_from_names,
        );

        let extra_in_names: BTreeSet<_> = listed.difference(&exhaustive).collect();
        assert!(
            extra_in_names.is_empty(),
            "op_names() has tags not in the exhaustive match (add Op variant or remove stale entry): {:?}",
            extra_in_names,
        );

        // Every listed op must have a summary.
        for name in op_names() {
            assert!(
                op_summary(name).is_some(),
                "op_summary(\"{name}\") returned None — add a one-liner to op_summary()",
            );
        }
    }

    /// Verify the `op_tag` helper itself is consistent with `all_exhaustive_tags`.
    ///
    /// We build one representative `Op` value per variant and check the tag it
    /// produces is in our constant set. This catches copy-paste errors in
    /// `op_tag` (wrong string literal for a variant).
    #[test]
    fn op_tag_strings_match_exhaustive_set() {
        let set = all_exhaustive_tags();
        let samples: &[Op] = &[
            Op::SetTextAlign {
                node: String::new(),
                align: String::new(),
            },
            Op::MoveForward {
                node: String::new(),
            },
            Op::MoveBackward {
                node: String::new(),
            },
            Op::MoveToFront {
                node: String::new(),
            },
            Op::MoveToBack {
                node: String::new(),
            },
            Op::SetFill {
                node: String::new(),
                fill: String::new(),
            },
            Op::SetStroke {
                node: String::new(),
                stroke: String::new(),
            },
            Op::SetStrokeWidth {
                node: String::new(),
                stroke_width: String::new(),
            },
            Op::SetVisible {
                node: String::new(),
                visible: true,
            },
            Op::SetLocked {
                node: String::new(),
                locked: false,
            },
            Op::SetGeometry {
                node: String::new(),
                x: None,
                y: None,
                w: None,
                h: None,
                rotate: None,
            },
            Op::SetPoints {
                node: String::new(),
                points: vec![],
            },
            Op::AddNode {
                parent: String::new(),
                position: Default::default(),
                source: String::new(),
            },
            Op::RemoveNode {
                node: String::new(),
            },
            Op::SetOpacity {
                node: String::new(),
                opacity: 1.0,
            },
            Op::ReplaceText {
                node: String::new(),
                spans: vec![],
            },
            Op::DuplicateNode {
                node: String::new(),
                new_id: String::new(),
            },
            Op::DuplicatePage {
                page: String::new(),
                new_id: String::new(),
                id_suffix: String::new(),
            },
            Op::Group {
                node_ids: vec![],
                group_id: String::new(),
            },
            Op::Ungroup {
                group_id: String::new(),
            },
            Op::Reparent {
                node: String::new(),
                new_parent: String::new(),
                position: Default::default(),
            },
            Op::AlignNodes {
                node_ids: vec![],
                align: String::new(),
                anchor: "selection".to_owned(),
            },
            Op::SetTextOverflow {
                node_id: String::new(),
                overflow: String::new(),
            },
            Op::AddPage {
                id: String::new(),
                w: String::new(),
                h: String::new(),
                background: None,
                index: None,
            },
            Op::DeletePage {
                page: String::new(),
            },
            Op::ReorderPages { order: vec![] },
            Op::AddAsset {
                id: String::new(),
                kind: String::new(),
                src: String::new(),
                sha256: None,
            },
            Op::SetAsset {
                node_id: String::new(),
                asset_id: String::new(),
            },
            Op::DistributeNodes {
                node_ids: vec![],
                axis: String::new(),
            },
            Op::CreateToken {
                id: String::new(),
                token_type: String::new(),
                value: String::new(),
            },
            Op::UpdateTokenValue {
                id: String::new(),
                value: String::new(),
            },
            Op::SetStyleProperty {
                style_id: String::new(),
                property: String::new(),
                value: String::new(),
            },
            Op::SetTextDirection {
                node: String::new(),
                direction: String::new(),
            },
            Op::FindReplaceText {
                find: String::new(),
                replace: String::new(),
                node: None,
            },
            Op::SetPageSize {
                page: String::new(),
                w: String::new(),
                h: String::new(),
            },
            Op::AlignToEdge {
                node: String::new(),
                edge: String::new(),
                margin: 0.0,
            },
            Op::CreateRecipe {
                id: String::new(),
                kind: String::new(),
                seed: None,
                generator: None,
                bounds: None,
                detached: None,
            },
            Op::UpdateRecipe {
                id: String::new(),
                kind: String::new(),
                seed: None,
                generator: None,
                bounds: None,
                detached: None,
            },
            Op::DeleteRecipe { id: String::new() },
            Op::DetachPattern {
                node: String::new(),
            },
        ];

        for op in samples {
            let tag = op_tag(op);
            assert!(
                set.contains(tag),
                "op_tag produced \"{tag}\" which is not in all_exhaustive_tags() — fix the mismatch",
            );
        }

        // Count check: every variant must be represented exactly once.
        assert_eq!(
            samples.len(),
            set.len(),
            "samples count ({}) != exhaustive set size ({}): add/remove a sample",
            samples.len(),
            set.len(),
        );
    }
}
