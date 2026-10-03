//! `zenith inspect` resolved boxes: every node reports its final
//! page-absolute `box` after auto-layout and group offsets, next to its
//! authored `geometry`. MCP `zenith_inspect` reports the same boxes. Also
//! checks that the `zenith schema node frame` layout example validates.

use serde_json::{Value, json};
use zenith_cli::commands::inspect::run;
use zenith_cli::config::CliPolicyFlags;
use zenith_cli::mcp::handle_message;

/// A row frame with two flow chips and an absolute badge, a free rect, and a
/// group (origin 100,300) holding a column frame and a plain rect.
const DOC: &str = r##"zenith version=1 {
  project id="proj.il" name="IL"
  tokens format="zenith-token-v1" {
    token id="color.k" type="color" value="#000000"
    token id="space.md" type="dimension" value=(px)10
  }
  styles {}
  document id="doc.il" title="IL" {
    page id="p" w=(px)800 h=(px)600 {
      frame id="row" x=(px)10 y=(px)20 w=(px)400 h=(px)60 layout="row" gap=(token)"space.md" align="start" {
        rect id="a" w=(px)40 h=(px)20 fill=(token)"color.k"
        rect id="b" w=(px)60 h=(px)30 fill=(token)"color.k"
        rect id="badge" x=(px)5 y=(px)6 w=(px)10 h=(px)10 fill=(token)"color.k" position="absolute"
      }
      rect id="free" x=(px)500 y=(px)400 w=(px)50 h=(px)50 fill=(token)"color.k"
      group id="g" x=(px)100 y=(px)300 {
        frame id="col" x=(px)10 y=(px)10 w=(px)200 layout="column" padding=(px)4 gap=(px)6 align="start" {
          rect id="c1" w=(px)30 h=(px)20 fill=(token)"color.k"
          rect id="c2" w="fill" h=(px)10 fill=(token)"color.k"
        }
        rect id="inner" x=(px)5 y=(px)5 w=(px)20 h=(px)20 fill=(token)"color.k"
      }
    }
  }
}
"##;

fn inspect_json(node: Option<&str>) -> Value {
    let out = run(DOC, node, true, None).expect("inspect succeeds");
    serde_json::from_str(&out).expect("inspect JSON parses")
}

/// Depth-first search for the node entry with `id`.
fn find<'v>(v: &'v Value, id: &str) -> Option<&'v Value> {
    if v.get("id").and_then(Value::as_str) == Some(id) && v.get("kind").is_some() {
        return Some(v);
    }
    let kids = v
        .get("children")
        .or_else(|| v.get("pages"))
        .and_then(Value::as_array)?;
    kids.iter().find_map(|k| find(k, id))
}

fn node<'v>(root: &'v Value, id: &str) -> &'v Value {
    find(root, id).unwrap_or_else(|| panic!("no node {id} in {root}"))
}

fn bx(v: &Value) -> (f64, f64, f64, f64) {
    let b = &v["box"];
    let f = |k: &str| {
        b[k].as_f64()
            .unwrap_or_else(|| panic!("box.{k} missing in {v}"))
    };
    (f("x"), f("y"), f("w"), f("h"))
}

#[test]
fn laid_out_children_report_exact_boxes() {
    let root = inspect_json(None);
    assert_eq!(bx(node(&root, "row")), (10.0, 20.0, 400.0, 60.0));
    assert_eq!(bx(node(&root, "a")), (10.0, 20.0, 40.0, 20.0));
    assert_eq!(bx(node(&root, "b")), (60.0, 20.0, 60.0, 30.0));
    // The authored geometry stays as written: a flow child has no x/y.
    let a = node(&root, "a");
    assert!(a["geometry"]["x"].is_null(), "{a}");
    assert_eq!(a["geometry"]["w"], 40.0);
}

#[test]
fn absolute_nodes_report_their_authored_box() {
    let root = inspect_json(None);
    let free = node(&root, "free");
    assert_eq!(bx(free), (500.0, 400.0, 50.0, 50.0));
    let g = &free["geometry"];
    assert_eq!(
        (
            g["x"].as_f64(),
            g["y"].as_f64(),
            g["w"].as_f64(),
            g["h"].as_f64()
        ),
        (Some(500.0), Some(400.0), Some(50.0), Some(50.0))
    );
    // An absolute child of a layout frame counts x/y from the frame's top-left.
    assert_eq!(bx(node(&root, "badge")), (15.0, 26.0, 10.0, 10.0));
}

#[test]
fn nodes_inside_groups_report_page_absolute_boxes() {
    let root = inspect_json(None);
    assert_eq!(bx(node(&root, "col")), (110.0, 310.0, 200.0, 44.0));
    assert_eq!(bx(node(&root, "c1")), (114.0, 314.0, 30.0, 20.0));
    assert_eq!(bx(node(&root, "c2")), (114.0, 340.0, 192.0, 10.0));
    assert_eq!(bx(node(&root, "inner")), (105.0, 305.0, 20.0, 20.0));
}

#[test]
fn node_subtree_carries_boxes() {
    let root = inspect_json(Some("col"));
    assert_eq!(root["node"]["id"], "col");
    assert_eq!(bx(node(&root["node"], "c2")), (114.0, 340.0, 192.0, 10.0));
}

#[test]
fn human_output_shows_boxes_that_differ_from_authored_geometry() {
    let out = run(DOC, None, false, None).expect("inspect succeeds");
    let line = |id: &str| {
        out.lines()
            .find(|l| l.split_whitespace().nth(1) == Some(id))
            .unwrap_or_else(|| panic!("no line for {id} in\n{out}"))
            .to_owned()
    };
    assert!(line("b").contains("box=60,20 60x30"), "{}", line("b"));
    assert!(
        line("inner").contains("box=105,305 20x20"),
        "{}",
        line("inner")
    );
    assert!(!line("free").contains("box="), "{}", line("free"));
}

#[test]
fn mcp_inspect_reports_the_same_boxes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("d.zen");
    std::fs::write(&path, DOC).expect("write doc");
    let req = json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {
            "name": "zenith_inspect",
            "arguments": { "doc": path.to_str().expect("utf-8 path"), "depth": 4, "detail": true }
        }
    });
    let resp = handle_message(&req.to_string()).expect("response");
    let mcp = &resp["result"]["structuredContent"];
    let cli = inspect_json(None);
    for id in ["row", "a", "b", "badge", "free", "col", "c1", "c2", "inner"] {
        assert_eq!(bx(node(mcp, id)), bx(node(&cli, id)), "{id}");
    }
    // Without detail, no geometry and no box.
    let shallow = json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {
            "name": "zenith_inspect",
            "arguments": { "doc": path.to_str().expect("utf-8 path"), "depth": 4 }
        }
    });
    let resp = handle_message(&shallow.to_string()).expect("response");
    let a = node(&resp["result"]["structuredContent"], "a");
    assert!(a["box"].is_null(), "{a}");
}

#[test]
fn schema_frame_example_validates() {
    let (text, code) = zenith_cli::commands::schema::node_detail("frame", false);
    assert_eq!(code, 0);
    assert!(text.contains("Example:"), "{text}");
    assert!(text.contains("layout=\"row\""), "{text}");
    assert!(text.contains("layout=\"column\""), "{text}");
    let example = zenith_core::schema::node_example("frame").expect("frame example");
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.ex" name="EX"
  tokens format="zenith-token-v1" {{
    token id="color.chip" type="color" value="#e2e8f0"
    token id="color.ink" type="color" value="#0f172a"
    token id="color.surface" type="color" value="#ffffff"
    token id="radius.pill" type="dimension" value=(px)999
    token id="size.label" type="dimension" value=(px)14
    token id="size.title" type="dimension" value=(px)22
    token id="space.md" type="dimension" value=(px)12
    token id="space.lg" type="dimension" value=(px)20
  }}
  styles {{}}
  document id="doc.ex" title="EX" {{
    page id="p" w=(px)800 h=(px)600 {{
{example}
    }}
  }}
}}
"##
    );
    let out = zenith_cli::commands::validate::run(&src, None, true, &CliPolicyFlags::default());
    assert_eq!(out.exit_code, 0, "{}", out.stdout);
    let report: Value = serde_json::from_str(&out.stdout).expect("validate JSON");
    let errors: Vec<&Value> = report["diagnostics"]
        .as_array()
        .map(|d| d.iter().filter(|x| x["severity"] == "error").collect())
        .unwrap_or_default();
    assert!(errors.is_empty(), "{errors:?}");
}

/// Anchored, self-sizing, stroked, table-cell, and master content.
const COMPILED_DOC: &str = r##"zenith version=1 {
  project id="proj.cb" name="CB"
  tokens format="zenith-token-v1" {
    token id="color.k" type="color" value="#000000"
    token id="size.stroke" type="dimension" value=(px)4
  }
  styles {}
  masters {
    master id="m" {
      field id="mf" type="page-number" x=(px)700 y=(px)10 w=(px)80 h=(px)30 fill=(token)"color.k"
      text id="mt" x=(px)10 y=(px)560 w=(px)100 h=(px)20 fill=(token)"color.k" {
        span "Footer"
      }
    }
  }
  document id="doc.cb" title="CB" {
    page id="p" w=(px)800 h=(px)600 master="m" {
      rect id="s" x=(px)100 y=(px)100 w=(px)200 h=(px)100 fill=(token)"color.k"
      rect id="anch" anchor="bottom-right" anchor-sibling="s" w=(px)40 h=(px)20 fill=(token)"color.k"
      text id="t" x=(px)10 y=(px)300 w=(px)200 font-size=(px)20 fill=(token)"color.k" {
        span "Hello"
      }
      line id="l" x1=(px)10 y1=(px)400 x2=(px)110 y2=(px)400 stroke=(token)"color.k" stroke-width=(token)"size.stroke"
      rect id="p1" x=(px)400 y=(px)100 w=(px)50 h=(px)50 fill=(token)"color.k"
      rect id="p2" x=(px)600 y=(px)100 w=(px)50 h=(px)50 fill=(token)"color.k"
      connector id="c" from="p1" to="p2" stroke=(token)"color.k"
      table id="tb" x=(px)20 y=(px)450 w=(px)200 h=(px)40 cell-padding=(px)0 gap=(px)0 {
        column
        row { cell { rect id="cellr" x=(px)5 y=(px)5 w=(px)10 h=(px)10 fill=(token)"color.k" } }
      }
    }
  }
}
"##;

fn compiled_json() -> Value {
    let out = run(COMPILED_DOC, None, true, None).expect("inspect succeeds");
    serde_json::from_str(&out).expect("inspect JSON parses")
}

#[test]
fn anchored_rect_box_uses_the_anchor() {
    assert_eq!(
        bx(node(&compiled_json(), "anch")),
        (260.0, 180.0, 40.0, 20.0)
    );
}

#[test]
fn text_without_h_box_has_the_measured_height() {
    let (x, y, w, h) = bx(node(&compiled_json(), "t"));
    assert_eq!((x, y, w), (10.0, 300.0, 200.0));
    assert!(h > 15.0 && h < 60.0, "measured height {h}");
}

#[test]
fn line_box_is_the_stroked_bounds() {
    assert_eq!(bx(node(&compiled_json(), "l")), (8.0, 398.0, 104.0, 4.0));
}

#[test]
fn connector_box_spans_its_route() {
    let (x, y, w, h) = bx(node(&compiled_json(), "c"));
    assert!(x >= 440.0 && x + w <= 610.0 && w >= 140.0, "{x} {w}");
    assert!(y <= 125.0 && y + h >= 125.0, "{y} {h}");
}

#[test]
fn table_cell_child_box_is_inside_its_cell() {
    assert_eq!(
        bx(node(&compiled_json(), "cellr")),
        (25.0, 455.0, 10.0, 10.0)
    );
}

#[test]
fn master_content_boxes_are_listed_under_expanded_ids() {
    let root = compiled_json();
    let expanded = &root["pages"][0]["expanded"];
    assert_eq!(bx(&expanded["p/mf"]), (700.0, 10.0, 80.0, 30.0));
    assert_eq!(bx(&expanded["p/mt"]), (10.0, 560.0, 100.0, 20.0));
    // Page-tree nodes are not repeated there.
    assert!(expanded.get("anch").is_none(), "{expanded}");
}

/// A rotated rect, a footnote, and a pattern.
const ROTATED_DOC: &str = r##"zenith version=1 {
  project id="proj.rt" name="RT"
  tokens format="zenith-token-v1" {
    token id="color.k" type="color" value="#000000"
  }
  styles {}
  document id="doc.rt" title="RT" {
    page id="p" w=(px)600 h=(px)900 margin-inner=(px)60 margin-outer=(px)60 margin-top=(px)80 margin-bottom=(px)80 {
      rect id="rot" x=(px)100 y=(px)100 w=(px)40 h=(px)20 rotate=(deg)90 fill=(token)"color.k"
      rect id="flat" x=(px)300 y=(px)100 w=(px)40 h=(px)20 fill=(token)"color.k"
      text id="word" x=(px)100 y=(px)300 w=(px)200 h=(px)40 font-size=(px)20 fill=(token)"color.k" {
        span "Ink" footnote-ref="fn.1"
      }
      footnote id="fn.1" fill=(token)"color.k" { span "A note." }
      pattern id="pat" kind="grid" x=(px)100 y=(px)500 w=(px)100 h=(px)100 spacing=(px)50 {
        ellipse id="dot" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.k"
      }
    }
  }
}
"##;

fn rotated_json() -> Value {
    let out = run(ROTATED_DOC, None, true, None).expect("inspect succeeds");
    serde_json::from_str(&out).expect("inspect JSON parses")
}

fn rounded(v: &Value, key: &str) -> (f64, f64, f64, f64) {
    let r = |k: &str| (v[key][k].as_f64().unwrap_or(f64::NAN) * 1000.0).round() / 1000.0;
    (r("x"), r("y"), r("w"), r("h"))
}

#[test]
fn rotated_node_reports_rotate_and_bounds() {
    let root = rotated_json();
    let rot = node(&root, "rot");
    assert_eq!(bx(rot), (100.0, 100.0, 40.0, 20.0));
    assert_eq!(rot["rotate"], 90.0);
    assert_eq!(rounded(rot, "bounds"), (110.0, 90.0, 20.0, 40.0));
    // An unrotated, unstroked rect paints exactly its box: no bounds, no rotate.
    let flat = node(&root, "flat");
    assert!(
        flat.get("bounds").is_none() && flat.get("rotate").is_none(),
        "{flat}"
    );
    let out = run(ROTATED_DOC, None, false, None).expect("inspect succeeds");
    assert!(out.contains("rot=90 bounds=110,90 20x40"), "{out}");
}

#[test]
fn text_bounds_are_its_ink() {
    let root = rotated_json();
    let word = node(&root, "word");
    assert_eq!(bx(word), (100.0, 300.0, 200.0, 40.0));
    assert!(
        word["bounds"]["w"].as_f64().is_some_and(|w| w < 200.0),
        "{word}"
    );
}

#[test]
fn footnote_box_is_its_zone_slot() {
    let (x, y, w, h) = bx(node(&rotated_json(), "fn.1"));
    assert_eq!((x, w), (60.0, 480.0));
    assert!(h > 0.0 && y > 700.0 && y + h <= 820.5, "{y} {h}");
}

#[test]
fn pattern_instances_are_listed_under_expanded_ids() {
    let root = rotated_json();
    let expanded = &root["pages"][0]["expanded"];
    assert_eq!(bx(&expanded["pat/0/dot"]), (100.0, 500.0, 10.0, 10.0));
    assert_eq!(bx(&expanded["pat/3/dot"]), (150.0, 550.0, 10.0, 10.0));
}
