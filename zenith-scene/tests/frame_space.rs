//! Frame child space: every frame translates its children by its top-left.
//! Nested frames, frames in groups, plain frames inside layout frames, and
//! rotated frames place children frame-local. Page anchors, runaround boxes,
//! connector targets, layout boxes, and compiled boxes stay page-absolute.

mod common;
use std::collections::BTreeMap;

use common::*;
use zenith_core::default_provider;
use zenith_scene::ir::SceneCommand;
use zenith_scene::{DocumentPrep, PageCompiler, compile, layout_boxes};

fn doc(body: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="p" name="P"
  tokens format="zenith-token-v1" {{
    token id="color.k" type="color" value="#000000"
    token id="color.line" type="color" value="#1e3a8a"
  }}
  styles {{}}
  document id="d" title="D" {{
    page id="pg" w=(px)400 h=(px)300 {{
      {body}
    }}
  }}
}}"##
    )
}

fn commands(body: &str) -> Vec<SceneCommand> {
    let result = compile(&parse(&doc(body)), &default_provider());
    assert!(
        result.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    result.scene.commands
}

fn rects(body: &str) -> Vec<(f64, f64, f64, f64)> {
    let result = compile(&parse(&doc(body)), &default_provider());
    assert!(
        result.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    fill_rects(&result)
}

fn clips(cmds: &[SceneCommand]) -> Vec<(f64, f64, f64, f64)> {
    cmds.iter()
        .filter_map(|c| match c {
            SceneCommand::PushClip { x, y, w, h } => Some((*x, *y, *w, *h)),
            _ => None,
        })
        .collect()
}

fn compiled_rects(body: &str) -> BTreeMap<String, (f64, f64, f64, f64)> {
    let doc = parse(&doc(body));
    let prep = DocumentPrep::new(&doc, None, None);
    let fonts = default_provider();
    PageCompiler::new(&prep, &fonts)
        .compiled_boxes(0)
        .into_iter()
        .map(|(k, b)| (k, (b.rect.x, b.rect.y, b.rect.w, b.rect.h)))
        .collect()
}

fn lowered_boxes(body: &str) -> BTreeMap<String, (f64, f64, f64, f64)> {
    layout_boxes(&parse(&doc(body)), 0, &default_provider())
        .into_iter()
        .map(|(k, b)| (k, (b.x, b.y, b.w, b.h)))
        .collect()
}

const RECT: &str = r#"rect id="r" x=(px)5 y=(px)6 w=(px)10 h=(px)10 fill=(token)"color.k""#;

#[test]
fn frame_translates_child_by_its_top_left() {
    let body = format!(r#"frame id="f" x=(px)30 y=(px)40 w=(px)100 h=(px)100 {{ {RECT} }}"#);
    assert_eq!(rects(&body), vec![(35.0, 46.0, 10.0, 10.0)]);
}

#[test]
fn nested_frames_accumulate_their_origins() {
    let body = format!(
        r#"frame id="f1" x=(px)10 y=(px)20 w=(px)300 h=(px)200 {{
        frame id="f2" x=(px)30 y=(px)40 w=(px)100 h=(px)100 {{ {RECT} }}
      }}"#
    );
    let cmds = commands(&body);
    // Page clip, then f1 at its own box, then f2 at 10+30 / 20+40.
    assert_eq!(
        clips(&cmds),
        vec![
            (0.0, 0.0, 400.0, 300.0),
            (10.0, 20.0, 300.0, 200.0),
            (40.0, 60.0, 100.0, 100.0),
        ]
    );
    assert_eq!(rects(&body), vec![(45.0, 66.0, 10.0, 10.0)]);
}

#[test]
fn frame_in_group_adds_both_origins() {
    let body = format!(
        r#"group id="g" x=(px)100 y=(px)50 {{
        frame id="f" x=(px)10 y=(px)10 w=(px)100 h=(px)100 {{ {RECT} }}
      }}"#
    );
    assert_eq!(rects(&body), vec![(115.0, 66.0, 10.0, 10.0)]);
}

#[test]
fn plain_frame_inside_a_layout_frame_translates_its_children() {
    // A column at (50, 40): the flow frame lands at its content top-left,
    // the absolute frame at column-local (100, 20). Both translate their
    // children.
    let body = r#"frame id="col" x=(px)50 y=(px)40 w=(px)300 h=(px)200 layout="column" {
        frame id="flow" w=(px)80 h=(px)50 {
          rect id="a" x=(px)5 y=(px)6 w=(px)10 h=(px)10 fill=(token)"color.k"
        }
        frame id="abs" x=(px)100 y=(px)20 w=(px)80 h=(px)50 position="absolute" {
          rect id="b" x=(px)7 y=(px)8 w=(px)10 h=(px)10 fill=(token)"color.k"
        }
      }"#;
    let found = rects(body);
    assert!(found.contains(&(55.0, 46.0, 10.0, 10.0)), "{found:?}");
    assert!(found.contains(&(157.0, 68.0, 10.0, 10.0)), "{found:?}");
    let lowered = lowered_boxes(body);
    assert_eq!(lowered.get("col"), Some(&(50.0, 40.0, 300.0, 200.0)));
    assert_eq!(lowered.get("flow"), Some(&(50.0, 40.0, 80.0, 50.0)));
    let compiled = compiled_rects(body);
    assert_eq!(compiled.get("a"), Some(&(55.0, 46.0, 10.0, 10.0)));
    assert_eq!(compiled.get("b"), Some(&(157.0, 68.0, 10.0, 10.0)));
}

#[test]
fn layout_frame_inside_a_plain_frame_lowers_frame_local() {
    let body = r#"frame id="host" x=(px)100 y=(px)100 w=(px)250 h=(px)150 {
        frame id="row" x=(px)10 y=(px)10 layout="row" gap=(px)10 {
          rect id="ra" w=(px)30 h=(px)20 fill=(token)"color.k"
          rect id="rb" w=(px)40 h=(px)20 fill=(token)"color.k"
        }
      }"#;
    let found = rects(body);
    assert_eq!(
        found,
        vec![(110.0, 110.0, 30.0, 20.0), (150.0, 110.0, 40.0, 20.0)]
    );
    let lowered = lowered_boxes(body);
    assert_eq!(lowered.get("row"), Some(&(110.0, 110.0, 80.0, 20.0)));
    assert_eq!(lowered.get("ra"), Some(&(110.0, 110.0, 30.0, 20.0)));
    assert_eq!(lowered.get("rb"), Some(&(150.0, 110.0, 40.0, 20.0)));
    let compiled = compiled_rects(body);
    assert_eq!(compiled.get("row"), Some(&(110.0, 110.0, 80.0, 20.0)));
    assert_eq!(compiled.get("rb"), Some(&(150.0, 110.0, 40.0, 20.0)));
}

#[test]
fn rotated_frame_pivots_on_its_box_and_translates_children() {
    let body =
        format!(r#"frame id="f" x=(px)10 y=(px)20 w=(px)100 h=(px)60 rotate=(deg)20 {{ {RECT} }}"#);
    let cmds = commands(&body);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            SceneCommand::PushTransform { angle_deg, cx, cy }
                if *angle_deg == 20.0 && *cx == 60.0 && *cy == 50.0
        )),
        "{cmds:?}"
    );
    assert_eq!(rects(&body), vec![(15.0, 26.0, 10.0, 10.0)]);
}

#[test]
fn page_anchor_inside_frames_stays_page_absolute() {
    // Page 400x300, rect 40x30 bottom-right: (360, 270) at any depth.
    let rect = r#"rect id="r" anchor="bottom-right" w=(px)40 h=(px)30 fill=(token)"color.k""#;
    let nested = format!(
        r#"frame id="f1" x=(px)15 y=(px)25 w=(px)385 h=(px)275 clip=#false {{
        frame id="f2" x=(px)7 y=(px)3 w=(px)300 h=(px)200 clip=#false {{ {rect} }}
      }}"#
    );
    assert_eq!(rects(&nested), vec![(360.0, 270.0, 40.0, 30.0)]);
}

#[test]
fn parent_anchor_inside_nested_frame_uses_its_page_box() {
    // f2's page box is (40, 60, 100, 100); a 20x20 center lands at (80, 100).
    let rect = r#"rect id="r" anchor="center" anchor-parent=#true w=(px)20 h=(px)20 fill=(token)"color.k""#;
    let body = format!(
        r#"frame id="f1" x=(px)10 y=(px)20 w=(px)300 h=(px)200 {{
        frame id="f2" x=(px)30 y=(px)40 w=(px)100 h=(px)100 {{ {rect} }}
      }}"#
    );
    assert_eq!(rects(&body), vec![(80.0, 100.0, 20.0, 20.0)]);
}

#[test]
fn connector_joins_targets_inside_two_frames() {
    // a: page (40, 40, 100, 80), right-mid (140, 80).
    // b: page (300, 60, 100, 80), left-mid (300, 100).
    let body = r#"frame id="fa" x=(px)20 y=(px)20 w=(px)200 h=(px)200 {
        rect id="a" x=(px)20 y=(px)20 w=(px)100 h=(px)80 fill=(token)"color.k"
      }
      frame id="fb" x=(px)250 y=(px)30 w=(px)150 h=(px)200 {
        rect id="b" x=(px)50 y=(px)30 w=(px)100 h=(px)80 fill=(token)"color.k"
      }
      connector id="c" from="a" to="b" stroke=(token)"color.line""#;
    let cmds = commands(body);
    let pts = cmds
        .iter()
        .find_map(|c| match c {
            SceneCommand::StrokePolyline { points, .. } => Some(points.clone()),
            _ => None,
        })
        .expect("connector stroke");
    assert_eq!(pts, vec![140.0, 80.0, 300.0, 100.0]);
}

#[test]
fn runaround_reads_an_exclusion_inside_a_frame_page_absolute() {
    // The framed exclusion covers page (0, 0, 200, 120), as the page-level one.
    let text = r#"text id="body" x=(px)0 y=(px)0 w=(px)400 h=(px)280 font-size=(px)20 text-exclusion="ex" fill=(token)"color.k" {
        span "The quick brown fox jumps over the lazy dog and then keeps running far beyond the box edge to force wrapping across many lines"
      }"#;
    let page_level = format!(
        r#"rect id="ex" x=(px)0 y=(px)0 w=(px)200 h=(px)120 fill=(token)"color.line"
      {text}"#
    );
    let framed = format!(
        r#"frame id="f" x=(px)100 y=(px)50 w=(px)300 h=(px)250 clip=#false {{
        rect id="ex" x=(px)-100 y=(px)-50 w=(px)200 h=(px)120 fill=(token)"color.line"
      }}
      {text}"#
    );
    // Lint advisories on the text do not matter here.
    let runs = |body: &str| {
        let result = compile(&parse(&doc(body)), &default_provider());
        glyph_run_positions(&result.scene.commands)
    };
    let page_runs = runs(&page_level);
    assert!(page_runs.iter().any(|&(x, _)| x >= 199.5), "{page_runs:?}");
    assert_eq!(runs(&framed), page_runs);
}

#[test]
fn compiled_boxes_are_page_absolute_inside_frames() {
    let body = format!(
        r#"group id="g" x=(px)100 y=(px)50 {{
        frame id="f" x=(px)10 y=(px)10 w=(px)100 h=(px)100 {{ {RECT} }}
      }}"#
    );
    let compiled = compiled_rects(&body);
    assert_eq!(compiled.get("f"), Some(&(110.0, 60.0, 100.0, 100.0)));
    assert_eq!(compiled.get("r"), Some(&(115.0, 66.0, 10.0, 10.0)));
}
