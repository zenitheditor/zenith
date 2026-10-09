//! Layout frames placed by an anchor: measured first, anchored at the
//! measured size, then arranged; anchors that depend on laid-out nodes
//! resolve in dependency order.

mod common;
use std::collections::BTreeMap;

use common::parse;
#[path = "common/fill_rects.rs"]
mod fill_rects;
use fill_rects::fill_rects;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::layout_boxes;

fn doc(children: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.la" name="LA"
  tokens format="zenith-token-v1" {{
    token id="color.k" type="color" value="#000000"
  }}
  styles {{}}
  document id="doc.la" title="LA" {{
    page id="p" w=(px)800 h=(px)600 {{
{children}
    }}
  }}
}}
"##
    )
}

fn boxes(src: &str) -> BTreeMap<String, (f64, f64, f64, f64)> {
    layout_boxes(&parse(src), 0, &default_provider())
        .into_iter()
        .map(|(k, b)| (k, (b.x, b.y, b.w, b.h)))
        .collect()
}

const CARD: &str = r#"layout="column" padding=(px)10 {
        rect id="r" w=(px)100 h=(px)50 fill=(token)"color.k"
      }"#;

#[test]
fn hugging_root_centers_on_the_page() {
    let src = doc(&format!(r#"      frame id="card" anchor="center" {CARD}"#));
    let b = boxes(&src);
    // 120x70 card centered on 800x600.
    assert_eq!(b.get("card"), Some(&(340.0, 265.0, 120.0, 70.0)));
    assert_eq!(b.get("r"), Some(&(350.0, 275.0, 100.0, 50.0)));
    let result = compile(&parse(&src), &default_provider());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(fill_rects(&result), vec![(350.0, 275.0, 100.0, 50.0)]);
}

#[test]
fn hugging_root_centers_in_its_parent() {
    let src = doc(&format!(
        r#"      frame id="host" x=(px)200 y=(px)100 w=(px)400 h=(px)300 {{
      frame id="card" anchor="center" anchor-parent=#true {CARD}
      }}"#
    ));
    let b = boxes(&src);
    assert_eq!(b.get("card"), Some(&(340.0, 215.0, 120.0, 70.0)));
    let result = compile(&parse(&src), &default_provider());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(fill_rects(&result).contains(&(350.0, 225.0, 100.0, 50.0)));
}

#[test]
fn anchored_root_inside_a_group_counts_the_group_origin() {
    let src = doc(&format!(
        r#"      group id="g" x=(px)100 y=(px)40 w=(px)400 h=(px)200 {{
      frame id="card" anchor="bottom-right" anchor-parent=#true {CARD}
      }}"#
    ));
    // Group box spans x 100..500, y 40..240; the card's bottom-right corner
    // sits on it.
    let b = boxes(&src);
    assert_eq!(b.get("card"), Some(&(380.0, 170.0, 120.0, 70.0)));
}

#[test]
fn anchor_chain_resolves_after_the_laid_out_target() {
    // `late` anchors to `mark`, which anchors below `col`: `late` comes
    // first in source order, so it waits for both.
    let src = doc(
        r#"      frame id="late" anchor-sibling="mark" anchor-edge="after" anchor-gap=(px)5 layout="row" {
        rect id="lr" w=(px)30 h=(px)30 fill=(token)"color.k"
      }
      frame id="col" x=(px)50 y=(px)60 layout="column" gap=(px)4 {
        rect id="c1" w=(px)80 h=(px)20 fill=(token)"color.k"
        rect id="c2" w=(px)80 h=(px)20 fill=(token)"color.k"
      }
      rect id="mark" anchor-sibling="col" anchor-edge="below" anchor-gap=(px)10 w=(px)40 h=(px)40 fill=(token)"color.k""#,
    );
    let b = boxes(&src);
    // col: 80x44 at (50, 60); mark: (50, 114) 40x40; late: right of mark.
    assert_eq!(b.get("col"), Some(&(50.0, 60.0, 80.0, 44.0)));
    assert_eq!(b.get("late"), Some(&(95.0, 114.0, 30.0, 30.0)));
    let result = compile(&parse(&src), &default_provider());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let rects = fill_rects(&result);
    assert!(rects.contains(&(50.0, 114.0, 40.0, 40.0)), "{rects:?}");
    assert!(rects.contains(&(95.0, 114.0, 30.0, 30.0)), "{rects:?}");
}

#[test]
fn anchored_root_follows_a_sibling_root() {
    let src = doc(
        r#"      frame id="a" x=(px)10 y=(px)20 layout="row" padding=(px)5 {
        rect id="ar" w=(px)50 h=(px)30 fill=(token)"color.k"
      }
      frame id="b" anchor-sibling="a" anchor-edge="below" anchor-gap=(px)8 anchor="top-center" layout="row" {
        rect id="br" w=(px)20 h=(px)10 fill=(token)"color.k"
      }"#,
    );
    let b = boxes(&src);
    // a: 60x40 at (10, 20); b: 20x10 centered under it.
    assert_eq!(b.get("b"), Some(&(30.0, 68.0, 20.0, 10.0)));
}

#[test]
fn anchor_cycle_leaves_the_roots_unresolved() {
    let src = doc(
        r#"      frame id="a" anchor-sibling="b" anchor-edge="below" layout="row" {
        rect id="ar" w=(px)10 h=(px)10 fill=(token)"color.k"
      }
      frame id="b" anchor-sibling="a" anchor-edge="below" layout="row" {
        rect id="br" w=(px)10 h=(px)10 fill=(token)"color.k"
      }"#,
    );
    let b = boxes(&src);
    assert!(!b.contains_key("a") && !b.contains_key("b"), "{b:?}");
}
