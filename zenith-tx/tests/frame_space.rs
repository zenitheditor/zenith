//! Ops across container spaces: `reparent`, `ungroup`, `align_nodes`,
//! `distribute_nodes`, `align_to_edge`, `snap_path_anchors`, and
//! `path_boolean` keep or compute page positions when frames and groups
//! translate their children. Compiled page boxes from `zenith-scene` check
//! the visual position.

mod common;
use std::collections::BTreeMap;

use common::parse;
#[path = "common/px_attr.rs"]
mod px_attr;
use px_attr::extract_px_attr;
use zenith_core::{Document, Node, PathNode, default_provider};
use zenith_scene::{DocumentPrep, PageCompiler};
use zenith_tx::op::OpPathBooleanOperation;
use zenith_tx::{Op, Permissions, Position, Transaction, TxResult, TxStatus, run_transaction};

fn doc(body: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {{
    token id="color.k" type="color" value="#000000"
  }}
  styles {{ }}
  document id="doc1" title="T" {{
    page id="pg1" w=(px)400 h=(px)300 {{
      {body}
    }}
  }}
}}"##
    )
}

fn run(source: &str, ops: Vec<Op>) -> TxResult {
    let tx = Transaction {
        ops,
        permissions: Permissions::default(),
    };
    run_transaction(&parse(source), &tx).expect("run_transaction must not error")
}

fn accepted(r: &TxResult) {
    assert!(
        matches!(
            r.status,
            TxStatus::Accepted | TxStatus::AcceptedWithWarnings
        ),
        "expected accepted; got {:?}: {:?}",
        r.status,
        r.diagnostics
    );
}

fn has_code(r: &TxResult, code: &str) -> bool {
    r.diagnostics.iter().any(|d| d.code == code)
}

/// Compiled page-space boxes `(x, y, w, h)` of page 0.
fn boxes(source: &str) -> BTreeMap<String, (f64, f64, f64, f64)> {
    let doc = parse(source);
    let prep = DocumentPrep::new(&doc, None, None);
    let fonts = default_provider();
    PageCompiler::new(&prep, &fonts)
        .compiled_boxes(0)
        .into_iter()
        .map(|(k, b)| (k, (b.rect.x, b.rect.y, b.rect.w, b.rect.h)))
        .collect()
}

fn box_of(source: &str, id: &str) -> (f64, f64, f64, f64) {
    *boxes(source)
        .get(id)
        .unwrap_or_else(|| panic!("no compiled box for {id}"))
}

fn reparent(node: &str, new_parent: &str) -> Op {
    Op::Reparent {
        node: node.to_owned(),
        new_parent: new_parent.to_owned(),
        position: Position::Last,
    }
}

fn xy(source: &str, id: &str) -> (Option<f64>, Option<f64>) {
    (
        extract_px_attr(source, id, "x"),
        extract_px_attr(source, id, "y"),
    )
}

// ── reparent ─────────────────────────────────────────────────────────────────

const NESTED: &str = r#"frame id="card" x=(px)100 y=(px)50 w=(px)200 h=(px)200 clip=#false {
        group id="cluster" x=(px)20 y=(px)30 {
          rect id="dot" x=(px)0 y=(px)0 w=(px)4 h=(px)4 fill=(token)"color.k"
        }
      }
      rect id="r" x=(px)140 y=(px)110 w=(px)30 h=(px)20 fill=(token)"color.k""#;

#[test]
fn reparent_round_trip_keeps_page_box() {
    let start = doc(NESTED);
    let page_box = box_of(&start, "r");
    assert_eq!(page_box, (140.0, 110.0, 30.0, 20.0));

    let in_frame = run(&start, vec![reparent("r", "card")]);
    accepted(&in_frame);
    assert_eq!(xy(&in_frame.source_after, "r"), (Some(40.0), Some(60.0)));
    assert_eq!(box_of(&in_frame.source_after, "r"), page_box);

    let in_group = run(&in_frame.source_after, vec![reparent("r", "cluster")]);
    accepted(&in_group);
    assert_eq!(xy(&in_group.source_after, "r"), (Some(20.0), Some(30.0)));
    assert_eq!(box_of(&in_group.source_after, "r"), page_box);

    let on_page = run(&in_group.source_after, vec![reparent("r", "pg1")]);
    accepted(&on_page);
    assert_eq!(xy(&on_page.source_after, "r"), (Some(140.0), Some(110.0)));
    assert_eq!(box_of(&on_page.source_after, "r"), page_box);
    assert!(!has_code(&on_page, "tx.coordinate_unresolved"));
}

#[test]
fn reparent_into_layout_flow_slot_keeps_values() {
    let start = doc(
        r#"frame id="col" x=(px)10 y=(px)10 w=(px)200 h=(px)280 layout="column" gap=(px)8 {
        rect id="c1" w=(px)50 h=(px)20 fill=(token)"color.k"
      }
      rect id="r" x=(px)300 y=(px)200 w=(px)30 h=(px)20 fill=(token)"color.k""#,
    );
    let r = run(&start, vec![reparent("r", "col")]);
    accepted(&r);
    assert!(
        !has_code(&r, "tx.coordinate_unresolved"),
        "{:?}",
        r.diagnostics
    );
    assert_eq!(xy(&r.source_after, "r"), (Some(300.0), Some(200.0)));
}

const SLOT: &str = r#"frame id="col" x=(px)10 y=(px)10 w=(px)200 h=(px)280 layout="column" {
        frame id="slot" w=(px)150 h=(px)150 clip=#false {
          group id="ga" x=(px)4 y=(px)0 {
            rect id="inner" x=(px)1 y=(px)1 w=(px)5 h=(px)5 fill=(token)"color.k"
          }
          group id="gb" x=(px)0 y=(px)9 {
            rect id="anchor" x=(px)0 y=(px)0 w=(px)5 h=(px)5 fill=(token)"color.k"
          }
        }
      }
      rect id="r" x=(px)300 y=(px)200 w=(px)30 h=(px)20 fill=(token)"color.k""#;

#[test]
fn reparent_into_unresolved_origin_emits_advisory() {
    let r = run(&doc(SLOT), vec![reparent("r", "slot")]);
    accepted(&r);
    let d = r
        .diagnostics
        .iter()
        .find(|d| d.code == "tx.coordinate_unresolved")
        .unwrap_or_else(|| panic!("expected tx.coordinate_unresolved; got {:?}", r.diagnostics));
    assert!(d.message.contains("\"r\"") && d.message.contains("\"slot\""));
    assert_eq!(xy(&r.source_after, "r"), (Some(300.0), Some(200.0)));
}

#[test]
fn reparent_below_shared_unresolved_container_converts() {
    let start = doc(SLOT);
    let page_box = box_of(&start, "inner");
    let r = run(&start, vec![reparent("inner", "gb")]);
    accepted(&r);
    assert!(
        !has_code(&r, "tx.coordinate_unresolved"),
        "{:?}",
        r.diagnostics
    );
    assert_eq!(xy(&r.source_after, "inner"), (Some(5.0), Some(-8.0)));
    assert_eq!(box_of(&r.source_after, "inner"), page_box);
}

// ── ungroup ──────────────────────────────────────────────────────────────────

#[test]
fn ungroup_in_frame_keeps_page_box() {
    let start = doc(
        r#"frame id="card" x=(px)100 y=(px)50 w=(px)200 h=(px)200 clip=#false {
        group id="g" x=(px)20 y=(px)30 {
          rect id="a" x=(px)5 y=(px)5 w=(px)10 h=(px)10 fill=(token)"color.k"
        }
      }"#,
    );
    let page_box = box_of(&start, "a");
    assert_eq!(page_box, (125.0, 85.0, 10.0, 10.0));
    let r = run(
        &start,
        vec![Op::Ungroup {
            group_id: "g".to_owned(),
        }],
    );
    accepted(&r);
    assert_eq!(xy(&r.source_after, "a"), (Some(25.0), Some(35.0)));
    assert_eq!(box_of(&r.source_after, "a"), page_box);
}

#[test]
fn ungroup_rotated_group_emits_advisory() {
    let r = run(
        &doc(r#"group id="g" x=(px)20 y=(px)30 rotate=(deg)30 {
        rect id="a" x=(px)5 y=(px)5 w=(px)10 h=(px)10 fill=(token)"color.k"
      }"#),
        vec![Op::Ungroup {
            group_id: "g".to_owned(),
        }],
    );
    accepted(&r);
    assert!(
        has_code(&r, "tx.coordinate_unresolved"),
        "{:?}",
        r.diagnostics
    );
    assert_eq!(xy(&r.source_after, "a"), (Some(5.0), Some(5.0)));
}

// ── align / distribute / align_to_edge ───────────────────────────────────────

const TWO_FRAMES: &str = r#"frame id="fa" x=(px)0 y=(px)0 w=(px)200 h=(px)200 clip=#false {
        rect id="a" x=(px)120 y=(px)10 w=(px)50 h=(px)20 fill=(token)"color.k"
      }
      frame id="fb" x=(px)100 y=(px)150 w=(px)200 h=(px)100 clip=#false {
        rect id="b" x=(px)50 y=(px)10 w=(px)50 h=(px)20 fill=(token)"color.k"
      }"#;

#[test]
fn align_left_across_frames_uses_page_space() {
    let r = run(
        &doc(TWO_FRAMES),
        vec![Op::AlignNodes {
            node_ids: vec!["a".to_owned(), "b".to_owned()],
            align: "left".to_owned(),
            anchor: "selection".to_owned(),
        }],
    );
    accepted(&r);
    assert_eq!(extract_px_attr(&r.source_after, "a", "x"), Some(120.0));
    assert_eq!(extract_px_attr(&r.source_after, "b", "x"), Some(20.0));
    assert_eq!(box_of(&r.source_after, "a").0, 120.0);
    assert_eq!(box_of(&r.source_after, "b").0, 120.0);
}

#[test]
fn align_to_page_dimension_anchor_is_page_space() {
    let r = run(
        &doc(TWO_FRAMES),
        vec![Op::AlignNodes {
            node_ids: vec!["b".to_owned()],
            align: "top".to_owned(),
            anchor: "(px)200".to_owned(),
        }],
    );
    accepted(&r);
    assert_eq!(extract_px_attr(&r.source_after, "b", "y"), Some(50.0));
    assert_eq!(box_of(&r.source_after, "b").1, 200.0);
}

#[test]
fn distribute_across_frames_uses_page_space() {
    let start = doc(
        r#"frame id="fa" x=(px)0 y=(px)0 w=(px)100 h=(px)100 clip=#false {
        rect id="a" x=(px)10 y=(px)10 w=(px)20 h=(px)20 fill=(token)"color.k"
      }
      frame id="fb" x=(px)30 y=(px)120 w=(px)100 h=(px)100 clip=#false {
        rect id="b" x=(px)10 y=(px)10 w=(px)20 h=(px)20 fill=(token)"color.k"
      }
      rect id="c" x=(px)110 y=(px)10 w=(px)20 h=(px)20 fill=(token)"color.k""#,
    );
    let r = run(
        &start,
        vec![Op::DistributeNodes {
            node_ids: vec!["a".to_owned(), "b".to_owned(), "c".to_owned()],
            axis: "horizontal".to_owned(),
        }],
    );
    accepted(&r);
    // Page lefts 10, 40, 110 with width 20: the equal gap is 30, so b moves
    // to page x 60 (frame-local 30).
    assert_eq!(extract_px_attr(&r.source_after, "b", "x"), Some(30.0));
    let after = boxes(&r.source_after);
    assert_eq!(after.get("a").map(|b| b.0), Some(10.0));
    assert_eq!(after.get("b").map(|b| b.0), Some(60.0));
    assert_eq!(after.get("c").map(|b| b.0), Some(110.0));
}

#[test]
fn align_to_edge_in_frame_uses_page_bounds() {
    let r = run(
        &doc(TWO_FRAMES),
        vec![Op::AlignToEdge {
            node: "b".to_owned(),
            edge: "right".to_owned(),
            margin: 10.0,
        }],
    );
    accepted(&r);
    // Page right edge 400 - margin 10 - width 50 = page x 340, frame-local 240.
    assert_eq!(extract_px_attr(&r.source_after, "b", "x"), Some(240.0));
    assert_eq!(box_of(&r.source_after, "b").0, 340.0);
}

#[test]
fn align_skips_node_in_unresolved_container() {
    let r = run(
        &doc(SLOT),
        vec![Op::AlignNodes {
            node_ids: vec!["inner".to_owned(), "r".to_owned()],
            align: "left".to_owned(),
            anchor: "selection".to_owned(),
        }],
    );
    let d = r
        .diagnostics
        .iter()
        .find(|d| d.code == "tx.geometry_unresolved")
        .unwrap_or_else(|| panic!("expected tx.geometry_unresolved; got {:?}", r.diagnostics));
    assert!(d.message.contains("\"slot\""), "{}", d.message);
}

#[test]
fn align_inside_shared_unresolved_container_works() {
    let r = run(
        &doc(SLOT),
        vec![Op::AlignNodes {
            node_ids: vec!["inner".to_owned(), "anchor".to_owned()],
            align: "left".to_owned(),
            anchor: "selection".to_owned(),
        }],
    );
    accepted(&r);
    // Slot-space lefts: inner 4 + 1 = 5, anchor 0. Both move to 0.
    assert_eq!(extract_px_attr(&r.source_after, "inner", "x"), Some(-4.0));
    assert_eq!(extract_px_attr(&r.source_after, "anchor", "x"), Some(0.0));
}

// ── path ops ─────────────────────────────────────────────────────────────────

fn find_path<'d>(nodes: &'d [Node], id: &str) -> Option<&'d PathNode> {
    for n in nodes {
        if let Node::Path(p) = n
            && p.id == id
        {
            return Some(p);
        }
        if let Some(found) = n.children().and_then(|c| find_path(c, id)) {
            return Some(found);
        }
    }
    None
}

fn path_points(source: &str, id: &str) -> Vec<(f64, f64)> {
    let doc: Document = parse(source);
    let page = doc.body.pages.first().expect("page");
    let path = find_path(&page.children, id).unwrap_or_else(|| panic!("no path {id}"));
    path.anchors
        .iter()
        .map(|a| {
            (
                a.x.as_ref().expect("x").value,
                a.y.as_ref().expect("y").value,
            )
        })
        .collect()
}

#[test]
fn snap_path_anchors_across_frames() {
    let r = run(
        &doc(
            r#"frame id="fs" x=(px)100 y=(px)100 w=(px)100 h=(px)100 clip=#false {
        path id="source" {
          anchor x=(px)0 y=(px)0
          anchor x=(px)10 y=(px)0
        }
      }
      path id="target" {
        anchor x=(px)113 y=(px)102
        anchor x=(px)113 y=(px)112
      }"#,
        ),
        vec![Op::SnapPathAnchors {
            node: "source".to_owned(),
            target: "target".to_owned(),
            tolerance: 4.0,
        }],
    );
    accepted(&r);
    // The source end (page 110, 100) lands on the target at page (113, 102).
    let pts = path_points(&r.source_after, "source");
    assert_eq!(pts.len(), 2);
    let close = |a: f64, b: f64| (a - b).abs() <= 1.0e-9;
    assert!(close(pts[0].0, 3.0) && close(pts[0].1, 2.0), "{pts:?}");
    assert!(close(pts[1].0, 13.0) && close(pts[1].1, 2.0), "{pts:?}");
}

#[test]
fn path_boolean_across_frames() {
    let r = run(
        &doc(
            r#"frame id="fs" x=(px)100 y=(px)100 w=(px)100 h=(px)100 clip=#false {
        path id="source" closed=#true {
          anchor x=(px)0 y=(px)0
          anchor x=(px)40 y=(px)0
          anchor x=(px)40 y=(px)40
          anchor x=(px)0 y=(px)40
        }
      }
      path id="target" closed=#true {
        anchor x=(px)120 y=(px)90
        anchor x=(px)160 y=(px)90
        anchor x=(px)160 y=(px)130
        anchor x=(px)120 y=(px)130
      }"#,
        ),
        vec![Op::PathBoolean {
            node: "source".to_owned(),
            target: "target".to_owned(),
            new_id: "both".to_owned(),
            operation: OpPathBooleanOperation::Intersect,
            tolerance: 0.5,
        }],
    );
    accepted(&r);
    // The overlap is page x 120..140, y 100..130: frame-local x 20..40, y 0..30.
    let pts = path_points(&r.source_after, "both");
    let min_x = pts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let max_x = pts.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    let min_y = pts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let max_y = pts.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
    assert_eq!((min_x, max_x, min_y, max_y), (20.0, 40.0, 0.0, 30.0));
}

#[test]
fn path_snap_with_unresolved_container_is_rejected() {
    let r = run(
        &doc(
            r#"frame id="col" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="column" {
        frame id="slot" w=(px)100 h=(px)100 {
          path id="source" {
            anchor x=(px)0 y=(px)0
            anchor x=(px)10 y=(px)0
          }
        }
      }
      path id="target" {
        anchor x=(px)13 y=(px)2
        anchor x=(px)13 y=(px)12
      }"#,
        ),
        vec![Op::SnapPathAnchors {
            node: "source".to_owned(),
            target: "target".to_owned(),
            tolerance: 4.0,
        }],
    );
    assert_eq!(r.status, TxStatus::Rejected, "{:?}", r.diagnostics);
    assert!(has_code(&r, "tx.invalid_geometry"), "{:?}", r.diagnostics);
}

// ── masters ──────────────────────────────────────────────────────────────────

const MASTER_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.k" type="color" value="#000000"
  }
  styles { }
  masters {
    master id="m.deck" {
      frame id="band" x=(px)20 y=(px)500 w=(px)760 h=(px)80 clip=#false {
        group id="mark" x=(px)10 y=(px)10 {
          rect id="dot" x=(px)5 y=(px)5 w=(px)4 h=(px)4 fill=(token)"color.k"
        }
      }
      rect id="logo" x=(px)700 y=(px)530 w=(px)40 h=(px)20 fill=(token)"color.k"
    }
  }
  document id="doc1" title="T" {
    page id="pg1" w=(px)800 h=(px)600 master="m.deck" {
      rect id="body" x=(px)40 y=(px)40 w=(px)100 h=(px)100 fill=(token)"color.k"
    }
  }
}"##;

#[test]
fn reparent_inside_master_converts() {
    let r = run(MASTER_DOC, vec![reparent("logo", "band")]);
    accepted(&r);
    assert!(
        !has_code(&r, "tx.coordinate_unresolved"),
        "{:?}",
        r.diagnostics
    );
    assert_eq!(xy(&r.source_after, "logo"), (Some(680.0), Some(30.0)));
    let back = run(&r.source_after, vec![reparent("logo", "m.deck")]);
    accepted(&back);
    assert_eq!(xy(&back.source_after, "logo"), (Some(700.0), Some(530.0)));
}

#[test]
fn ungroup_inside_master_shifts_children() {
    let r = run(
        MASTER_DOC,
        vec![Op::Ungroup {
            group_id: "mark".to_owned(),
        }],
    );
    accepted(&r);
    assert!(
        !r.source_after.contains("id=\"mark\""),
        "{}",
        r.source_after
    );
    assert_eq!(xy(&r.source_after, "dot"), (Some(15.0), Some(15.0)));
}

// ── anchored containers ──────────────────────────────────────────────────────

#[test]
fn anchored_group_translates_by_authored_origin() {
    // The scene ignores a group anchor for the child space: absent x/y is 0.
    let start = doc(r#"group id="g" anchor="bottom-right" w=(px)50 h=(px)50 {
        rect id="a" x=(px)30 y=(px)40 w=(px)10 h=(px)10 fill=(token)"color.k"
      }"#);
    let page_box = box_of(&start, "a");
    let r = run(&start, vec![reparent("a", "pg1")]);
    accepted(&r);
    assert!(
        !has_code(&r, "tx.coordinate_unresolved"),
        "{:?}",
        r.diagnostics
    );
    assert_eq!(box_of(&r.source_after, "a"), page_box);
}

/// Anchored layout frames whose origin derives from authored data.
const ANCHORED_FRAMES: &str = r#"safe-zone id="sz" type="required" x=(px)100 y=(px)50 w=(px)200 h=(px)100
      frame id="br" anchor="bottom-right" w=(px)120 h=(px)60 layout="column" {
        rect id="br_item" w=(px)20 h=(px)20 fill=(token)"color.k"
      }
      frame id="zoned" anchor="center" anchor-zone="sz" w=(px)80 h=(px)40 layout="row" {
        rect id="zoned_item" w=(px)20 h=(px)20 fill=(token)"color.k"
      }
      rect id="base" x=(px)10 y=(px)10 w=(px)60 h=(px)30 fill=(token)"color.k"
      frame id="next" anchor-sibling="base" anchor-edge="below" anchor-gap=(px)8 w=(px)60 h=(px)40 layout="column" {
        rect id="next_item" w=(px)20 h=(px)20 fill=(token)"color.k"
      }
      frame id="hug" anchor="top-right" layout="column" {
        rect id="hug_item" w=(px)20 h=(px)20 fill=(token)"color.k"
      }
      rect id="r" x=(px)300 y=(px)260 w=(px)10 h=(px)10 fill=(token)"color.k" position="absolute""#;

#[test]
fn reparent_into_anchored_layout_frames_keeps_page_box() {
    let start = doc(ANCHORED_FRAMES);
    let page_box = box_of(&start, "r");
    for frame in ["br", "zoned", "next"] {
        let r = run(&start, vec![reparent("r", frame)]);
        accepted(&r);
        assert!(
            !has_code(&r, "tx.coordinate_unresolved"),
            "{frame}: {:?}",
            r.diagnostics
        );
        assert_eq!(box_of(&r.source_after, "r"), page_box, "{frame}");
    }
}

#[test]
fn reparent_into_frame_anchored_to_anchored_layout_frame_keeps_page_box() {
    // `above` sits on `br`, a fixed-size layout frame placed by its anchor.
    let start = doc(&format!(
        r#"{ANCHORED_FRAMES}
      frame id="above" anchor-sibling="br" anchor-edge="above" anchor="top-right" anchor-gap=(px)4 w=(px)50 h=(px)30 layout="row" {{
        rect id="above_item" w=(px)20 h=(px)20 fill=(token)"color.k"
      }}"#
    ));
    let page_box = box_of(&start, "r");
    let r = run(&start, vec![reparent("r", "above")]);
    accepted(&r);
    assert!(
        !has_code(&r, "tx.coordinate_unresolved"),
        "{:?}",
        r.diagnostics
    );
    assert_eq!(box_of(&r.source_after, "r"), page_box);
}

#[test]
fn reparent_into_hugging_anchored_frame_emits_advisory() {
    let r = run(&doc(ANCHORED_FRAMES), vec![reparent("r", "hug")]);
    accepted(&r);
    assert!(
        has_code(&r, "tx.coordinate_unresolved"),
        "{:?}",
        r.diagnostics
    );
    assert_eq!(xy(&r.source_after, "r"), (Some(300.0), Some(260.0)));
}
