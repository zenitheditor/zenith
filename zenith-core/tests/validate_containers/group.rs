use super::*;

// ── Group helpers ─────────────────────────────────────────────────────

fn minimal_group(id: &str, children: Vec<Node>) -> Node {
    Node::Group(GroupNode {
        id: id.to_owned(),
        name: None,
        role: None,
        x: None,
        y: None,
        w: None,
        h: None,
        layout_item: Default::default(),
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
        semantic_role: None,
        intensity: None,
        layer_priority: None,
        symmetry_count: None,
        symmetry_cx: None,
        symmetry_cy: None,
        symmetry_start_angle: None,
        symmetry_mode: None,
        anchor: None,
        anchor_zone: None,
        anchor_sibling: None,
        anchor_edge: None,
        anchor_gap: None,
        anchor_parent: None,
        children,
        protected_regions: Vec::new(),
        editable_param_ids: Vec::new(),
        source_span: None,
        unknown_props: BTreeMap::new(),
    })
}

// ── Group: no required geometry — clean group has no errors ──────────

#[test]
fn group_with_children_no_errors() {
    let doc = doc_with(
        vec![color_token("color.fill")],
        vec![minimal_page(
            "page.one",
            vec![minimal_group(
                "group.one",
                vec![minimal_rect("rect.inner", Some(token_ref("color.fill")))],
            )],
        )],
    );
    let report = validate(&doc);
    assert!(
        report.diagnostics.is_empty(),
        "expected no diagnostics for clean group doc, got: {:?}",
        codes(&report)
    );
    assert!(!report.has_errors());
}

#[test]
fn group_shadow_parses_formats_and_validates() {
    let src = r##"zenith version=1 {
  project id="proj.group.shadow" name="Group Shadow"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#445566"
    token id="color.shadow" type="color" value="#00000088"
    token id="shadow.card" type="shadow" {
      layer dx=(px)2 dy=(px)4 blur=(px)8 color=(token)"color.shadow"
    }
  }
  styles {}
  document id="doc.group.shadow" title="Group Shadow" {
    page id="page.one" w=(px)200 h=(px)160 {
      group id="card" x=(px)10 y=(px)20 w=(px)100 h=(px)80 shadow=(token)"shadow.card" {
        rect id="card.bg" x=(px)0 y=(px)0 w=(px)100 h=(px)80 fill=(token)"color.fill"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let report = validate(&doc);
    assert!(
        report.diagnostics.is_empty(),
        "expected clean group shadow doc, got: {:?}",
        codes(&report)
    );

    let formatted = format_document(&doc).expect("format");
    let formatted = String::from_utf8(formatted).expect("utf8");
    assert!(
        formatted.contains(
            r#"group id="card" x=(px)10 y=(px)20 w=(px)100 h=(px)80 shadow=(token)"shadow.card""#
        ),
        "formatted group must keep attached shadow: {formatted}"
    );
}

#[test]
fn group_shadow_requires_shadow_token() {
    let src = r##"zenith version=1 {
  project id="proj.group.bad" name="Group Bad"
  tokens format="zenith-token-v1" {
    token id="color.shadow" type="color" value="#000000"
  }
  styles {}
  document id="doc.group.bad" title="Group Bad" {
    page id="page.one" w=(px)200 h=(px)160 {
      group id="card" shadow=(token)"color.shadow" {}
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let report = validate(&doc);
    assert!(
        has_code(&report, "token.incompatible_property"),
        "codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

#[test]
fn frame_shadow_rejects_raw_literal() {
    let src = r##"zenith version=1 {
  project id="proj.frame.bad" name="Frame Bad"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.frame.bad" title="Frame Bad" {
    page id="page.one" w=(px)200 h=(px)160 {
      frame id="panel" x=(px)10 y=(px)20 w=(px)100 h=(px)80 shadow="bad" {}
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let report = validate(&doc);
    assert!(
        has_code(&report, "token.raw_visual_literal"),
        "codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

// ── Group: nested id duplicate with page sibling → id.duplicate ──────

#[test]
fn group_nested_id_duplicate_with_page_sibling() {
    // Page has a rect "shared" and a group containing another node "shared".
    // The walk must share seen_ids across page-level and group-children,
    // so the second "shared" triggers id.duplicate.
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.one",
            vec![
                minimal_rect("shared", None),
                minimal_group("group.one", vec![minimal_rect("shared", None)]),
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

// ── Group: child with missing geometry surfaces → node.missing_geometry

#[test]
fn group_child_missing_geometry_surfaces() {
    // A rect nested inside a group has no `x` property; walk_node must
    // recurse into group children and report the missing geometry.
    let child_rect = Node::Rect(Box::new(RectNode {
        shadow: None,
        filter: None,
        mask: None,
        id: "rect.inner".to_owned(),
        name: None,
        role: None,
        x: None, // missing — triggers node.missing_geometry
        y: Some(pxv(0.0)),
        w: Some(pxv(50.0)),
        h: Some(pxv(50.0)),
        layout_item: Default::default(),
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
            vec![minimal_group("group.one", vec![child_rect])],
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

// ── Group: unknown property → node.unknown_property (Warning) ─────────

#[test]
fn group_unknown_property_errors() {
    let mut unknown_props = BTreeMap::new();
    unknown_props.insert(
        "future-blend".to_owned(),
        zenith_core::UnknownProperty {
            value: zenith_core::UnknownValue::String("multiply".to_owned()),
            ty: None,
        },
    );
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.one",
            vec![Node::Group(GroupNode {
                id: "group.one".to_owned(),
                name: None,
                role: None,
                x: None,
                y: None,
                w: None,
                h: None,
                layout_item: Default::default(),
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
                semantic_role: None,
                intensity: None,
                layer_priority: None,
                symmetry_count: None,
                symmetry_cx: None,
                symmetry_cy: None,
                symmetry_start_angle: None,
                symmetry_mode: None,
                anchor: None,
                anchor_zone: None,
                anchor_sibling: None,
                anchor_edge: None,
                anchor_gap: None,
                anchor_parent: None,
                children: vec![],
                protected_regions: Vec::new(),
                editable_param_ids: Vec::new(),
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
