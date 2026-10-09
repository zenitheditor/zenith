//! Page lint: `layout.block_overlap` between sibling blocks.

mod common;
use common::parse;
use zenith_core::Diagnostic;
use zenith_core::default_provider;
use zenith_scene::{DocumentPrep, PageCompiler};

/// A one-page 800 x 600 document with `body` on the page.
fn doc(body: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.blocks" name="Blocks"
  tokens format="zenith-token-v1" {{
    token id="color.paper" type="color" value="#ffffff"
    token id="color.panel" type="color" value="#e8ecf2"
    token id="color.card" type="color" value="#2255aa"
  }}
  styles {{}}
  document id="doc.blocks" title="Blocks" {{
    page id="p" w=(px)800 h=(px)600 background=(token)"color.paper" {{
{body}
    }}
  }}
}}
"##
    )
}

fn hits(src: &str) -> Vec<Diagnostic> {
    let document = parse(src);
    let fonts = default_provider();
    let prep = DocumentPrep::new(&document, None, None);
    PageCompiler::new(&prep, &fonts)
        .compile_page_local(0)
        .diagnostics
        .into_iter()
        .filter(|d| d.code == "layout.block_overlap")
        .collect()
}

fn rect(id: &str, (x, y, w, h): (u32, u32, u32, u32), extra: &str) -> String {
    format!(
        r#"      rect id="{id}" x=(px){x} y=(px){y} w=(px){w} h=(px){h} fill=(token)"color.card" {extra}"#
    )
}

#[test]
fn partial_overlap_fires_on_the_later_block() {
    let body = format!(
        "{}\n{}",
        rect("a", (40, 40, 200, 100), ""),
        rect("b", (200, 80, 200, 100), "")
    );
    let found = hits(&doc(&body));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].subject_id.as_deref(), Some("b"));
    assert_eq!(
        found[0].message,
        "block \"b\" overlaps \"a\" by 40x60 at (200,80); move one, group them as one \
         drawing, or mark the intended layer role=\"decoration\"/\"background\"."
    );
    assert!(found[0].fix().is_none());
}

#[test]
fn group_children_form_one_drawing() {
    let body = format!(
        "      group id=\"icon\" {{\n{}\n{}\n      }}",
        rect("shackle", (40, 40, 200, 100), ""),
        rect("body", (200, 80, 200, 100), "")
    );
    assert!(hits(&doc(&body)).is_empty());
}

#[test]
fn containment_is_silent() {
    let body = format!(
        "{}\n{}",
        r#"      frame id="panel" x=(px)40 y=(px)40 w=(px)400 h=(px)300 fill=(token)"color.panel""#,
        rect("card", (60, 60, 100, 100), "")
    );
    assert!(hits(&doc(&body)).is_empty());
}

#[test]
fn decoration_sibling_is_silent() {
    for extra in [
        r#"role="decoration""#,
        r#"role="background""#,
        "opacity=0.3",
    ] {
        let body = format!(
            "{}\n{}",
            rect("a", (40, 40, 200, 100), ""),
            rect("b", (200, 80, 200, 100), extra)
        );
        assert!(hits(&doc(&body)).is_empty(), "{extra}");
    }
}

#[test]
fn outlines_effects_and_off_page_paint_are_silent() {
    let cases = [
        // An unfilled ellipse paints a ring only.
        format!(
            "{}\n{}",
            r#"      ellipse id="ring" x=(px)40 y=(px)40 w=(px)300 h=(px)300 stroke=(token)"color.card" stroke-width=(px)2"#,
            rect("dot", (300, 100, 100, 100), "")
        ),
        // A blend mode composites on purpose.
        format!(
            "{}\n{}",
            rect("a", (40, 40, 200, 100), ""),
            rect("b", (200, 80, 200, 100), r#"blend-mode="screen""#)
        ),
        // Only the on-page part of the corner circle is seen: it sits inside
        // the full-page backdrop.
        format!(
            "{}\n{}",
            rect("bg", (0, 0, 800, 600), ""),
            r#"      ellipse id="corner" x=(px)600 y=(px)400 w=(px)400 h=(px)400 fill=(token)"color.panel""#
        ),
    ];
    for body in cases {
        assert!(hits(&doc(&body)).is_empty(), "{body}");
    }
}

#[test]
fn disjoint_siblings_are_silent() {
    let body = format!(
        "{}\n{}",
        rect("a", (40, 40, 200, 100), ""),
        rect("b", (260, 40, 200, 100), "")
    );
    assert!(hits(&doc(&body)).is_empty());
}

#[test]
fn same_flow_siblings_are_silent() {
    let body = r#"      frame id="row" x=(px)40 y=(px)40 w=(px)600 h=(px)120 layout="row" gap=(px)-40 {
        rect id="a" w=(px)200 h=(px)100 fill=(token)"color.card"
        rect id="b" w=(px)200 h=(px)100 fill=(token)"color.card"
      }"#;
    let src = doc(body);
    // The negative gap makes the two flow children overlap.
    let document = parse(&src);
    let fonts = default_provider();
    let prep = DocumentPrep::new(&document, None, None);
    let boxes = PageCompiler::new(&prep, &fonts).compiled_boxes(0);
    let (a, b) = (boxes["a"].rect, boxes["b"].rect);
    assert!(b.x < a.x + a.w, "{a:?} {b:?}");
    assert!(hits(&src).is_empty());
}

#[test]
fn hug_column_grown_into_the_next_sibling_fires() {
    let body = format!(
        r#"      frame id="col" x=(px)40 y=(px)40 w=(px)300 h="hug" layout="column" gap=(px)10 {{
{children}
      }}
      frame id="next" x=(px)40 y=(px)250 w=(px)300 h=(px)100 fill=(token)"color.panel""#,
        children = ["c1", "c2", "c3", "c4"]
            .map(|id| format!(
                r#"        rect id="{id}" w=(px)300 h=(px)60 fill=(token)"color.card""#
            ))
            .join("\n")
    );
    let found = hits(&doc(&body));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].subject_id.as_deref(), Some("next"));
    assert!(
        found[0]
            .message
            .starts_with("block \"next\" overlaps \"col\" by 300x60 at (40,250)"),
        "{}",
        found[0].message
    );
}

#[test]
fn block_lint_is_deterministic() {
    let body = format!(
        "{}\n{}\n{}",
        rect("a", (40, 40, 200, 100), ""),
        rect("b", (200, 80, 200, 100), ""),
        rect("c", (100, 120, 200, 100), "")
    );
    let src = doc(&body);
    let first = hits(&src);
    assert_eq!(first.len(), 3, "{first:#?}");
    assert_eq!(first, hits(&src));
}
