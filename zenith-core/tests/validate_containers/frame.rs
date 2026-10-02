use super::*;

// ── Frame helpers ─────────────────────────────────────────────────────

fn minimal_frame(id: &str, x: f64, y: f64, w: f64, h: f64, children: Vec<Node>) -> Node {
    Node::Frame(FrameNode {
        id: id.to_owned(),
        name: None,
        role: None,
        x: Some(pxv(x)),
        y: Some(pxv(y)),
        w: Some(pxv(w)),
        h: Some(pxv(h)),
        layout: None,
        columns: None,
        rows: None,
        opacity: None,
        visible: None,
        locked: None,
        rotate: None,
        blend_mode: None,
        shadow: None,
        filter: None,
        mask: None,
        blur: None,
        style: None,
        anchor: None,
        anchor_zone: None,
        anchor_sibling: None,
        anchor_edge: None,
        anchor_gap: None,
        anchor_parent: None,
        children,
        source_span: None,
        unknown_props: BTreeMap::new(),
    })
}

// ── Frame: clean doc with valid frame + child rect → no diagnostics ───

#[test]
fn frame_clean_doc_no_errors() {
    // Child rect sits fully inside the frame box (40,40,120,100), so neither
    // off_canvas nor frame.child_overflow fire.
    let inner = Node::Rect(Box::new(RectNode {
        shadow: None,
        filter: None,
        mask: None,
        id: "rect.inner".to_owned(),
        name: None,
        role: None,
        x: Some(pxv(50.0)),
        y: Some(pxv(50.0)),
        w: Some(pxv(40.0)),
        h: Some(pxv(40.0)),
        radius: None,
        radius_tl: None,
        radius_tr: None,
        radius_br: None,
        radius_bl: None,
        style: None,
        fill: Some(token_ref("color.fill")),
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
        anchor_sibling: None,
        anchor_edge: None,
        anchor_gap: None,
        anchor_parent: None,
        source_span: None,
        unknown_props: BTreeMap::new(),
    }));
    let doc = doc_with(
        vec![color_token("color.fill")],
        vec![minimal_page(
            "page.one",
            vec![minimal_frame(
                "frame.clip",
                40.0,
                40.0,
                120.0,
                100.0,
                vec![inner],
            )],
        )],
    );
    let report = validate(&doc);
    assert!(
        report.diagnostics.is_empty(),
        "expected no diagnostics for clean frame doc, got: {:?}",
        codes(&report)
    );
    assert!(!report.has_errors());
}

// ── Frame: missing x → node.missing_geometry ──────────────────────────

#[test]
fn frame_missing_x_produces_node_missing_geometry() {
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.one",
            vec![Node::Frame(FrameNode {
                id: "frame.nox".to_owned(),
                name: None,
                role: None,
                x: None, // missing
                y: Some(pxv(0.0)),
                w: Some(pxv(100.0)),
                h: Some(pxv(100.0)),
                layout: None,
                columns: None,
                rows: None,
                opacity: None,
                visible: None,
                locked: None,
                rotate: None,
                blend_mode: None,
                shadow: None,
                filter: None,
                mask: None,
                blur: None,
                style: None,
                anchor: None,
                anchor_zone: None,
                anchor_sibling: None,
                anchor_edge: None,
                anchor_gap: None,
                anchor_parent: None,
                children: vec![],
                source_span: None,
                unknown_props: BTreeMap::new(),
            })],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "node.missing_geometry"),
        "codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

// ── Frame: missing h → node.missing_geometry ──────────────────────────

#[test]
fn frame_missing_h_produces_node_missing_geometry() {
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.one",
            vec![Node::Frame(FrameNode {
                id: "frame.noh".to_owned(),
                name: None,
                role: None,
                x: Some(pxv(0.0)),
                y: Some(pxv(0.0)),
                w: Some(pxv(100.0)),
                h: None, // missing
                layout: None,
                columns: None,
                rows: None,
                opacity: None,
                visible: None,
                locked: None,
                rotate: None,
                blend_mode: None,
                shadow: None,
                filter: None,
                mask: None,
                blur: None,
                style: None,
                anchor: None,
                anchor_zone: None,
                anchor_sibling: None,
                anchor_edge: None,
                anchor_gap: None,
                anchor_parent: None,
                children: vec![],
                source_span: None,
                unknown_props: BTreeMap::new(),
            })],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "node.missing_geometry"),
        "codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

// ── Frame: child rect with no x → node.missing_geometry (recursion) ───

#[test]
fn frame_child_missing_geometry_surfaces() {
    // A rect nested inside a frame has no `x`; walk_node must recurse
    // into frame children and report the missing geometry.
    let child_rect = Node::Rect(Box::new(RectNode {
        shadow: None,
        filter: None,
        mask: None,
        id: "rect.inner".to_owned(),
        name: None,
        role: None,
        x: None, // missing
        y: Some(pxv(0.0)),
        w: Some(pxv(50.0)),
        h: Some(pxv(50.0)),
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
        anchor_sibling: None,
        anchor_edge: None,
        anchor_gap: None,
        anchor_parent: None,
        source_span: None,
        unknown_props: BTreeMap::new(),
    }));
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.one",
            vec![minimal_frame(
                "frame.clip",
                0.0,
                0.0,
                100.0,
                100.0,
                vec![child_rect],
            )],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "node.missing_geometry"),
        "codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

// ── Frame: child overflow advisories ──────────────────────────────────

/// A frame child whose `x + w` exceeds the frame's right edge → advisory
/// `frame.child_overflow`.
#[test]
fn frame_child_overflowing_right_edge_advises() {
    // Frame box: x=40 y=40 w=120 h=100 → right edge at 160.
    // Child rect: x=100 w=100 → right edge at 200 > 160 → protrudes.
    let doc = doc_with(
        vec![],
        vec![bounded_page(
            "page.one",
            1000.0,
            1000.0,
            vec![minimal_frame(
                "frame.clip",
                40.0,
                40.0,
                120.0,
                100.0,
                vec![rect_at("rect.over", 100.0, 50.0, 100.0, 40.0)],
            )],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "frame.child_overflow"),
        "expected frame.child_overflow; codes: {:?}",
        codes(&report)
    );
}

/// A frame child fully inside the frame box → no overflow advisory.
#[test]
fn frame_child_fully_inside_is_clean() {
    // Frame box: x=40 y=40 w=120 h=100. Child rect fully inside.
    let doc = doc_with(
        vec![],
        vec![bounded_page(
            "page.one",
            1000.0,
            1000.0,
            vec![minimal_frame(
                "frame.clip",
                40.0,
                40.0,
                120.0,
                100.0,
                vec![rect_at("rect.in", 50.0, 50.0, 40.0, 40.0)],
            )],
        )],
    );
    let report = validate(&doc);
    assert!(
        !has_code(&report, "frame.child_overflow"),
        "inside child must not overflow; codes: {:?}",
        codes(&report)
    );
}

/// A flow-frame child with no explicit geometry → no overflow advisory
/// (node_bbox is None, so the child is naturally skipped).
#[test]
fn flow_frame_child_without_geometry_is_skipped() {
    let child_rect = Node::Rect(Box::new(RectNode {
        shadow: None,
        filter: None,
        mask: None,
        id: "rect.flow".to_owned(),
        name: None,
        role: None,
        x: None,
        y: None,
        w: None,
        h: None,
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
        anchor_sibling: None,
        anchor_edge: None,
        anchor_gap: None,
        anchor_parent: None,
        source_span: None,
        unknown_props: BTreeMap::new(),
    }));
    let flow_frame = Node::Frame(FrameNode {
        id: "frame.flow".to_owned(),
        name: None,
        role: None,
        x: Some(pxv(40.0)),
        y: Some(pxv(40.0)),
        w: Some(pxv(120.0)),
        h: Some(pxv(100.0)),
        layout: Some("flow".to_owned()),
        columns: None,
        rows: None,
        opacity: None,
        visible: None,
        locked: None,
        rotate: None,
        blend_mode: None,
        shadow: None,
        filter: None,
        mask: None,
        blur: None,
        style: None,
        anchor: None,
        anchor_zone: None,
        anchor_sibling: None,
        anchor_edge: None,
        anchor_gap: None,
        anchor_parent: None,
        children: vec![child_rect],
        source_span: None,
        unknown_props: BTreeMap::new(),
    });
    let doc = doc_with(
        vec![],
        vec![bounded_page("page.one", 1000.0, 1000.0, vec![flow_frame])],
    );
    let report = validate(&doc);
    assert!(
        !has_code(&report, "frame.child_overflow"),
        "flow child without geometry must be skipped; codes: {:?}",
        codes(&report)
    );
}

// ── Frame: nested id duplicate with page sibling → id.duplicate ───────

#[test]
fn frame_nested_id_duplicate_with_page_sibling() {
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.one",
            vec![
                minimal_rect("shared", None),
                minimal_frame(
                    "frame.clip",
                    0.0,
                    0.0,
                    100.0,
                    100.0,
                    vec![minimal_rect("shared", None)],
                ),
            ],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "id.duplicate"),
        "codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

// ── Frame: unknown property → node.unknown_property (Warning) ─────────

#[test]
fn frame_unknown_property_errors() {
    let mut unknown_props = BTreeMap::new();
    unknown_props.insert(
        "future-scroll".to_owned(),
        zenith_core::UnknownProperty {
            value: zenith_core::UnknownValue::Bool(true),
            ty: None,
        },
    );
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.one",
            vec![Node::Frame(FrameNode {
                id: "frame.one".to_owned(),
                name: None,
                role: None,
                x: Some(pxv(0.0)),
                y: Some(pxv(0.0)),
                w: Some(pxv(100.0)),
                h: Some(pxv(100.0)),
                layout: None,
                columns: None,
                rows: None,
                opacity: None,
                visible: None,
                locked: None,
                rotate: None,
                blend_mode: None,
                shadow: None,
                filter: None,
                mask: None,
                blur: None,
                style: None,
                anchor: None,
                anchor_zone: None,
                anchor_sibling: None,
                anchor_edge: None,
                anchor_gap: None,
                anchor_parent: None,
                children: vec![],
                source_span: None,
                unknown_props,
            })],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "node.unknown_property"),
        "codes: {:?}",
        codes(&report)
    );
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.unknown_property")
        .expect("should exist");
    assert_eq!(diag.severity, Severity::Error);
    assert!(report.has_errors());
}
