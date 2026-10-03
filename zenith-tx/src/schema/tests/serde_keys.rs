//! Serde field-name drift guards: every serialized `Op` key must be documented
//! in `op_fields()`, and optional path-anchor fields must be omitted when absent.

use super::common::add_asset_sample_op;
use crate::op::{
    Op, OpPathAnchor, OpPathBooleanOperation, OpPathHandle, OpPathSubpath, OpPathTransform,
};
use crate::schema::*;

/// Every key in a serialized representative `Op` value (other than `"op"`)
/// must appear in the `op_fields()` list for that op.
///
/// This is the **serde field-name drift guard**: if a field is renamed or
/// added in `Op` but `op_fields()` is not updated, the serialized key will
/// be absent from the documented list and this test will fail.
#[test]
fn op_fields_names_match_serde_keys() {
    use crate::op::{Op, OpPoint, OpSpan, Position};

    // Build one representative `Op` per variant that has non-optional
    // fields set to real values so serde emits all keys (including
    // skip_serializing_if=None fields that ARE present here as Some).
    // We deliberately make every Option<T> a Some(_) so the serialized
    // output contains every possible key.
    let samples: &[(&str, Op)] = &[
        (
            "set_text_align",
            Op::SetTextAlign {
                node: "n".into(),
                align: "center".into(),
            },
        ),
        ("move_forward", Op::MoveForward { node: "n".into() }),
        ("move_backward", Op::MoveBackward { node: "n".into() }),
        ("move_to_front", Op::MoveToFront { node: "n".into() }),
        ("move_to_back", Op::MoveToBack { node: "n".into() }),
        (
            "set_fill",
            Op::SetFill {
                node: "n".into(),
                fill: "color.brand".into(),
            },
        ),
        (
            "set_fill_rule",
            Op::SetFillRule {
                node: "n".into(),
                fill_rule: "evenodd".into(),
            },
        ),
        (
            "set_stroke",
            Op::SetStroke {
                node: "n".into(),
                stroke: "color.rule".into(),
            },
        ),
        (
            "set_stroke_width",
            Op::SetStrokeWidth {
                node: "n".into(),
                stroke_width: "size.stroke".into(),
            },
        ),
        (
            "set_visible",
            Op::SetVisible {
                node: "n".into(),
                visible: true,
            },
        ),
        (
            "set_locked",
            Op::SetLocked {
                node: "n".into(),
                locked: false,
            },
        ),
        (
            "set_geometry",
            Op::SetGeometry {
                node: "n".into(),
                x: Some(Some(0.0)),
                y: Some(Some(0.0)),
                w: Some(Some(100.0.into())),
                h: Some(Some("hug".into())),
                rotate: Some(Some(0.0)),
            },
        ),
        (
            "set_points",
            Op::SetPoints {
                node: "n".into(),
                points: vec![OpPoint { x: 0.0, y: 0.0 }],
            },
        ),
        (
            "set_path_anchors",
            Op::SetPathAnchors {
                node: "n".into(),
                subpath_index: None,
                anchors: vec![OpPathAnchor {
                    x: 0.0,
                    y: 0.0,
                    kind: Some("smooth".into()),
                    in_x: Some(-10.0),
                    in_y: Some(0.0),
                    out_x: Some(10.0),
                    out_y: Some(0.0),
                }],
            },
        ),
        (
            "set_path_anchor_kind",
            Op::SetPathAnchorKind {
                node: "n".into(),
                subpath_index: None,
                anchor_index: 1,
                kind: Some("smooth".into()),
            },
        ),
        (
            "remove_path_anchor",
            Op::RemovePathAnchor {
                node: "n".into(),
                subpath_index: None,
                anchor_index: 1,
            },
        ),
        (
            "insert_path_anchor",
            Op::InsertPathAnchor {
                node: "n".into(),
                subpath_index: None,
                segment_index: 0,
                t: 0.5,
            },
        ),
        (
            "insert_path_anchor_at_point",
            Op::InsertPathAnchorAtPoint {
                node: "n".into(),
                x: 50.0,
                y: 2.0,
                tolerance: 4.0,
            },
        ),
        (
            "move_path_anchor",
            Op::MovePathAnchor {
                node: "n".into(),
                subpath_index: None,
                anchor_index: 0,
                dx: 10.0,
                dy: -4.0,
            },
        ),
        (
            "move_path_handle",
            Op::MovePathHandle {
                node: "n".into(),
                subpath_index: None,
                anchor_index: 0,
                handle: OpPathHandle::Out,
                dx: 10.0,
                dy: -4.0,
            },
        ),
        (
            "simplify_path_anchors",
            Op::SimplifyPathAnchors {
                node: "n".into(),
                subpath_index: None,
                tolerance: 0.5,
            },
        ),
        (
            "transform_path_anchors",
            Op::TransformPathAnchors {
                node: "n".into(),
                transform: OpPathTransform::Rotate {
                    angle_degrees: 90.0,
                    cx: 10.0,
                    cy: 20.0,
                },
            },
        ),
        (
            "snap_path_anchors",
            Op::SnapPathAnchors {
                node: "n".into(),
                target: "target".into(),
                tolerance: 4.0,
            },
        ),
        (
            "make_path_symmetric",
            Op::MakePathSymmetric {
                node: "n".into(),
                id_prefix: "n.sym.".into(),
                count: 4,
                cx: 10.0,
                cy: 20.0,
                start_angle_degrees: 0.0,
                mirror: false,
            },
        ),
        (
            "path_boolean",
            Op::PathBoolean {
                node: "a".into(),
                target: "b".into(),
                new_id: "out".into(),
                operation: OpPathBooleanOperation::Union,
                tolerance: 0.5,
            },
        ),
        (
            "add_node",
            Op::AddNode {
                parent: "p".into(),
                position: Position::Last,
                source: "rect id=\"x\"".into(),
            },
        ),
        (
            "add_path",
            Op::AddPath {
                parent: "p".into(),
                id: "path.new".into(),
                position: Position::Last,
                closed: Some(true),
                anchors: vec![OpPathAnchor {
                    x: 0.0,
                    y: 0.0,
                    kind: Some("corner".into()),
                    in_x: Some(-10.0),
                    in_y: Some(0.0),
                    out_x: Some(10.0),
                    out_y: Some(0.0),
                }],
                subpaths: vec![OpPathSubpath {
                    closed: Some(false),
                    anchors: vec![OpPathAnchor {
                        x: 1.0,
                        y: 2.0,
                        kind: None,
                        in_x: None,
                        in_y: None,
                        out_x: None,
                        out_y: None,
                    }],
                }],
            },
        ),
        ("remove_node", Op::RemoveNode { node: "n".into() }),
        (
            "set_opacity",
            Op::SetOpacity {
                node: "n".into(),
                opacity: 1.0,
            },
        ),
        (
            "replace_text",
            Op::ReplaceText {
                node: "n".into(),
                spans: vec![OpSpan {
                    text: "hi".into(),
                    fill: Some("color.brand".into()),
                    font_weight: Some("font.bold".into()),
                    italic: Some(true),
                    underline: Some(false),
                    strikethrough: Some(false),
                    vertical_align: Some("super".into()),
                    footnote_ref: Some("fn1".into()),
                }],
            },
        ),
        (
            "duplicate_node",
            Op::DuplicateNode {
                node: "n".into(),
                new_id: "n2".into(),
            },
        ),
        (
            "duplicate_page",
            Op::DuplicatePage {
                page: "p".into(),
                new_id: "p2".into(),
                id_suffix: ".v2".into(),
            },
        ),
        (
            "group",
            Op::Group {
                node_ids: vec!["a".into()],
                group_id: "g".into(),
            },
        ),
        (
            "ungroup",
            Op::Ungroup {
                group_id: "g".into(),
            },
        ),
        (
            "reparent",
            Op::Reparent {
                node: "n".into(),
                new_parent: "p".into(),
                position: Position::Last,
            },
        ),
        (
            "align_nodes",
            Op::AlignNodes {
                node_ids: vec!["a".into()],
                align: "left".into(),
                anchor: "selection".into(),
            },
        ),
        (
            "set_text_overflow",
            Op::SetTextOverflow {
                node_id: "n".into(),
                overflow: "clip".into(),
            },
        ),
        (
            "add_page",
            Op::AddPage {
                id: "p".into(),
                w: "(px)1800".into(),
                h: "(px)1200".into(),
                background: Some("color.bg".into()),
                index: Some(0),
            },
        ),
        ("delete_page", Op::DeletePage { page: "p".into() }),
        (
            "reorder_pages",
            Op::ReorderPages {
                order: vec!["a".into()],
            },
        ),
        ("add_asset", add_asset_sample_op()),
        (
            "set_asset",
            Op::SetAsset {
                node_id: "pic".into(),
                asset_id: "asset.hero".into(),
            },
        ),
        (
            "distribute_nodes",
            Op::DistributeNodes {
                node_ids: vec!["a".into()],
                axis: "horizontal".into(),
            },
        ),
        (
            "create_token",
            Op::CreateToken {
                id: "color.brand".into(),
                token_type: "color".into(),
                value: "#e11d48".into(),
                set: Some("@zenith/theme.cobalt".into()),
                layers: vec![],
                filter_ops: vec![],
                stops: vec![],
                angle: None,
                radial: None,
                center_x: None,
                center_y: None,
                radius: None,
                shape: None,
                feather: None,
                invert: None,
            },
        ),
        (
            "update_token_value",
            Op::UpdateTokenValue {
                id: "color.brand".into(),
                value: "#3b82f6".into(),
                set: Some(Some("@zenith/theme.cobalt".into())),
            },
        ),
        (
            "set_style_property",
            Op::SetStyleProperty {
                style_id: "heading".into(),
                property: "font-family".into(),
                value: "font.body".into(),
            },
        ),
        (
            "create_style",
            Op::CreateStyle {
                id: "cta.label".into(),
                properties: std::collections::BTreeMap::from([(
                    "fill".into(),
                    "color.primary.content".into(),
                )]),
            },
        ),
        (
            "delete_style",
            Op::DeleteStyle {
                id: "cta.label".into(),
            },
        ),
        (
            "create_master",
            Op::CreateMaster {
                id: "m.deck".into(),
            },
        ),
        (
            "delete_master",
            Op::DeleteMaster {
                id: "m.deck".into(),
            },
        ),
        (
            "set_page_master",
            Op::SetPageMaster {
                page: "page.1".into(),
                master: Some("m.deck".into()),
            },
        ),
        (
            "set_text_direction",
            Op::SetTextDirection {
                node: "n".into(),
                direction: "ltr".into(),
            },
        ),
        (
            "find_replace_text",
            Op::FindReplaceText {
                find: "Draft".into(),
                replace: "Final".into(),
                node: Some("label".into()),
            },
        ),
        (
            "set_page_size",
            Op::SetPageSize {
                page: "p".into(),
                w: "(px)794".into(),
                h: "(px)1123".into(),
            },
        ),
        (
            "align_to_edge",
            Op::AlignToEdge {
                node: "n".into(),
                edge: "right".into(),
                margin: 0.0,
            },
        ),
        (
            "create_recipe",
            Op::CreateRecipe {
                id: "recipe.scatter".into(),
                kind: "scatter".into(),
                seed: Some(42),
                generator: Some("scatter@1".into()),
                bounds: Some("frame1".into()),
                detached: Some(false),
            },
        ),
        (
            "update_recipe",
            Op::UpdateRecipe {
                id: "recipe.scatter".into(),
                kind: "scatter".into(),
                seed: Some(42),
                generator: Some("scatter@1".into()),
                bounds: Some("frame1".into()),
                detached: Some(true),
            },
        ),
        ("delete_recipe", Op::DeleteRecipe { id: "r".into() }),
        (
            "set_default",
            Op::SetDefault {
                page: Some("p".into()),
                kind: "shape".into(),
                style: "box".into(),
                text_style: Some(Some("box.label".into())),
            },
        ),
        (
            "remove_default",
            Op::RemoveDefault {
                page: Some("p".into()),
                kind: "shape".into(),
            },
        ),
        (
            "detach_pattern",
            Op::DetachPattern {
                node: "dots".into(),
            },
        ),
        (
            "set_layout",
            Op::SetLayout(crate::op::LayoutEdit {
                node: "n".into(),
                layout: Some(Some("row".into())),
                gap: Some(Some(8.0.into())),
                wrap_gap: Some(None),
                padding: Some(Some("space.md".into())),
                padding_x: Some(None),
                padding_y: Some(None),
                padding_top: Some(None),
                padding_right: Some(None),
                padding_bottom: Some(None),
                padding_left: Some(None),
                justify: Some(Some("center".into())),
                align: Some(None),
                wrap: Some(Some(true)),
                clip: Some(None),
                position: Some(None),
                min_w: Some(None),
                max_w: Some(None),
                min_h: Some(None),
                max_h: Some(None),
            }),
        ),
    ];

    for (name, op) in samples {
        // Serialize the Op to JSON.
        let json_str = serde_json::to_string(op)
            .unwrap_or_else(|e| panic!("failed to serialize Op sample for \"{name}\": {e}"));
        let v: serde_json::Value = serde_json::from_str(&json_str)
            .unwrap_or_else(|e| panic!("failed to re-parse serialized Op for \"{name}\": {e}"));
        let obj = v
            .as_object()
            .unwrap_or_else(|| panic!("serialized Op for \"{name}\" is not a JSON object"));

        // Collect the documented field names for this op.
        let fields = op_fields(name)
            .unwrap_or_else(|| panic!("op_fields(\"{name}\") returned None — update op_fields()"));
        let documented: std::collections::BTreeSet<&str> = fields.iter().map(|f| f.name).collect();

        // Every serialized key (except "op") must be in the documented set.
        for key in obj.keys() {
            if key == "op" {
                continue;
            }
            assert!(
                documented.contains(key.as_str()),
                "op \"{name}\": serialized key \"{key}\" is not in op_fields() — \
                 update op_fields() to document this field",
            );
        }
    }

    // Count check: every variant in op_names() must appear in samples.
    let sample_names: std::collections::BTreeSet<&str> =
        samples.iter().map(|(name, _)| *name).collect();
    let all_names: std::collections::BTreeSet<&str> = op_names().iter().copied().collect();
    let missing: std::collections::BTreeSet<_> = all_names.difference(&sample_names).collect();
    assert!(
        missing.is_empty(),
        "op_fields_names_match_serde_keys is missing samples for ops: {:?}",
        missing,
    );
}

#[test]
fn path_anchor_optional_fields_are_omitted_when_absent() {
    let json = serde_json::to_value(Op::SetPathAnchors {
        node: "n".into(),
        subpath_index: None,
        anchors: vec![OpPathAnchor {
            x: 1.0,
            y: 2.0,
            kind: None,
            in_x: None,
            in_y: None,
            out_x: None,
            out_y: None,
        }],
    })
    .expect("serialization should succeed");

    let anchors = json
        .get("anchors")
        .and_then(|value| value.as_array())
        .expect("anchors should serialize as an array");
    let anchor = anchors
        .first()
        .and_then(|value| value.as_object())
        .expect("anchor should serialize as an object");

    assert_eq!(anchor.len(), 2);
    assert!(anchor.contains_key("x"));
    assert!(anchor.contains_key("y"));
    assert!(!anchor.contains_key("in_x"));
    assert!(!anchor.contains_key("in_y"));
    assert!(!anchor.contains_key("out_x"));
    assert!(!anchor.contains_key("out_y"));
}
