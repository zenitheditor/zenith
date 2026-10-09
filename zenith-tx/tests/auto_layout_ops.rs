//! Auto-layout transaction ops: `set_layout` (set, clear, invalid kind and
//! value), `set_geometry` `hug` / `fill` sizes, and the `tx.layout_managed`
//! guard on every hand-placement op.

mod common;
use common::parse;
use zenith_core::Severity;
use zenith_scene::layout_boxes;
use zenith_tx::{LayoutEdit, Op, Permissions, Transaction, TxResult, TxStatus, run_transaction};

/// A page with a row frame `chips` (two in-flow chips and one absolute
/// child), a grid frame `tiles`, a free rect, and a line.
const DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.k" type="color" value="#000000"
    token id="space.md" type="dimension" value=(px)16
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)800 h=(px)600 {
      frame id="chips" x=(px)10 y=(px)20 w=(px)400 h=(px)60 layout="row" gap=(px)10 {
        rect id="a" w=(px)40 h=(px)20 fill=(token)"color.k"
        rect id="b" w=(px)60 h=(px)30 fill=(token)"color.k"
        rect id="abs" x=(px)5 y=(px)5 w=(px)10 h=(px)10 fill=(token)"color.k" position="absolute"
      }
      frame id="tiles" x=(px)10 y=(px)200 w=(px)200 h=(px)100 layout="grid" columns=2 {
        rect id="t1" fill=(token)"color.k"
        rect id="t2" fill=(token)"color.k"
      }
      rect id="free" x=(px)500 y=(px)400 w=(px)50 h=(px)50 fill=(token)"color.k"
      line id="rule" x1=(px)0 y1=(px)590 x2=(px)800 y2=(px)590 stroke=(token)"color.k"
    }
  }
}"##;

fn run(ops: Vec<Op>) -> TxResult {
    let tx = Transaction {
        ops,
        permissions: Permissions::default(),
    };
    run_transaction(&parse(DOC), &tx).expect("run_transaction must not error")
}

fn run_json(json: &str) -> TxResult {
    let tx = Transaction::from_json(json).expect("transaction JSON parses");
    run_transaction(&parse(DOC), &tx).expect("run_transaction must not error")
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

/// Assert the op was rejected with `code` and left the source unchanged.
fn rejected_with<'r>(r: &'r TxResult, code: &str) -> &'r zenith_core::Diagnostic {
    assert_eq!(r.status, TxStatus::Rejected, "{:?}", r.diagnostics);
    assert_eq!(
        r.source_after, r.source_before,
        "rejected tx must not change source"
    );
    r.diagnostics
        .iter()
        .find(|d| d.code == code)
        .unwrap_or_else(|| panic!("expected {code}; got {:?}", r.diagnostics))
}

/// The source line holding `id="<id>"`.
fn line_of<'s>(src: &'s str, id: &str) -> &'s str {
    let needle = format!("id=\"{id}\"");
    src.lines()
        .find(|l| l.contains(&needle))
        .unwrap_or_else(|| panic!("no line for {id}"))
}

fn edit(node: &str) -> LayoutEdit {
    LayoutEdit {
        node: node.to_owned(),
        ..LayoutEdit::default()
    }
}

// ── set_layout: set ──────────────────────────────────────────────────────────

#[test]
fn set_layout_sets_container_fields_and_shows_the_diff() {
    let r = run(vec![Op::SetLayout(LayoutEdit {
        layout: Some(Some("column".into())),
        gap: Some(Some(12.0.into())),
        padding: Some(Some("space.md".into())),
        padding_left: Some(Some("(pt)6".into())),
        justify: Some(Some("center".into())),
        align: Some(Some("start".into())),
        wrap: Some(Some(true)),
        clip: Some(Some(false)),
        ..edit("chips")
    })]);
    accepted(&r);
    assert_eq!(r.affected_node_ids, vec!["chips".to_owned()]);
    let before = line_of(&r.source_before, "chips");
    let after = line_of(&r.source_after, "chips");
    assert!(before.contains("layout=\"row\""), "{before}");
    for want in [
        "layout=\"column\"",
        "gap=(px)12",
        "padding=(token)\"space.md\"",
        "padding-left=(pt)6",
        "justify=\"center\"",
        "align=\"start\"",
        "wrap=#true",
        "clip=#false",
    ] {
        assert!(after.contains(want), "missing {want} in {after}");
    }
    // Only the frame line changes in the diff.
    let changed: Vec<(&str, &str)> = r
        .source_before
        .lines()
        .zip(r.source_after.lines())
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(changed.len(), 1, "{changed:?}");
}

#[test]
fn set_layout_result_lays_out_as_a_column() {
    let r = run(vec![Op::SetLayout(LayoutEdit {
        layout: Some(Some("column".into())),
        align: Some(Some("start".into())),
        ..edit("chips")
    })]);
    accepted(&r);
    let boxes = layout_boxes(&parse(&r.source_after), 0, &zenith_core::default_provider());
    let a = boxes.get("a").expect("box a");
    let b = boxes.get("b").expect("box b");
    assert_eq!((a.x, a.y, a.w, a.h), (10.0, 20.0, 40.0, 20.0));
    assert_eq!((b.x, b.y, b.w, b.h), (10.0, 50.0, 60.0, 30.0));
}

#[test]
fn set_layout_sets_item_fields_on_a_box_node() {
    let r = run(vec![Op::SetLayout(LayoutEdit {
        min_w: Some(Some(30.0.into())),
        max_w: Some(Some("(px)90".into())),
        min_h: Some(Some("(token)space.md".into())),
        max_h: Some(Some(40.0.into())),
        ..edit("a")
    })]);
    accepted(&r);
    let after = line_of(&r.source_after, "a");
    for want in [
        "min-w=(px)30",
        "max-w=(px)90",
        "min-h=(token)\"space.md\"",
        "max-h=(px)40",
    ] {
        assert!(after.contains(want), "missing {want} in {after}");
    }
}

// ── set_layout: clear ────────────────────────────────────────────────────────

#[test]
fn set_layout_null_clears_fields() {
    let r = run_json(
        r#"{"ops":[
            {"op":"set_layout","node":"chips","gap":null},
            {"op":"set_layout","node":"abs","position":null}
        ]}"#,
    );
    accepted(&r);
    let chips = line_of(&r.source_after, "chips");
    assert!(!chips.contains("gap="), "{chips}");
    assert!(chips.contains("layout=\"row\""), "{chips}");
    let abs = line_of(&r.source_after, "abs");
    assert!(!abs.contains("position="), "{abs}");
}

#[test]
fn set_layout_clearing_layout_is_rejected_when_children_lose_placement() {
    let r = run_json(r#"{"ops":[{"op":"set_layout","node":"chips","layout":null}]}"#);
    rejected_with(&r, "node.missing_geometry");
}

#[test]
fn set_layout_absent_fields_stay_unchanged() {
    let r = run_json(r#"{"ops":[{"op":"set_layout","node":"chips","justify":"end"}]}"#);
    accepted(&r);
    let chips = line_of(&r.source_after, "chips");
    assert!(chips.contains("gap=(px)10"), "{chips}");
    assert!(chips.contains("layout=\"row\""), "{chips}");
    assert!(chips.contains("justify=\"end\""), "{chips}");
}

#[test]
fn set_layout_without_fields_is_a_noop() {
    let r = run(vec![Op::SetLayout(edit("chips"))]);
    assert_eq!(r.source_after, r.source_before);
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.code == "tx.noop" && d.severity == Severity::Advisory),
        "{:?}",
        r.diagnostics
    );
}

// ── set_layout: invalid kind / value ─────────────────────────────────────────

#[test]
fn set_layout_container_field_on_a_rect_is_wrong_node_type() {
    let r = run(vec![Op::SetLayout(LayoutEdit {
        gap: Some(Some(8.0.into())),
        justify: Some(Some("center".into())),
        ..edit("free")
    })]);
    let d = rejected_with(&r, "tx.wrong_node_type");
    assert!(d.message.contains("gap, justify"), "{}", d.message);
    assert!(d.message.contains("\"free\""), "{}", d.message);
    assert!(d.message.contains("rect"), "{}", d.message);
}

#[test]
fn set_layout_item_field_on_a_line_is_unsupported() {
    let r = run(vec![Op::SetLayout(LayoutEdit {
        position: Some(Some("absolute".into())),
        ..edit("rule")
    })]);
    let d = rejected_with(&r, "tx.unsupported_property");
    assert!(d.message.contains("position"), "{}", d.message);
    assert!(d.message.contains("line"), "{}", d.message);
}

#[test]
fn set_layout_unknown_node_is_rejected() {
    let r = run(vec![Op::SetLayout(LayoutEdit {
        gap: Some(Some(8.0.into())),
        ..edit("ghost")
    })]);
    rejected_with(&r, "tx.unknown_node");
}

#[test]
fn set_layout_rejects_unknown_enum_values() {
    for (field, json) in [
        ("layout", r#""flow""#),
        ("justify", r#""middle""#),
        ("align", r#""baseline""#),
    ] {
        let tx = format!(r#"{{"ops":[{{"op":"set_layout","node":"chips","{field}":{json}}}]}}"#);
        let r = run_json(&tx);
        let d = rejected_with(&r, "tx.invalid_value");
        assert!(d.message.contains(field), "{}", d.message);
        assert!(d.message.contains(json), "{}", d.message);
        assert!(d.message.contains("use one of"), "{}", d.message);
    }
    let r = run_json(r#"{"ops":[{"op":"set_layout","node":"a","position":"fixed"}]}"#);
    let d = rejected_with(&r, "tx.invalid_value");
    assert!(d.message.contains("auto, absolute"), "{}", d.message);
}

#[test]
fn set_layout_rejects_bad_dimensions() {
    for json in [r#""(pct)50""#, r#""(px)abc""#, r#""two words""#, r#""""#] {
        let tx = format!(r#"{{"ops":[{{"op":"set_layout","node":"chips","gap":{json}}}]}}"#);
        let r = run_json(&tx);
        let d = rejected_with(&r, "tx.invalid_value");
        assert!(d.message.contains("gap"), "{}", d.message);
        assert!(d.message.contains("px number"), "{}", d.message);
    }
}

#[test]
fn set_layout_unknown_token_is_rejected_by_validation() {
    let r = run(vec![Op::SetLayout(LayoutEdit {
        gap: Some(Some("space.missing".into())),
        ..edit("chips")
    })]);
    assert_eq!(r.status, TxStatus::Rejected, "{:?}", r.diagnostics);
}

// ── set_geometry: hug / fill ─────────────────────────────────────────────────

fn set_size(node: &str, w: Option<&str>, h: Option<f64>) -> Op {
    Op::SetGeometry {
        node: node.to_owned(),
        x: None,
        y: None,
        w: w.map(|v| Some(v.into())),
        h: h.map(|v| Some(v.into())),
        rotate: None,
    }
}

#[test]
fn set_geometry_keyword_writes_the_layout_item() {
    let r = run(vec![set_size("a", Some("fill"), None)]);
    accepted(&r);
    let a = line_of(&r.source_after, "a");
    assert!(a.contains("w=\"fill\""), "{a}");
    assert!(!a.contains("w=(px)"), "{a}");
    let boxes = layout_boxes(&parse(&r.source_after), 0, &zenith_core::default_provider());
    // 400 wide row, gap 10, b is 60 wide: a fills 330.
    assert_eq!(boxes.get("a").map(|b| b.w), Some(330.0));
}

#[test]
fn set_geometry_px_clears_the_keyword() {
    let first = run(vec![set_size("a", Some("hug"), None)]);
    accepted(&first);
    assert!(line_of(&first.source_after, "a").contains("w=\"hug\""));
    let tx = Transaction {
        ops: vec![Op::SetGeometry {
            node: "a".into(),
            x: None,
            y: None,
            w: Some(Some(55.0.into())),
            h: Some(Some("hug".into())),
            rotate: None,
        }],
        permissions: Permissions::default(),
    };
    let r = run_transaction(&parse(&first.source_after), &tx).expect("runs");
    accepted(&r);
    let a = line_of(&r.source_after, "a");
    assert!(a.contains("w=(px)55"), "{a}");
    assert!(!a.contains("w=\"hug\""), "{a}");
    assert!(a.contains("h=\"hug\""), "{a}");
}

#[test]
fn set_geometry_keyword_from_json() {
    let r = run_json(r#"{"ops":[{"op":"set_geometry","node":"b","h":"fill"}]}"#);
    accepted(&r);
    assert!(line_of(&r.source_after, "b").contains("h=\"fill\""));
}

#[test]
fn set_geometry_rejects_an_unknown_keyword() {
    let r = run_json(r#"{"ops":[{"op":"set_geometry","node":"a","w":"auto"}]}"#);
    let d = rejected_with(&r, "tx.invalid_value");
    assert!(d.message.contains("\"auto\""), "{}", d.message);
    assert!(d.message.contains("\"hug\""), "{}", d.message);
}

#[test]
fn set_geometry_keyword_on_a_line_is_unsupported() {
    let r = run(vec![set_size("rule", Some("hug"), None)]);
    rejected_with(&r, "tx.unsupported_property");
}

// ── tx.layout_managed guard ──────────────────────────────────────────────────

fn assert_managed(r: &TxResult, node: &str, frame: &str) {
    let d = rejected_with(r, "tx.layout_managed");
    assert_eq!(d.severity, Severity::Error);
    assert_eq!(d.subject_id.as_deref(), Some(node));
    for want in [
        format!("node \"{node}\""),
        format!("frame \"{frame}\""),
        "set_layout with position=\"absolute\"".to_owned(),
        "reparent".to_owned(),
        "move_forward".to_owned(),
        "move_backward".to_owned(),
    ] {
        assert!(d.message.contains(&want), "missing {want} in {}", d.message);
    }
}

#[test]
fn guard_rejects_set_geometry_xy_on_an_in_flow_child() {
    let r = run(vec![Op::SetGeometry {
        node: "a".into(),
        x: Some(Some(100.0)),
        y: None,
        w: None,
        h: None,
        rotate: None,
    }]);
    assert_managed(&r, "a", "chips");
}

#[test]
fn guard_allows_set_geometry_size_on_an_in_flow_child() {
    let r = run(vec![Op::SetGeometry {
        node: "a".into(),
        x: None,
        y: None,
        w: Some(Some(70.0.into())),
        h: None,
        rotate: None,
    }]);
    accepted(&r);
}

#[test]
fn guard_rejects_align_nodes_on_in_flow_children() {
    let r = run(vec![Op::AlignNodes {
        node_ids: vec!["free".into(), "b".into()],
        align: "left".into(),
        anchor: "page".into(),
    }]);
    assert_managed(&r, "b", "chips");
}

#[test]
fn guard_rejects_distribute_nodes_on_in_flow_children() {
    let r = run(vec![Op::DistributeNodes {
        node_ids: vec!["a".into(), "b".into(), "free".into()],
        axis: "horizontal".into(),
    }]);
    let managed: Vec<_> = r
        .diagnostics
        .iter()
        .filter(|d| d.code == "tx.layout_managed")
        .filter_map(|d| d.subject_id.clone())
        .collect();
    assert_eq!(managed, vec!["a".to_owned(), "b".to_owned()]);
    assert_managed(&r, "a", "chips");
}

#[test]
fn guard_rejects_align_to_edge_on_an_in_flow_child() {
    let r = run(vec![Op::AlignToEdge {
        node: "a".into(),
        edge: "right".into(),
        margin: 0.0,
    }]);
    assert_managed(&r, "a", "chips");
}

#[test]
fn guard_covers_grid_children() {
    let r = run(vec![Op::AlignToEdge {
        node: "t2".into(),
        edge: "left".into(),
        margin: 0.0,
    }]);
    assert_managed(&r, "t2", "tiles");
}

#[test]
fn guard_allows_an_absolute_child() {
    let r = run(vec![Op::SetGeometry {
        node: "abs".into(),
        x: Some(Some(20.0)),
        y: Some(Some(25.0)),
        w: None,
        h: None,
        rotate: None,
    }]);
    accepted(&r);
    let abs = line_of(&r.source_after, "abs");
    assert!(
        abs.contains("x=(px)20") && abs.contains("y=(px)25"),
        "{abs}"
    );
}

#[test]
fn guard_allows_a_child_made_absolute_in_the_same_transaction() {
    let r = run(vec![
        Op::SetLayout(LayoutEdit {
            position: Some(Some("absolute".into())),
            ..edit("a")
        }),
        Op::SetGeometry {
            node: "a".into(),
            x: Some(Some(300.0)),
            y: Some(Some(10.0)),
            w: None,
            h: None,
            rotate: None,
        },
    ]);
    accepted(&r);
    let a = line_of(&r.source_after, "a");
    assert!(a.contains("position=\"absolute\""), "{a}");
    assert!(a.contains("x=(px)300"), "{a}");
}

#[test]
fn guard_leaves_nodes_outside_layout_frames_alone() {
    let r = run(vec![
        Op::AlignToEdge {
            node: "free".into(),
            edge: "left".into(),
            margin: 8.0,
        },
        Op::SetGeometry {
            node: "chips".into(),
            x: Some(Some(30.0)),
            y: None,
            w: None,
            h: None,
            rotate: None,
        },
    ]);
    accepted(&r);
}
