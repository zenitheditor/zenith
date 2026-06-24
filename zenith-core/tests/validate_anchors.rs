//! Integration tests: anchor diagnostics.
//!
//! Covers anchor-sibling-without-anchor, sibling-anchor graph validation
//! (unresolved sibling, two-node cycle), anchor-edge + anchor-sibling
//! interaction, anchor-edge-without-sibling, anchor-edge unknown value,
//! and anchor-gap invalid unit.

use std::collections::BTreeMap;

mod common;

use common::*;

// ── anchor-sibling without anchor → anchor.sibling_without_anchor ─────

#[test]
fn anchor_sibling_without_anchor_produces_sibling_without_anchor() {
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.one",
            vec![
                // A real sibling so anchor-sibling resolves; isolates the
                // without-anchor warning from anchor.unresolved_sibling.
                minimal_rect("sib.target", None),
                Node::Rect(Box::new(RectNode {
                    shadow: None,
                    filter: None,
                    mask: None,
                    id: "rect.sib".to_owned(),
                    name: None,
                    role: None,
                    x: Some(px(0.0)),
                    y: Some(px(0.0)),
                    w: Some(px(50.0)),
                    h: Some(px(50.0)),
                    radius: None,
                    radius_tl: None,
                    radius_tr: None,
                    radius_br: None,
                    radius_bl: None,
                    style: None,
                    fill: None,
                    stroke: None,
                    stroke_width: None,
                    stroke_alignment: None,
                    stroke_dash: None,
                    stroke_gap: None,
                    stroke_linecap: None,
                    border_top: None,
                    border_bottom: None,
                    border_left: None,
                    border_right: None,
                    border_width: None,
                    stroke_outer: None,
                    stroke_outer_width: None,
                    opacity: None,
                    visible: None,
                    locked: None,
                    rotate: None,
                    blend_mode: None,
                    blur: None,
                    anchor: None, // absent — triggers the warning
                    anchor_zone: None,
                    anchor_sibling: Some("sib.target".to_owned()),
                    anchor_edge: None,
                    anchor_gap: None,
                    anchor_parent: None,
                    source_span: None,
                    unknown_props: BTreeMap::new(),
                })),
            ],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "anchor.sibling_without_anchor"),
        "expected anchor.sibling_without_anchor diagnostic; codes: {:?}",
        codes(&report)
    );
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "anchor.sibling_without_anchor")
        .expect("diagnostic must exist");
    assert_eq!(diag.severity, Severity::Warning);
    assert!(!report.has_errors());
}

// ── sibling-anchor graph validation ──────────────────────────────

/// Build a `minimal_rect` carrying a recognized `anchor` and an
/// `anchor_sibling` target, for the sibling-graph validator tests.
fn anchored_sibling_rect(id: &str, target: &str) -> Node {
    let mut node = minimal_rect(id, None);
    if let Node::Rect(ref mut r) = node {
        r.anchor = Some("top-left".to_owned());
        r.anchor_sibling = Some(target.to_owned());
    }
    node
}

// ── anchor-sibling naming a non-existent id → anchor.unresolved_sibling ─

#[test]
fn anchor_sibling_unresolved_target_errors() {
    // The rect references sibling "ghost" which is not present in the scope.
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.one",
            vec![anchored_sibling_rect("rect.ref", "ghost")],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "anchor.unresolved_sibling"),
        "expected anchor.unresolved_sibling diagnostic; codes: {:?}",
        codes(&report)
    );
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "anchor.unresolved_sibling")
        .expect("diagnostic must exist");
    assert_eq!(diag.severity, Severity::Error);
    assert!(report.has_errors());
}

// ── two-node cycle A↔B → anchor.cycle ─────────────────────────────────

#[test]
fn anchor_sibling_two_node_cycle_errors() {
    // a anchors to b, b anchors to a: a mutual cycle.
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.one",
            vec![
                anchored_sibling_rect("a", "b"),
                anchored_sibling_rect("b", "a"),
            ],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "anchor.cycle"),
        "expected anchor.cycle diagnostic; codes: {:?}",
        codes(&report)
    );
    // Both cyclic nodes are reported exactly once each.
    let cycle_count = report
        .diagnostics
        .iter()
        .filter(|d| d.code == "anchor.cycle")
        .count();
    assert_eq!(
        cycle_count, 2,
        "expected exactly two anchor.cycle diagnostics"
    );
    assert!(report.has_errors());
}

// ── anchor-edge + anchor-sibling (no anchor, no x/y) ─────────────────

/// anchor-sibling + anchor-edge, no 9-pt anchor, no x/y: must be valid
/// (no `node.missing_geometry`, no `anchor.sibling_without_anchor`).
#[test]
fn anchor_edge_with_sibling_no_anchor_is_valid() {
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.one",
            vec![
                minimal_rect("sib.target", None),
                Node::Rect(Box::new(RectNode {
                    shadow: None,
                    filter: None,
                    mask: None,
                    id: "rect.edge".to_owned(),
                    name: None,
                    role: None,
                    x: None, // absent — valid because anchor-edge supplies position
                    y: None, // absent
                    w: Some(px(50.0)),
                    h: Some(px(50.0)),
                    radius: None,
                    radius_tl: None,
                    radius_tr: None,
                    radius_br: None,
                    radius_bl: None,
                    style: None,
                    fill: None,
                    stroke: None,
                    stroke_width: None,
                    stroke_alignment: None,
                    stroke_dash: None,
                    stroke_gap: None,
                    stroke_linecap: None,
                    border_top: None,
                    border_bottom: None,
                    border_left: None,
                    border_right: None,
                    border_width: None,
                    stroke_outer: None,
                    stroke_outer_width: None,
                    opacity: None,
                    visible: None,
                    locked: None,
                    rotate: None,
                    blend_mode: None,
                    blur: None,
                    anchor: None,
                    anchor_zone: None,
                    anchor_sibling: Some("sib.target".to_owned()),
                    anchor_edge: Some("below".to_owned()),
                    anchor_gap: None,
                    anchor_parent: None,
                    source_span: None,
                    unknown_props: BTreeMap::new(),
                })),
            ],
        )],
    );
    let report = validate(&doc);
    assert!(
        !has_code(&report, "node.missing_geometry"),
        "anchor-edge + anchor-sibling must suppress missing_geometry; codes: {:?}",
        codes(&report)
    );
    assert!(
        !has_code(&report, "anchor.sibling_without_anchor"),
        "anchor-edge + anchor-sibling must NOT warn sibling_without_anchor; codes: {:?}",
        codes(&report)
    );
    assert!(
        !report.has_errors(),
        "expected no errors; codes: {:?}",
        codes(&report)
    );
}

// ── anchor-edge without anchor-sibling → anchor.edge_without_sibling ─

#[test]
fn anchor_edge_without_sibling_produces_edge_without_sibling() {
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.one",
            vec![Node::Rect(Box::new(RectNode {
                shadow: None,
                filter: None,
                mask: None,
                id: "rect.edge-only".to_owned(),
                name: None,
                role: None,
                x: Some(px(0.0)),
                y: Some(px(0.0)),
                w: Some(px(50.0)),
                h: Some(px(50.0)),
                radius: None,
                radius_tl: None,
                radius_tr: None,
                radius_br: None,
                radius_bl: None,
                style: None,
                fill: None,
                stroke: None,
                stroke_width: None,
                stroke_alignment: None,
                stroke_dash: None,
                stroke_gap: None,
                stroke_linecap: None,
                border_top: None,
                border_bottom: None,
                border_left: None,
                border_right: None,
                border_width: None,
                stroke_outer: None,
                stroke_outer_width: None,
                opacity: None,
                visible: None,
                locked: None,
                rotate: None,
                blend_mode: None,
                blur: None,
                anchor: None,
                anchor_zone: None,
                anchor_sibling: None, // absent
                anchor_edge: Some("above".to_owned()),
                anchor_gap: None,
                anchor_parent: None,
                source_span: None,
                unknown_props: BTreeMap::new(),
            }))],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "anchor.edge_without_sibling"),
        "expected anchor.edge_without_sibling; codes: {:?}",
        codes(&report)
    );
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "anchor.edge_without_sibling")
        .expect("diagnostic must exist");
    assert_eq!(diag.severity, Severity::Warning);
    assert!(!report.has_errors());
}

// ── anchor-edge with unknown value → anchor.unknown_edge ──────────────

#[test]
fn anchor_edge_unknown_value_produces_unknown_edge() {
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.one",
            vec![
                minimal_rect("sib.target", None),
                Node::Rect(Box::new(RectNode {
                    shadow: None,
                    filter: None,
                    mask: None,
                    id: "rect.bad-edge".to_owned(),
                    name: None,
                    role: None,
                    x: Some(px(0.0)),
                    y: Some(px(0.0)),
                    w: Some(px(50.0)),
                    h: Some(px(50.0)),
                    radius: None,
                    radius_tl: None,
                    radius_tr: None,
                    radius_br: None,
                    radius_bl: None,
                    style: None,
                    fill: None,
                    stroke: None,
                    stroke_width: None,
                    stroke_alignment: None,
                    stroke_dash: None,
                    stroke_gap: None,
                    stroke_linecap: None,
                    border_top: None,
                    border_bottom: None,
                    border_left: None,
                    border_right: None,
                    border_width: None,
                    stroke_outer: None,
                    stroke_outer_width: None,
                    opacity: None,
                    visible: None,
                    locked: None,
                    rotate: None,
                    blend_mode: None,
                    blur: None,
                    anchor: None,
                    anchor_zone: None,
                    anchor_sibling: Some("sib.target".to_owned()),
                    anchor_edge: Some("sideways".to_owned()), // not a valid edge
                    anchor_gap: None,
                    anchor_parent: None,
                    source_span: None,
                    unknown_props: BTreeMap::new(),
                })),
            ],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "anchor.unknown_edge"),
        "expected anchor.unknown_edge; codes: {:?}",
        codes(&report)
    );
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "anchor.unknown_edge")
        .expect("diagnostic must exist");
    assert_eq!(diag.severity, Severity::Error);
    assert!(report.has_errors());
}

// ── anchor-gap with non-px-convertible unit → anchor.gap_invalid_unit ─

#[test]
fn anchor_gap_non_px_unit_produces_gap_invalid_unit() {
    // Unit::Pct returns None from dim_to_px — the canonical non-convertible unit.
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.one",
            vec![
                minimal_rect("sib.target", None),
                Node::Rect(Box::new(RectNode {
                    shadow: None,
                    filter: None,
                    mask: None,
                    id: "rect.gap-pct".to_owned(),
                    name: None,
                    role: None,
                    x: None,
                    y: None,
                    w: Some(px(50.0)),
                    h: Some(px(50.0)),
                    radius: None,
                    radius_tl: None,
                    radius_tr: None,
                    radius_br: None,
                    radius_bl: None,
                    style: None,
                    fill: None,
                    stroke: None,
                    stroke_width: None,
                    stroke_alignment: None,
                    stroke_dash: None,
                    stroke_gap: None,
                    stroke_linecap: None,
                    border_top: None,
                    border_bottom: None,
                    border_left: None,
                    border_right: None,
                    border_width: None,
                    stroke_outer: None,
                    stroke_outer_width: None,
                    opacity: None,
                    visible: None,
                    locked: None,
                    rotate: None,
                    blend_mode: None,
                    blur: None,
                    anchor: None,
                    anchor_zone: None,
                    anchor_sibling: Some("sib.target".to_owned()),
                    anchor_edge: Some("below".to_owned()),
                    anchor_gap: Some(Dimension {
                        value: 10.0,
                        unit: Unit::Pct, // dim_to_px returns None for Pct
                    }),
                    anchor_parent: None,
                    source_span: None,
                    unknown_props: BTreeMap::new(),
                })),
            ],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "anchor.gap_invalid_unit"),
        "expected anchor.gap_invalid_unit; codes: {:?}",
        codes(&report)
    );
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "anchor.gap_invalid_unit")
        .expect("diagnostic must exist");
    assert_eq!(diag.severity, Severity::Warning);
}
