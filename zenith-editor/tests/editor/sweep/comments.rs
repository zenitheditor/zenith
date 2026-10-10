//! Property check over generated documents: every comment of the text
//! before an edit is in the text after it, or the reply's
//! `removed_comments` reports it. Every reported comment is gone.
//!
//! The documents carry each comment kind in each place: `//` lines and
//! `/* … */` lines above a node, a trailing `// …`, `/* … */` between
//! entries, after `=`, and inside a type, slashdashed entries, and
//! slashdashed nodes. Each comment text is unique, so a comment is either
//! kept or reported.

use serde_json::{Value, json};

use crate::common::{Driver, doc};

/// A small deterministic generator (an LCG): the same documents on every
/// run and machine.
struct Gen(u64);

impl Gen {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

/// A generated document body and its comments.
struct Generated {
    body: String,
    comments: Vec<String>,
    rects: usize,
}

fn generate(seed: u64) -> Generated {
    let mut g = Gen(seed);
    let mut body = String::new();
    let mut comments: Vec<String> = Vec::new();
    let note = |text: String, comments: &mut Vec<String>| {
        comments.push(text.clone());
        text
    };
    let rects = 3 + (g.next() % 3) as usize;
    for i in 0..rects {
        let tag = format!("{seed}.{i}");
        if g.chance(50) {
            let c = note(format!("// above {tag}"), &mut comments);
            body.push_str(&format!("      {c}\n"));
        }
        if g.chance(30) {
            let c = note(format!("/* block {tag} */"), &mut comments);
            body.push_str(&format!("      {c}\n"));
        }
        let x = match g.next() % 3 {
            0 => "x=(px)10".to_owned(),
            1 => {
                let c = note(format!("/* eq {tag} */"), &mut comments);
                format!("x= {c} (px)10")
            }
            _ => {
                let c = note(format!("/* ty {tag} */"), &mut comments);
                format!("x=({c}px)10")
            }
        };
        let mid = if g.chance(40) {
            let c = note(format!("/* mid {tag} */"), &mut comments);
            format!(" {c}")
        } else {
            String::new()
        };
        let dashed = if g.chance(30) {
            let c = note(format!("/-label=\"sd {tag}\""), &mut comments);
            format!(" {c}")
        } else {
            String::new()
        };
        let trail = if g.chance(50) {
            let c = note(format!("// trail {tag}"), &mut comments);
            format!(" {c}")
        } else {
            String::new()
        };
        let y = 20 + 40 * i;
        body.push_str(&format!(
            "      rect id=\"r{i}\" {x} y=(px){y}{mid} w=(px)40 h=(px)30{dashed}{trail}\n"
        ));
        if g.chance(25) {
            let dead = note(
                format!("/-rect id=\"dead{i}\" x=(px)0 y=(px)0 w=(px)1 h=(px)1"),
                &mut comments,
            );
            let c = note(format!("// dead {tag}"), &mut comments);
            body.push_str(&format!("      {dead} {c}\n"));
        }
    }
    let inner = note(format!("// inside {seed}"), &mut comments);
    body.push_str(&format!(
        "      frame id=\"f\" x=(px)200 y=(px)20 w=(px)120 h=(px)120 {{\n        {inner}\n        \
         rect id=\"fr\" x=(px)0 y=(px)0 w=(px)20 h=(px)20\n      }}"
    ));
    Generated {
        body,
        comments,
        rects,
    }
}

/// The edits to try on a document with `rects` rects.
fn edits(seed: u64, rects: usize) -> Vec<(&'static str, Value)> {
    let i = (seed as usize) % rects;
    let j = (i + 1) % rects;
    let id = format!("r{i}");
    vec![
        ("gesture.commit", json!({ "node": id, "dx": 5, "dy": -3 })),
        (
            "gesture.commit",
            json!({ "node": id, "handle": "se", "dx": 4, "dy": 6 }),
        ),
        (
            "gesture.commit",
            json!({ "node": id, "handle": "rotate", "angle": 30 }),
        ),
        (
            "gesture.commit",
            json!({ "nodes": [id, format!("r{j}"), "f"], "dx": 2, "dy": 2 }),
        ),
        ("node.set", json!({ "id": id, "x": 33, "opacity": 0.5 })),
        (
            "tx.apply",
            json!({ "ops": [{ "op": "set_geometry", "node": id, "x": 1.5, "y": 2.5 }] }),
        ),
        ("node.remove", json!({ "ids": [id] })),
        ("node.remove", json!({ "ids": ["f"] })),
        ("node.duplicate", json!({ "id": id, "dx": 10 })),
        ("node.reorder", json!({ "id": id, "to": "front" })),
        ("node.group", json!({ "ids": [id, format!("r{j}")] })),
        ("doc.format", json!({})),
    ]
}

/// The comment text of a `removed_comments` entry, `line N: <comment>`
/// with an optional ` (in: <line>)`.
fn reported_has(reported: &[String], comment: &str) -> bool {
    reported.iter().any(|r| {
        r.split_once(": ")
            .is_some_and(|(head, rest)| head.starts_with("line ") && rest.starts_with(comment))
    })
}

#[test]
fn every_dropped_comment_is_reported_and_every_report_is_dropped() {
    let mut checked = 0;
    let mut reported_total = 0;
    for seed in 0..48_u64 {
        let generated = generate(seed);
        let text = doc(&generated.body);
        for (command, params) in edits(seed, generated.rects) {
            let mut d = Driver::open(&text);
            assert!(d.session.valid, "seed {seed} invalid:\n{text}");
            let reply = d.ok(command, params.clone());
            let after = d.session.text.clone();
            let reported: Vec<String> = reply["removed_comments"]
                .as_array()
                .expect("removed_comments")
                .iter()
                .map(|v| v.as_str().expect("string").to_owned())
                .collect();
            for c in &generated.comments {
                let kept = after.contains(c.as_str());
                let told = reported_has(&reported, c);
                assert!(
                    kept != told,
                    "seed {seed} {command} {params}: {c:?} kept={kept} reported={told}\n\
                     reported: {reported:?}\n{after}"
                );
            }
            assert!(
                reported.iter().all(|r| generated
                    .comments
                    .iter()
                    .any(|c| reported_has(std::slice::from_ref(r), c))),
                "seed {seed} {command}: unknown report in {reported:?}"
            );
            reported_total += reported.len();
            checked += 1;
        }
    }
    assert_eq!(checked, 48 * 12);
    assert!(reported_total > 0, "some edit drops comments");
}
