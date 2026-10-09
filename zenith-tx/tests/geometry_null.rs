//! `set_geometry` null semantics: an absent field keeps the attribute, a
//! `null` field removes it, and removing a box field the node needs is
//! rejected with `tx.geometry_required`.

mod common;
use common::parse;
use zenith_tx::{Op, Transaction, TxResult, TxStatus, run_transaction};

/// A row frame with a stale-x/y in-flow rect, an in-flow column card that
/// fills the row, and an absolute child. Outside flow: a free rect, an
/// anchored rect, a group, a hugging column frame, and a plain frame.
const DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.k" type="color" value="#000000"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)800 h=(px)600 {
      frame id="row" x=(px)10 y=(px)20 w=(px)400 h=(px)80 layout="row" gap=(px)10 {
        rect id="flow" x=(px)5 y=(px)7 w=(px)40 h=(px)20 rotate=(deg)10 fill=(token)"color.k"
        frame id="card" w="fill" h=(px)40 layout="column" {
          rect id="card.dot" w=(px)10 h=(px)10 fill=(token)"color.k"
        }
        rect id="abs" x=(px)5 y=(px)5 w=(px)10 h=(px)10 fill=(token)"color.k" position="absolute"
      }
      rect id="free" x=(px)500 y=(px)400 w=(px)50 h=(px)50 rotate=(deg)15 fill=(token)"color.k"
      rect id="pinned" anchor="center" x=(px)0 y=(px)0 w=(px)50 h=(px)50 fill=(token)"color.k"
      group id="grp" x=(px)10 y=(px)300 w=(px)40 h=(px)40 {
        rect id="grp.a" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.k"
      }
      frame id="hug" x=(px)300 y=(px)300 w=(px)100 h=(px)100 layout="column" {
        rect id="hug.a" w=(px)10 h=(px)10 fill=(token)"color.k"
      }
      frame id="plain" x=(px)600 y=(px)100 w=(px)100 h=(px)100 {
        rect id="plain.a" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.k"
      }
    }
  }
}"##;

fn run_json(json: &str) -> TxResult {
    let tx = Transaction::from_json(json).expect("transaction JSON parses");
    run_transaction(&parse(DOC), &tx).expect("run_transaction must not error")
}

/// `{"ops":[{"op":"set_geometry","node":<node>,<fields>}]}`.
fn geometry(node: &str, fields: &str) -> TxResult {
    run_json(&format!(
        r#"{{"ops":[{{"op":"set_geometry","node":"{node}",{fields}}}]}}"#
    ))
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

/// The source line holding `id="<id>"`.
fn line_of<'s>(src: &'s str, id: &str) -> &'s str {
    let needle = format!("id=\"{id}\"");
    src.lines()
        .find(|l| l.contains(&needle))
        .unwrap_or_else(|| panic!("no line for {id}"))
}

/// `true` when the node line sets attribute `attr` (`x=`, `w=` ...).
fn has_attr(line: &str, attr: &str) -> bool {
    line.split_whitespace()
        .any(|tok| tok.starts_with(&format!("{attr}=")))
}

/// Assert a rejection with one `tx.geometry_required` per field in `fields`,
/// each naming its field and the reason, and an unchanged source.
fn rejected_required(r: &TxResult, node: &str, fields: &[&str]) {
    assert_eq!(r.status, TxStatus::Rejected, "{:?}", r.diagnostics);
    assert_eq!(r.source_after, r.source_before);
    let hits: Vec<&zenith_core::Diagnostic> = r
        .diagnostics
        .iter()
        .filter(|d| d.code == "tx.geometry_required")
        .collect();
    assert_eq!(hits.len(), fields.len(), "{:?}", r.diagnostics);
    for (d, field) in hits.iter().zip(fields) {
        assert_eq!(d.subject_id.as_deref(), Some(node));
        assert!(
            d.message.contains(&format!("{field}=null")),
            "message must name {field}: {}",
            d.message
        );
        assert!(
            d.message.contains("no row/column/grid frame"),
            "message must give the reason: {}",
            d.message
        );
    }
}

// ── Removal ─────────────────────────────────────────────────────────────────

#[test]
fn null_x_and_y_remove_stale_position_from_an_in_flow_child() {
    let r = geometry("flow", r#""x":null,"y":null"#);
    accepted(&r);
    let line = line_of(&r.source_after, "flow");
    assert!(!has_attr(line, "x"), "{line}");
    assert!(!has_attr(line, "y"), "{line}");
    assert!(has_attr(line, "w"), "absent w must stay: {line}");
    assert!(has_attr(line, "rotate"), "absent rotate must stay: {line}");
    assert!(
        !r.diagnostics.iter().any(|d| d.code == "tx.layout_managed"),
        "removal is not hand placement: {:?}",
        r.diagnostics
    );
}

#[test]
fn null_rotate_removes_rotate() {
    let r = geometry("free", r#""rotate":null"#);
    accepted(&r);
    let line = line_of(&r.source_after, "free");
    assert!(!has_attr(line, "rotate"), "{line}");
    assert!(has_attr(line, "x") && has_attr(line, "y"), "{line}");
}

#[test]
fn null_w_and_h_remove_px_size_and_keyword_in_flow() {
    let r = geometry("card", r#""w":null,"h":null"#);
    accepted(&r);
    let line = line_of(&r.source_after, "card");
    assert!(!has_attr(line, "w"), "w keyword must go: {line}");
    assert!(!has_attr(line, "h"), "h px must go: {line}");
}

#[test]
fn null_xy_on_an_anchored_node_is_accepted() {
    let r = geometry("pinned", r#""x":null,"y":null"#);
    accepted(&r);
    let line = line_of(&r.source_after, "pinned");
    assert!(!has_attr(line, "x") && !has_attr(line, "y"), "{line}");
    assert!(line.contains("anchor=\"center\""), "{line}");
}

#[test]
fn null_box_on_a_group_is_accepted() {
    let r = geometry("grp", r#""x":null,"y":null,"w":null,"h":null"#);
    accepted(&r);
    let line = line_of(&r.source_after, "grp");
    for attr in ["x", "y", "w", "h"] {
        assert!(!has_attr(line, attr), "{attr}: {line}");
    }
}

#[test]
fn null_size_on_a_hugging_layout_frame_is_accepted() {
    let r = geometry("hug", r#""w":null,"h":null"#);
    accepted(&r);
    let line = line_of(&r.source_after, "hug");
    assert!(!has_attr(line, "w") && !has_attr(line, "h"), "{line}");
    assert!(has_attr(line, "x") && has_attr(line, "y"), "{line}");
}

#[test]
fn absent_fields_leave_attributes_unchanged() {
    let r = geometry("free", r#""w":60"#);
    accepted(&r);
    let line = line_of(&r.source_after, "free");
    assert!(
        line.contains("x=(px)500") && line.contains("y=(px)400"),
        "{line}"
    );
    assert!(
        line.contains("w=(px)60") && line.contains("h=(px)50"),
        "{line}"
    );
    assert!(line.contains("rotate=(deg)15"), "{line}");
}

// ── Rejection ───────────────────────────────────────────────────────────────

#[test]
fn null_x_on_a_free_node_is_rejected() {
    rejected_required(&geometry("free", r#""x":null"#), "free", &["x"]);
}

#[test]
fn null_y_on_a_free_node_is_rejected() {
    rejected_required(&geometry("free", r#""y":null"#), "free", &["y"]);
}

#[test]
fn null_w_and_h_on_a_free_node_are_rejected() {
    rejected_required(
        &geometry("free", r#""w":null,"h":null"#),
        "free",
        &["w", "h"],
    );
}

#[test]
fn null_xy_on_an_absolute_child_is_rejected() {
    rejected_required(&geometry("abs", r#""x":null,"y":null"#), "abs", &["x", "y"]);
}

#[test]
fn null_size_on_a_plain_frame_is_rejected() {
    rejected_required(&geometry("plain", r#""w":null"#), "plain", &["w"]);
}

// ── Serde shape ─────────────────────────────────────────────────────────────

#[test]
fn null_and_absent_round_trip_through_json() {
    let tx = Transaction::from_json(
        r#"{"ops":[{"op":"set_geometry","node":"n","x":null,"w":"fill","rotate":5}]}"#,
    )
    .expect("parses");
    let [
        Op::SetGeometry {
            x, y, w, h, rotate, ..
        },
    ] = tx.ops.as_slice()
    else {
        panic!("expected one set_geometry op: {:?}", tx.ops);
    };
    assert_eq!(*x, Some(None));
    assert_eq!(*y, None);
    assert_eq!(*w, Some(Some("fill".into())));
    assert_eq!(*h, None);
    assert_eq!(*rotate, Some(Some(5.0)));

    let json = serde_json::to_value(&tx.ops).expect("serializes");
    let op = &json[0];
    assert!(op.get("x").is_some_and(serde_json::Value::is_null), "{op}");
    assert!(op.get("y").is_none(), "{op}");
    assert!(op.get("h").is_none(), "{op}");
}
