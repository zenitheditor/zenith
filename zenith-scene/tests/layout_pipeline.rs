//! Auto-layout lowering in the compile pipeline: downstream consumers
//! (connectors, anchors, contrast) read laid-out geometry, masters and
//! components lower too, output is deterministic, and documents without a
//! layout frame skip the clone.

mod common;
use common::*;
use zenith_scene::{DocumentPrep, PageCompiler};

fn doc(children: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.lp" name="LP"
  tokens format="zenith-token-v1" {{
    token id="color.k" type="color" value="#000000"
    token id="color.ink" type="color" value="#222222"
    token id="color.dark" type="color" value="#111111"
    token id="color.w" type="color" value="#ffffff"
  }}
  styles {{}}
  components {{
    component id="comp.card" {{
      frame id="card" x=(px)0 y=(px)0 w=(px)100 layout="column" gap=(px)5 {{
        rect id="top" h=(px)10 fill=(token)"color.k"
        rect id="bottom" h=(px)20 fill=(token)"color.k"
      }}
    }}
  }}
  masters {{
    master id="m" {{
      frame id="band" x=(px)0 y=(px)500 w=(px)800 h=(px)40 layout="row" justify="end" {{
        rect id="mark" w=(px)30 fill=(token)"color.k"
      }}
    }}
  }}
  document id="doc.lp" title="LP" {{
    page id="p" w=(px)800 h=(px)600 background=(token)"color.w" {{
{children}
    }}
  }}
}}
"##
    )
}

#[test]
fn connectors_attach_to_laid_out_nodes() {
    let src = doc(
        r#"      frame id="f" x=(px)100 y=(px)100 w=(px)200 layout="column" gap=(px)60 {
        rect id="a" h=(px)40 fill=(token)"color.k"
        rect id="b" h=(px)40 fill=(token)"color.k"
      }
      connector id="c" from="a" to="b" stroke=(token)"color.k""#,
    );
    let result = compile(&parse(&src), &default_provider());
    let stroke = result.scene.commands.iter().find_map(|c| match c {
        SceneCommand::StrokePolyline {
            points,
            closed: false,
            ..
        } => Some(points.clone()),
        _ => None,
    });
    let points = stroke.expect("connector stroke");
    // a spans y 100..140, b spans 200..240, both x 100..300 (center 200).
    assert_eq!(points.first().copied(), Some(200.0), "{points:?}");
    let ys: Vec<f64> = points.iter().skip(1).step_by(2).copied().collect();
    assert!(ys.iter().all(|y| (140.0..=200.0).contains(y)), "{points:?}");
}

#[test]
fn anchors_resolve_against_a_laid_out_sibling() {
    let src = doc(
        r#"      frame id="f" x=(px)40 y=(px)50 w=(px)200 layout="column" {
        rect id="a" h=(px)30 fill=(token)"color.k"
        rect id="n" anchor-sibling="a" anchor-edge="below" anchor-gap=(px)5 w=(px)20 h=(px)10 position="absolute" fill=(token)"color.k"
      }"#,
    );
    let result = compile(&parse(&src), &default_provider());
    assert!(
        fill_rects(&result).contains(&(40.0, 85.0, 20.0, 10.0)),
        "{:?}",
        fill_rects(&result)
    );
}

#[test]
fn contrast_reads_the_laid_out_backdrop() {
    let src = doc(
        r#"      frame id="f" x=(px)10 y=(px)10 layout="column" padding=(px)10 fill=(token)"color.dark" {
        text id="t" font-size=(px)14 fill=(token)"color.ink" {
          span "Low contrast"
        }
      }"#,
    );
    let parsed = parse(&src);
    let validated = zenith_core::validate(&parsed);
    assert!(
        !validated
            .diagnostics
            .iter()
            .any(|d| d.code.starts_with("contrast.")),
        "validation leaves layout pages to the scene: {:?}",
        validated.diagnostics
    );
    let result = compile(&parsed, &default_provider());
    let low: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "contrast.low" || d.code == "contrast.invisible")
        .collect();
    assert_eq!(low.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(low[0].subject_id.as_deref(), Some("t"));
    assert!(low[0].span.is_some(), "points at the authored node");
}

#[test]
fn masters_and_components_lower_at_expansion() {
    let src = doc(r#"      instance id="i" component="comp.card" x=(px)300 y=(px)200"#).replace(
        r#"page id="p" w=(px)800 h=(px)600"#,
        r#"page id="p" w=(px)800 h=(px)600 master="m""#,
    );
    let result = compile(&parse(&src), &default_provider());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let rects = fill_rects(&result);
    assert!(
        rects.contains(&(770.0, 500.0, 30.0, 40.0)),
        "master: {rects:?}"
    );
    assert!(
        rects.contains(&(300.0, 200.0, 100.0, 10.0)),
        "instance: {rects:?}"
    );
    assert!(
        rects.contains(&(300.0, 215.0, 100.0, 20.0)),
        "instance: {rects:?}"
    );
}

#[test]
fn lowering_is_deterministic() {
    let src = doc(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)300 layout="row" wrap=#true gap=(px)7 justify="center" {
        text id="t1" font-size=(px)18 fill=(token)"color.k" {
          span "Alpha beta"
        }
        rect id="r" w="fill" min-w=(px)40 h=(px)12 fill=(token)"color.k"
        text id="t2" font-size=(px)18 fill=(token)"color.k" {
          span "Gamma delta epsilon"
        }
      }"#,
    );
    let parsed = parse(&src);
    let one = compile(&parsed, &default_provider());
    let two = compile(&parsed, &default_provider());
    assert_eq!(
        one.scene.to_json().expect("json"),
        two.scene.to_json().expect("json")
    );
    assert_eq!(one.diagnostics, two.diagnostics);
}

#[test]
fn documents_without_layout_frames_are_not_cloned() {
    let fonts = default_provider();
    let plain = parse(&doc(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)100 h=(px)100 {
        rect id="r" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.k"
      }"#,
    ));
    let prep = DocumentPrep::new(&plain, None, None);
    let compiler = PageCompiler::new(&prep, &fonts);
    assert!(std::ptr::eq(compiler.document(), prep.document()));
    assert_eq!(compiler.layout_boxes(0).map(|m| m.len()), Some(0));

    let laid = parse(&doc(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)100 layout="column" {
        rect id="r" h=(px)10 fill=(token)"color.k"
      }"#,
    ));
    let prep = DocumentPrep::new(&laid, None, None);
    let compiler = PageCompiler::new(&prep, &fonts);
    assert!(!std::ptr::eq(compiler.document(), prep.document()));
    let boxes = compiler.layout_boxes(0).expect("page 0");
    assert_eq!(boxes.len(), 2);
    let r = boxes.get("r").expect("r");
    assert_eq!((r.x, r.y, r.w, r.h), (0.0, 0.0, 100.0, 10.0));
}

#[test]
fn layout_boxes_are_page_absolute_inside_groups() {
    let src = doc(r#"      group id="g" x=(px)100 y=(px)50 {
        frame id="f" x=(px)10 y=(px)10 w=(px)50 layout="column" {
          rect id="r" h=(px)10 fill=(token)"color.k"
        }
      }"#);
    let boxes = zenith_scene::layout_boxes(&parse(&src), 0, &default_provider());
    let r = boxes.get("r").expect("r");
    assert_eq!((r.x, r.y, r.w, r.h), (110.0, 60.0, 50.0, 10.0));
    let result = compile(&parse(&src), &default_provider());
    assert!(fill_rects(&result).contains(&(110.0, 60.0, 50.0, 10.0)));
}

#[test]
fn chain_members_in_a_column_flow_through_laid_out_boxes() {
    let long = "One two three four five six seven eight nine ten eleven twelve thirteen \
                fourteen fifteen sixteen seventeen eighteen nineteen twenty";
    let src = doc(&format!(
        r#"      frame id="f" x=(px)20 y=(px)20 w=(px)160 layout="column" gap=(px)30 {{
        text id="c1" chain="story" h=(px)40 font-size=(px)16 fill=(token)"color.k" {{
          span "{long}"
        }}
        text id="c2" chain="story" h=(px)40 font-size=(px)16 fill=(token)"color.k"
      }}"#
    ));
    let result = compile(&parse(&src), &default_provider());
    let ys: Vec<f64> = result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::DrawGlyphRun { y, .. } => Some(*y),
            _ => None,
        })
        .collect();
    assert!(ys.iter().any(|y| *y < 60.0), "first member: {ys:?}");
    assert!(
        ys.iter().any(|y| *y > 90.0 && *y < 130.0),
        "second member: {ys:?}"
    );
}
