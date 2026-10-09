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
        "set_fill_rule" => Some("Set the authored fill-rule of a polygon, polyline, or path."),
        "set_stroke" => Some("Set the stroke (outline) color of a node to a token reference."),
        "set_stroke_width" => {
            Some("Set the stroke width of a node to a dimension token reference.")
        }
        "set_node_token" => Some(
            "Bind radius, font-family, font-size, or font-weight of a node to a token; null \
             removes the attribute so the style or default applies.",
        ),
        "set_span_text" => Some(
            "Replace the text of one span (0-based) of a text or shape node, keeping the span's \
             own attributes.",
        ),
        "set_visible" => Some("Show or hide a node by toggling its visible property."),
        "set_locked" => Some("Lock or unlock a node to prevent accidental edits."),
        "set_geometry" => Some(
            "Move and/or resize a node by setting x, y, w, h (px, or \"hug\"/\"fill\"), or rotate. \
             Omit a field to keep it; null removes the attribute. x/y values on an in-flow child of \
             a row/column/grid frame are rejected (tx.layout_managed); null x/y there is allowed. \
             Removing x/y/w/h that a node needs outside flow is rejected (tx.geometry_required). \
             Writes px: it replaces a token ref, a (pt) value, or a hug/fill keyword on that axis. \
             Use nudge_geometry to move by a delta and keep tokens and units.",
        ),
        "nudge_geometry" => Some(
            "Move/resize a box node by px deltas dx, dy, dw, dh in its authored (unrotated, \
             parent) space, keeping each attribute's unit. Rejects a token-bound axis \
             (tx.token_bound; detach=true writes px), a hug/fill or absent size \
             (tx.computed_size), an anchor-supplied x/y (tx.anchored), a pct/literal value \
             (tx.value_unresolved), a negative size (tx.invalid_geometry), and dx/dy on an \
             in-flow child (tx.layout_managed). Absent x/y on a group or instance counts as 0.",
        ),
        "set_anchor" => Some(
            "Set or clear a node's anchor attributes (anchor, anchor_zone, anchor_sibling, \
             anchor_parent, anchor_edge, anchor_gap). Omit keeps, null removes. Removing the \
             anchor of a node without x/y is rejected (tx.geometry_required): use detach_anchor.",
        ),
        "nudge_anchor_gap" => Some(
            "Move an edge-anchored node (anchor_sibling + anchor_edge) along its edge axis by \
             changing anchor-gap in its unit. dx/dy is the drag delta. Below adds dy, above \
             subtracts dy, after adds dx, before subtracts dx. Cross-axis movement is rejected.",
        ),
        "detach_anchor" => Some(
            "Remove every anchor attribute of a node and write each anchor-supplied x/y as px \
             at its derived position, so the node and nodes anchored to it keep their place.",
        ),
        "nudge_line_points" => Some(
            "Move a line's endpoints by px deltas (dx1, dy1 start; dx2, dy2 end), keeping units. \
             A connector is rejected (tx.derived_geometry): move its targets instead.",
        ),
        "set_points" => Some("Replace the full vertex list of a polygon or polyline node."),
        "set_path_anchors" => Some("Replace the full anchor list of a path node."),
        "set_path_anchor_kind" => Some("Set or clear one path anchor's authoring intent metadata."),
        "remove_path_anchor" => Some("Remove one path anchor by index."),
        "insert_path_anchor" => Some("Insert a path anchor by splitting an existing segment."),
        "insert_path_anchor_at_point" => {
            Some("Insert a path anchor at the nearest path projection within a tolerance.")
        }
        "move_path_anchor" => Some("Move one path anchor and its handles by a pixel delta."),
        "move_path_handle" => {
            Some("Move one path handle by a pixel delta, preserving anchor intent.")
        }
        "simplify_path_anchors" => {
            Some("Simplify an open path node's anchors using a pixel tolerance.")
        }
        "transform_path_anchors" => Some(
            "Translate, rotate, reflect, or scale a path node's anchor and handle points. \
             Scale (sx, sy about cx, cy) resizes the path. A zero factor is rejected.",
        ),
        "snap_path_anchors" => {
            Some("Translate a path so its nearest boundary point lands on another path.")
        }
        "make_path_symmetric" => Some(
            "Materialize radial (or, with mirror=true, dihedral mirror) symmetry copies of a path as editable sibling paths.",
        ),
        "path_boolean" => {
            Some("Materialize a simple closed-contour boolean result as a sibling path.")
        }
        "add_node" => Some("Parse a .zen source fragment and insert it into a container."),
        "add_path" => Some("Create a typed path node and insert it into a container."),
        "remove_node" => Some("Remove a node and its subtree from the document."),
        "set_opacity" => Some("Set the opacity of a node (0.0 = fully transparent, 1.0 = opaque)."),
        "replace_text" => Some("Replace all text spans of a text or shape node."),
        "duplicate_node" => Some(
            "Clone a node with its whole subtree (descendants get a suffix derived from new_id) and insert the copy after the original.",
        ),
        "duplicate_page" => Some("Deep-clone a page and insert the copy after the original."),
        "group" => Some(
            "Wrap a set of sibling nodes inside a new group node; members keep document (paint) \
             order whatever order node_ids lists; in a layout flow frame the group \
             takes a flow slot (tx.flow_placed), and a changed page box warns (tx.page_box_changed).",
        ),
        "ungroup" => Some(
            "Dissolve a group node, moving its children up to the parent; children keep their page \
             position, except in a layout flow slot (tx.flow_placed); a changed page box warns \
             (tx.page_box_changed).",
        ),
        "reparent" => Some(
            "Move a node into a different container (page, group, or frame); x/y convert so the \
             node keeps its page position, except in a layout flow slot (tx.flow_placed); a \
             changed page box warns (tx.page_box_changed).",
        ),
        "align_nodes" => Some(
            "Align a set of nodes to a common edge or centre along one axis; nodes in different \
             containers align by page position.",
        ),
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
        "create_token" => Some(
            "Create a design token (scalar via value; shadow/filter/gradient/mask via structured fields).",
        ),
        "update_token_value" => Some("Replace the literal value of an existing design token."),
        "set_style_property" => Some(
            "Set a recognized visual property on a named style to a token reference \
                 (an enum value for align / v-align).",
        ),
        "create_style" => Some("Create a named style in the document styles block."),
        "delete_style" => Some("Remove a named style from the document styles block."),
        "create_master" => {
            Some("Create an empty master-page definition (shared chrome); populate via add_node.")
        }
        "delete_master" => Some("Remove a master-page definition from the masters block."),
        "set_page_master" => {
            Some("Assign or clear a page's master attribute (shared chrome projection).")
        }
        "set_text_direction" => Some("Set the text direction (ltr or rtl) of a text node."),
        "find_replace_text" => Some("Literal find-and-replace across text and shape label spans."),
        "set_layout" => Some(
            "Set or clear auto-layout attributes: frame container fields (layout, gap, padding*, \
             justify, align, wrap, clip) and box-node item fields (position, min/max w/h); null clears.",
        ),
        "set_page_size" => Some("Resize a page by setting new width and height dimensions."),
        "align_to_edge" => {
            Some("Snap a node's edge or centre to the boundary of its containing page.")
        }
        "create_recipe" => Some("Create a new recipe entry in the document's recipes block."),
        "update_recipe" => Some("Replace the scalar fields of an existing recipe."),
        "delete_recipe" => Some("Remove a recipe from the document's recipes block."),
        "set_default" => Some(
            "Upsert a defaults entry (node kind to default style) in the document or a page defaults block.",
        ),
        "remove_default" => {
            Some("Remove a defaults entry by node kind from the document or a page defaults block.")
        }
        "detach_pattern" => {
            Some("Materialize a pattern node into an editable group of native shapes.")
        }
        _ => None,
    }
}
