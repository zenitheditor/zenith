//! Coverage drift guards: op_names/op_summary/op_fields/op_example must each
//! cover every `Op` variant, and the op→tag map must stay consistent.

use super::common::{all_exhaustive_tags, op_tag};
use crate::op::{AddAssetMetadata, Op, OpPathBooleanOperation, OpPathHandle, OpPathTransform};
use crate::schema::*;
use std::collections::BTreeSet;

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
        Op::SetFillRule {
            node: String::new(),
            fill_rule: String::new(),
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
        Op::SetPathAnchors {
            node: String::new(),
            subpath_index: None,
            anchors: vec![],
        },
        Op::SetPathAnchorKind {
            node: String::new(),
            subpath_index: None,
            anchor_index: 0,
            kind: Some("smooth".into()),
        },
        Op::RemovePathAnchor {
            node: String::new(),
            subpath_index: None,
            anchor_index: 0,
        },
        Op::InsertPathAnchor {
            node: String::new(),
            subpath_index: None,
            segment_index: 0,
            t: 0.5,
        },
        Op::InsertPathAnchorAtPoint {
            node: String::new(),
            x: 50.0,
            y: 2.0,
            tolerance: 4.0,
        },
        Op::MovePathAnchor {
            node: String::new(),
            subpath_index: None,
            anchor_index: 0,
            dx: 0.0,
            dy: 0.0,
        },
        Op::MovePathHandle {
            node: String::new(),
            subpath_index: None,
            anchor_index: 0,
            handle: OpPathHandle::Out,
            dx: 0.0,
            dy: 0.0,
        },
        Op::SimplifyPathAnchors {
            node: String::new(),
            subpath_index: None,
            tolerance: 1.0,
        },
        Op::TransformPathAnchors {
            node: String::new(),
            transform: OpPathTransform::Translate { dx: 0.0, dy: 0.0 },
        },
        Op::SnapPathAnchors {
            node: String::new(),
            target: String::new(),
            tolerance: 1.0,
        },
        Op::MakePathSymmetric {
            node: String::new(),
            id_prefix: "copy.".into(),
            count: 4,
            cx: 0.0,
            cy: 0.0,
            start_angle_degrees: 0.0,
            mirror: false,
        },
        Op::PathBoolean {
            node: String::new(),
            target: String::new(),
            new_id: String::new(),
            operation: OpPathBooleanOperation::Union,
            tolerance: 1.0,
        },
        Op::AddNode {
            parent: String::new(),
            position: Default::default(),
            source: String::new(),
        },
        Op::AddPath {
            parent: String::new(),
            id: String::new(),
            position: Default::default(),
            closed: None,
            anchors: vec![],
            subpaths: vec![],
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
            metadata: Box::new(AddAssetMetadata::default()),
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
            set: None,
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
        Op::UpdateTokenValue {
            id: String::new(),
            value: String::new(),
            set: None,
        },
        Op::SetStyleProperty {
            style_id: String::new(),
            property: String::new(),
            value: String::new(),
        },
        Op::CreateStyle {
            id: String::new(),
            properties: std::collections::BTreeMap::new(),
        },
        Op::DeleteStyle { id: String::new() },
        Op::CreateMaster { id: String::new() },
        Op::DeleteMaster { id: String::new() },
        Op::SetPageMaster {
            page: String::new(),
            master: None,
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
        Op::SetLayout(crate::op::LayoutEdit::default()),
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

/// Every op must have a non-`None` `op_fields` result.
///
/// This is a **drift guard**: a new op variant added to `op_names()` must
/// also appear in `op_fields()` or this test fails at compile+run time.
#[test]
fn op_fields_covers_every_op() {
    for &name in op_names() {
        assert!(
            op_fields(name).is_some(),
            "op_fields(\"{name}\") returned None — add an arm to op_fields()",
        );
    }
}

#[test]
fn add_asset_schema_lists_optional_provenance_fields() {
    let fields = op_fields("add_asset").expect("add_asset fields should be documented");
    let optional_names: BTreeSet<&str> = fields
        .iter()
        .filter(|field| !field.required)
        .map(|field| field.name)
        .collect();

    assert_eq!(
        optional_names,
        BTreeSet::from([
            "sha256",
            "producer_kind",
            "producer_source",
            "ai_prompt",
            "ai_model",
            "ai_provider",
            "ai_seed",
            "ai_generation_date",
            "ai_license",
            "ai_source_rights",
            "ai_safety_status",
            "ai_reuse_policy",
        ])
    );
}

/// Every op must have a non-`None` `op_example` result, and the returned
/// string must parse as valid JSON whose `"op"` field matches the op name.
///
/// This is a **drift guard**: a new op that lacks an example fails here.
#[test]
fn op_example_covers_every_op() {
    for &name in op_names() {
        let example = op_example(name).unwrap_or_else(|| {
            panic!("op_example(\"{name}\") returned None — add an arm to op_example()")
        });
        // Must parse as a JSON object.
        let v: serde_json::Value = serde_json::from_str(example).unwrap_or_else(|e| {
            panic!("op_example(\"{name}\") is not valid JSON: {e}\n  value: {example}")
        });
        // The "op" field must match the op name.
        let op_field = v
            .get("op")
            .and_then(|f| f.as_str())
            .unwrap_or_else(|| panic!("op_example(\"{name}\") has no string \"op\" field"));
        assert_eq!(
            op_field, name,
            "op_example(\"{name}\") has wrong \"op\" tag: got \"{op_field}\"",
        );
    }
}
