//! Property checks over every example and every box-kind node:
//! - a committed move by `(dx, dy)` moves the node's page corners by
//!   exactly `(dx, dy)` (within 1e-6);
//! - a resize of a rotated node from each grip keeps the opposite grip at
//!   its page position (within 1e-6).
//!
//! Nodes the plain gesture rejects (tokens, anchors, layout flow, locks)
//! and nodes under a group that turns about its content are counted as
//! skipped, by reason.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::common::{Driver, assert_moved, doc, example_names, near};

const BOX_KINDS: &[&str] = &[
    "rect", "ellipse", "text", "code", "frame", "group", "image", "instance", "field", "toc",
    "table", "shape", "pattern", "chart", "mesh",
];

fn collect(layers: &Value, out: &mut Vec<(String, String)>) {
    for layer in layers.as_array().into_iter().flatten() {
        if let (Some(id), Some(kind)) = (layer["id"].as_str(), layer["kind"].as_str())
            && layer["guide"] == false
            && layer["visible"] == true
        {
            out.push((id.to_owned(), kind.to_owned()));
        }
        collect(&layer["children"], out);
    }
}

/// Every visible, non-guide node of every page: `(id, kind)`.
fn nodes(d: &mut Driver) -> Vec<(String, String)> {
    let outline = d.ok("doc.outline", json!({}));
    let mut out = Vec::new();
    for page in outline["pages"].as_array().into_iter().flatten() {
        collect(&page["children"], &mut out);
    }
    out
}

fn handle_at(v: &Value, id: &str) -> (f64, f64) {
    let h = v["handles"]
        .as_array()
        .expect("handles")
        .iter()
        .find(|h| h["id"] == id)
        .unwrap_or_else(|| panic!("no handle {id}"));
    (h["x"].as_f64().expect("x"), h["y"].as_f64().expect("y"))
}

#[derive(Default)]
struct Tally {
    ran: usize,
    skipped: BTreeMap<String, usize>,
}

impl Tally {
    fn skip(&mut self, why: &str) {
        *self.skipped.entry(why.to_owned()).or_default() += 1;
    }
}

#[test]
fn moves_shift_page_corners_exactly_in_every_example() {
    let (dx, dy) = (7.25, -3.5);
    let mut tally = Tally::default();
    for name in example_names() {
        let mut d = Driver::example(&name);
        if !d.session.valid {
            tally.skip("example invalid without its project");
            continue;
        }
        for (id, kind) in nodes(&mut d) {
            if !BOX_KINDS.contains(&kind.as_str()) {
                continue;
            }
            let handles = match d.outcome("node.handles", json!({ "id": id })).result {
                Ok(h) => h,
                Err(e) => {
                    tally.skip(&e.code);
                    continue;
                }
            };
            if handles["pivot_follows_content"] == true {
                tally.skip("pivot_follows_content");
                continue;
            }
            let before = d.corners(&id);
            match d
                .outcome("gesture.commit", json!({ "node": id, "dx": dx, "dy": dy }))
                .result
            {
                Ok(reply) if reply["notes"].as_array().is_some_and(|n| !n.is_empty()) => {
                    // An edge anchor keeps its cross axis, and a table cell
                    // re-sizes its row: the reply says the move is not exact.
                    tally.skip(reply["notes"][0]["code"].as_str().unwrap_or("note"));
                }
                Ok(_) => {
                    let after = d.corners(&id);
                    assert_moved(before, after, dx, dy, &format!("{name} {id}"));
                    tally.ran += 1;
                    if !d.session.valid {
                        d.ok("history.undo", json!({}));
                    }
                }
                Err(e) => {
                    let code = e
                        .diagnostics
                        .first()
                        .map_or(e.code.clone(), |x| x.code.clone());
                    tally.skip(&code);
                }
            }
        }
    }
    println!(
        "move property: {} cases ran; skipped {:?}",
        tally.ran, tally.skipped
    );
    assert!(tally.ran >= 75, "only {} cases ran", tally.ran);
}

const ROTATED: &str = r#"      rect id="r" x=(px)40 y=(px)40 w=(px)120 h=(px)60 rotate=(deg)25 fill=(token)"color.ink"
      ellipse id="e" x=(pt)200 y=(px)30 w=(px)80 h=(px)50 rotate=(deg)-40 fill=(token)"color.ink"
      text id="t" x=(px)40 y=(px)160 w=(px)140 h=(px)50 rotate=(deg)12 fill=(token)"color.ink" { span "Turn" }
      frame id="f" x=(px)220 y=(px)140 w=(px)120 h=(px)100 rotate=(deg)200 {
        rect id="fr" x=(px)10 y=(px)10 w=(px)40 h=(px)30 rotate=(deg)33 fill=(token)"color.ink"
      }
      group id="g" x=(px)10 y=(px)220 w=(px)100 h=(px)60 rotate=(deg)-75 {
        rect id="gr" x=(px)0 y=(px)0 w=(px)50 h=(px)20 rotate=(deg)90 fill=(token)"color.ink"
      }"#;

#[test]
fn rotated_resizes_keep_the_opposite_grip_from_every_grip() {
    let grips = [
        ("nw", "se"),
        ("n", "s"),
        ("ne", "sw"),
        ("e", "w"),
        ("se", "nw"),
        ("s", "n"),
        ("sw", "ne"),
        ("w", "e"),
    ];
    let mut tally = Tally::default();
    let mut sources: Vec<(String, Driver)> =
        vec![("synthetic".to_owned(), Driver::open(&doc(ROTATED)))];
    for name in example_names() {
        sources.push((name.clone(), Driver::example(&name)));
    }
    for (name, mut d) in sources {
        if !d.session.valid {
            continue;
        }
        for (id, kind) in nodes(&mut d) {
            if !BOX_KINDS.contains(&kind.as_str()) {
                continue;
            }
            let Ok(h) = d.outcome("node.handles", json!({ "id": id })).result else {
                continue;
            };
            let angle = h["angle"].as_f64().unwrap_or(0.0);
            if angle.abs() < 1e-9 {
                continue;
            }
            if h["pivot_follows_content"] == true {
                tally.skip("pivot_follows_content");
                continue;
            }
            for (grip, opposite) in grips {
                let h = d.ok("node.handles", json!({ "id": id }));
                let fixed = handle_at(&h, opposite);
                let r = d.outcome(
                    "gesture.commit",
                    json!({ "node": id, "handle": grip, "dx": 6.5, "dy": 4.25 }),
                );
                match r.result {
                    Ok(_) => {
                        let now = handle_at(&d.ok("node.handles", json!({ "id": id })), opposite);
                        assert!(
                            near(fixed, now, 1e-6),
                            "{name} {id} {grip}: {fixed:?} -> {now:?}"
                        );
                        tally.ran += 1;
                    }
                    Err(e) => {
                        let code = e
                            .diagnostics
                            .first()
                            .map_or(e.code.clone(), |x| x.code.clone());
                        tally.skip(&code);
                    }
                }
            }
        }
    }
    println!(
        "rotated resize property: {} cases ran; skipped {:?}",
        tally.ran, tally.skipped
    );
    assert!(tally.ran >= 40, "only {} cases ran", tally.ran);
}
