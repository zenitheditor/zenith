//! Page lint: `align.near_miss`, `spacing.uneven_gap`,
//! `connector.crosses_node`, and the contrast of imported and scaled
//! instance content.

mod common;
use common::parse;
use zenith_core::{Diagnostic, FixHint, default_provider};
use zenith_scene::{DocumentPrep, ImportGraph, PageCompiler};

/// A one-page document with `body` on the page and `components` declared.
fn doc(components: &str, body: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.arrange" name="Arrange"
  tokens format="zenith-token-v1" {{
    token id="color.ink" type="color" value="#111111"
    token id="color.paper" type="color" value="#ffffff"
    token id="color.night" type="color" value="#101010"
    token id="color.snow" type="color" value="#fafafa"
    token id="color.box" type="color" value="#2255aa"
    token id="color.line" type="color" value="#333333"
    token id="size.type" type="dimension" value=(px)24
    token id="pos.x" type="dimension" value=(px)140
  }}
  styles {{}}
  components {{
{components}
  }}
  document id="doc.arrange" title="Arrange" {{
    page id="p" w=(px)800 h=(px)600 background=(token)"color.paper" {{
{body}
    }}
  }}
}}
"##
    )
}

fn lint(src: &str) -> Vec<Diagnostic> {
    let document = parse(src);
    let fonts = default_provider();
    let prep = DocumentPrep::new(&document, None, None);
    PageCompiler::new(&prep, &fonts)
        .compile_page_local(0)
        .diagnostics
}

fn with_code<'a>(diags: &'a [Diagnostic], code: &str) -> Vec<&'a Diagnostic> {
    diags.iter().filter(|d| d.code == code).collect()
}

fn rect(id: &str, x: &str, y: u32, w: u32, h: u32, extra: &str) -> String {
    format!(
        r#"      rect id="{id}" x={x} y=(px){y} w=(px){w} h=(px){h} fill=(token)"color.box" {extra}"#
    )
}

fn set_property(d: &Diagnostic) -> (String, String) {
    match d.fix() {
        Some(FixHint::SetProperty { property, to }) => (property.clone(), to.clone()),
        other => panic!("expected SetProperty fix, got {other:?}"),
    }
}

/// Two boxes share left=142; a third sits at 140, `g4_y` px down.
fn near_miss_body(g4_x: &str, g4_y: u32, g4_extra: &str) -> String {
    [
        rect("g1-a", "(px)142", 100, 100, 60, ""),
        rect("g1-b", "(px)142", 200, 100, 60, ""),
        rect("g4-b1", g4_x, g4_y, 100, 60, g4_extra),
    ]
    .join("\n")
}

#[test]
fn near_miss_fires_with_an_x_fix() {
    let diags = lint(&doc("", &near_miss_body("(px)140", 300, "")));
    let hits = with_code(&diags, "align.near_miss");
    assert_eq!(hits.len(), 1, "{diags:#?}");
    let d = hits[0];
    assert_eq!(d.subject_id.as_deref(), Some("g4-b1"));
    assert_eq!(
        d.message,
        "'g4-b1' left=140 is 2px off 142 shared by 'g1-a','g1-b' — set x=(px)142"
    );
    assert_eq!(set_property(d), ("x".to_owned(), "(px)142".to_owned()));
}

#[test]
fn near_miss_stays_quiet_without_a_near_shared_edge() {
    // Exact alignment.
    let diags = lint(&doc("", &near_miss_body("(px)142", 300, "")));
    assert!(
        with_code(&diags, "align.near_miss").is_empty(),
        "{diags:#?}"
    );
    // Only one other sibling shares the edge.
    let body = [
        rect("g1-a", "(px)142", 100, 100, 60, ""),
        rect("g4-b1", "(px)140", 300, 100, 60, ""),
        rect("other", "(px)500", 300, 100, 60, ""),
    ]
    .join("\n");
    let diags = lint(&doc("", &body));
    assert!(
        with_code(&diags, "align.near_miss").is_empty(),
        "{diags:#?}"
    );
    // Far away on the cross axis: 370px gap > 2 × 60 + 16.
    let diags = lint(&doc("", &near_miss_body("(px)140", 530, "")));
    assert!(
        with_code(&diags, "align.near_miss").is_empty(),
        "{diags:#?}"
    );
    // Decoration is exempt.
    let diags = lint(&doc(
        "",
        &near_miss_body("(px)140", 300, r#"role="decoration""#),
    ));
    assert!(
        with_code(&diags, "align.near_miss").is_empty(),
        "{diags:#?}"
    );
}

#[test]
fn near_miss_without_a_literal_position_has_no_fix() {
    let body = [
        rect("g1-a", "(px)142", 100, 100, 60, ""),
        rect("g1-b", "(px)142", 200, 100, 60, ""),
        rect("g4-b1", r#"(token)"pos.x""#, 300, 100, 60, ""),
    ]
    .join("\n");
    let diags = lint(&doc("", &body));
    let hits = with_code(&diags, "align.near_miss");
    assert_eq!(hits.len(), 1, "{diags:#?}");
    assert!(hits[0].fix().is_none(), "{:?}", hits[0]);
    assert!(
        hits[0].message.ends_with("— move 'g4-b1' right 2px"),
        "{}",
        hits[0].message
    );
}

#[test]
fn near_miss_skips_layout_frame_children() {
    // A column frame places its children: their right edges 100/100/98
    // would be a near miss if authored absolutely.
    let body = r#"      frame id="col" x=(px)100 y=(px)100 w=(px)300 layout="column" gap=(px)10 {
        rect id="c1" w=(px)100 h=(px)40 fill=(token)"color.box"
        rect id="c2" w=(px)100 h=(px)40 fill=(token)"color.box"
        rect id="c3" w=(px)98 h=(px)40 fill=(token)"color.box"
      }"#;
    let diags = lint(&doc("", body));
    assert!(
        with_code(&diags, "align.near_miss").is_empty(),
        "{diags:#?}"
    );
    assert!(
        with_code(&diags, "spacing.uneven_gap").is_empty(),
        "{diags:#?}"
    );
}

#[test]
fn near_miss_compares_text_baselines() {
    let text = |id: &str, x: u32, y: &str| {
        format!(
            r#"      text id="{id}" x=(px){x} y={y} w=(px)90 font-size=(token)"size.type" fill=(token)"color.ink" {{ span "Label" }}"#
        )
    };
    let body = [
        text("t1", 40, "(px)100"),
        text("t2", 140, "(px)100"),
        text("t3", 240, "(px)101.5"),
    ]
    .join("\n");
    let diags = lint(&doc("", &body));
    let hits = with_code(&diags, "align.near_miss");
    assert_eq!(hits.len(), 1, "{diags:#?}");
    let d = hits[0];
    assert_eq!(d.subject_id.as_deref(), Some("t3"));
    assert!(d.message.contains("baseline="), "{}", d.message);
    assert_eq!(set_property(d), ("y".to_owned(), "(px)100".to_owned()));
}

/// A row of boxes at `xs`, 100px wide, all at `y`.
fn row_at(xs: &[u32], y: u32) -> String {
    xs.iter()
        .enumerate()
        .map(|(i, x)| rect(&format!("r{}", i + 1), &format!("(px){x}"), y, 100, 60, ""))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A row of boxes at `xs`, 100px wide, all at y=300.
fn row(xs: &[u32]) -> String {
    row_at(xs, 300)
}

#[test]
fn uneven_gap_moves_the_single_outlier() {
    let diags = lint(&doc("", &row(&[40, 160, 280, 403])));
    let hits = with_code(&diags, "spacing.uneven_gap");
    assert_eq!(hits.len(), 1, "{diags:#?}");
    let d = hits[0];
    assert_eq!(d.subject_id.as_deref(), Some("r4"));
    assert!(
        d.message
            .starts_with("row 'r1','r2','r3','r4' has uneven gaps 20,20,23px"),
        "{}",
        d.message
    );
    assert_eq!(set_property(d), ("x".to_owned(), "(px)400".to_owned()));
}

#[test]
fn uneven_gap_of_three_has_no_single_fix() {
    let diags = lint(&doc("", &row(&[40, 160, 282])));
    let hits = with_code(&diags, "spacing.uneven_gap");
    assert_eq!(hits.len(), 1, "{diags:#?}");
    assert!(hits[0].fix().is_none(), "{:?}", hits[0]);
    assert!(
        hits[0].message.contains("set every gap to 21px"),
        "{}",
        hits[0].message
    );
}

#[test]
fn uneven_gap_stays_quiet_on_even_or_deliberate_spacing() {
    let diags = lint(&doc("", &row(&[40, 160, 280, 400])));
    assert!(
        with_code(&diags, "spacing.uneven_gap").is_empty(),
        "{diags:#?}"
    );
    // A 20px spread over a 20px median gap is deliberate.
    let diags = lint(&doc("", &row(&[40, 160, 280, 420])));
    assert!(
        with_code(&diags, "spacing.uneven_gap").is_empty(),
        "{diags:#?}"
    );
}

/// Two shapes joined by a connector with a box between them.
fn crossing_body(route: &str, blocker_extra: &str, backdrop: bool) -> String {
    let panel = if backdrop {
        r#"      rect id="panel" x=(px)20 y=(px)200 w=(px)760 h=(px)200 fill=(token)"color.paper""#
    } else {
        ""
    };
    format!(
        r#"{panel}
      shape id="a" kind="process" x=(px)40 y=(px)250 w=(px)100 h=(px)60 fill=(token)"color.box"
      shape id="b" kind="process" x=(px)600 y=(px)250 w=(px)100 h=(px)60 fill=(token)"color.box"
      rect id="c" x=(px)300 y=(px)240 w=(px)100 h=(px)80 fill=(token)"color.box" {blocker_extra}
      connector id="link" from="a" to="b" {route} stroke=(token)"color.line" stroke-width=(px)2"#
    )
}

#[test]
fn connector_crossing_fires_with_a_route_fix() {
    let diags = lint(&doc("", &crossing_body("", "", false)));
    let hits = with_code(&diags, "connector.crosses_node");
    assert_eq!(hits.len(), 1, "{diags:#?}");
    let d = hits[0];
    assert_eq!(d.subject_id.as_deref(), Some("link"));
    assert!(d.message.contains("rect 'c'"), "{}", d.message);
    assert_eq!(set_property(d), ("route".to_owned(), "avoid".to_owned()));

    // The fix routes around the box.
    let diags = lint(&doc("", &crossing_body(r#"route="avoid""#, "", false)));
    assert!(
        with_code(&diags, "connector.crosses_node").is_empty(),
        "{diags:#?}"
    );
}

#[test]
fn connector_crossing_skips_exempt_nodes_and_backdrops() {
    let diags = lint(&doc("", &crossing_body("", r#"role="decoration""#, false)));
    assert!(
        with_code(&diags, "connector.crosses_node").is_empty(),
        "{diags:#?}"
    );
    // The panel behind the diagram holds both endpoints; only 'c' crosses.
    let diags = lint(&doc("", &crossing_body("", "", true)));
    let hits = with_code(&diags, "connector.crosses_node");
    assert_eq!(hits.len(), 1, "{diags:#?}");
    assert!(!hits[0].message.contains("'panel'"), "{}", hits[0].message);
}

const IMPORTED: &str = r##"zenith version=1 {
  project id="proj.lib" name="Library"
  tokens format="zenith-token-v1" {
    token id="color.pale" type="color" value="#f4f4f4"
    token id="size.type" type="dimension" value=(px)24
  }
  styles {}
  components {
    component id="card" {
      text id="t" x=(px)0 y=(px)0 w=(px)300 h=(px)40 font-size=(token)"size.type" fill=(token)"color.pale" { span "Faint" }
    }
  }
  document id="doc.lib" title="Library" {
    page id="lib" w=(px)10 h=(px)10 {}
  }
}
"##;

#[test]
fn imported_instance_text_contrast_uses_the_import_scope() {
    // The host has no `color.pale`: only the import scope resolves it.
    let host = parse(&doc(
        "",
        r#"      instance id="inst" source="library#component.card" x=(px)40 y=(px)300"#,
    ));
    let imported = parse(IMPORTED);
    let graph = ImportGraph::new().with_document("library", &imported);
    let fonts = default_provider();
    let prep = DocumentPrep::new(&host, None, Some(&graph));
    let diags = PageCompiler::new(&prep, &fonts)
        .compile_page_local(0)
        .diagnostics;
    let hit = diags
        .iter()
        .find(|d| d.code.starts_with("contrast.") && d.subject_id.as_deref() == Some("inst/t"))
        .unwrap_or_else(|| panic!("{diags:#?}"));
    assert_eq!(hit.import(), Some("library"));
    let span = hit.span.expect("span");
    let authored = &IMPORTED[span.start..span.end];
    assert!(authored.starts_with(r#"text id="t""#), "{authored}");
}

/// A 100x20 component scaled by 3 into a 300x300 contain box at (100,100):
/// the text lands at (100,220)-(400,280). `backdrop_y` places a dark host
/// rect (100,y)-(400,y+100).
fn scaled_doc(backdrop_y: u32) -> String {
    let components = r#"    component id="chip" {
      text id="t" x=(px)0 y=(px)0 w=(px)100 h=(px)20 font-size=(px)12 fill=(token)"color.snow" { span "Chip" }
    }"#;
    let body = format!(
        r#"      rect id="dark" x=(px)100 y=(px){backdrop_y} w=(px)300 h=(px)100 fill=(token)"color.night"
      instance id="inst" component="chip" x=(px)100 y=(px)100 w=(px)300 h=(px)300"#
    );
    doc(components, &body)
}

#[test]
fn scaled_instance_text_contrast_samples_the_scaled_box() {
    // The dark rect sits under the scaled text, not under its unscaled box.
    let src = scaled_doc(200);
    let diags = lint(&src);
    assert!(
        diags.iter().all(
            |d| !(d.code.starts_with("contrast.") && d.subject_id.as_deref() == Some("inst/t"))
        ),
        "{diags:#?}"
    );
    // Moved off the scaled box, the near-white text sits on the white page.
    let src = scaled_doc(450);
    let diags = lint(&src);
    let hit = diags
        .iter()
        .find(|d| d.code.starts_with("contrast.") && d.subject_id.as_deref() == Some("inst/t"))
        .unwrap_or_else(|| panic!("{diags:#?}"));
    let span = hit.span.expect("span");
    assert!(src[span.start..span.end].starts_with(r#"text id="t""#));
}

#[test]
fn arrangement_lint_is_deterministic() {
    let body = format!(
        "{}\n{}",
        near_miss_body("(px)140", 300, ""),
        row_at(&[300, 420, 540, 663], 480),
    );
    let src = doc("", &body);
    let first = lint(&src);
    assert!(
        !with_code(&first, "align.near_miss").is_empty(),
        "{first:#?}"
    );
    assert!(
        !with_code(&first, "spacing.uneven_gap").is_empty(),
        "{first:#?}"
    );
    assert_eq!(first, lint(&src));
}
