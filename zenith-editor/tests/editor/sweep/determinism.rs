//! The same command script twice gives the same bytes: sessions, results,
//! and PNGs.

use serde_json::{Value, json};
use zenith_editor::hex_sha256;

use crate::common::Driver;

/// A script over `flowchart.zen`: open, render, select, preview, commit,
/// type, undo, redo, structure edits.
fn script(d: &mut Driver) -> Vec<String> {
    let steps: Vec<(&str, Value)> = vec![
        ("doc.render", json!({ "scale": 0.5 })),
        ("doc.outline", json!({})),
        ("select.hit", json!({ "x": 180, "y": 60, "tolerance": 2 })),
        ("node.handles", json!({})),
        (
            "gesture.preview",
            json!({ "dx": 12, "dy": 8, "scale": 0.5 }),
        ),
        ("gesture.commit", json!({ "dx": 12, "dy": 8 })),
        ("doc.render", json!({ "scale": 0.5 })),
        ("history.undo", json!({})),
        ("history.redo", json!({})),
        ("node.duplicate", json!({})),
        ("node.reorder", json!({ "to": "back" })),
        ("node.inspect", json!({})),
        ("doc.diagnose", json!({})),
        ("commands.list", json!({})),
    ];
    let mut log = Vec::new();
    for (command, params) in steps {
        let out = d.outcome(command, params);
        let result = match &out.result {
            Ok(v) => v.to_string(),
            Err(e) => format!("error {}", serde_json::to_string(e).expect("error json")),
        };
        let png = out.image.as_ref().map(|i| hex_sha256(&i.png));
        log.push(format!(
            "{command}\n{}\n{result}\n{png:?}\n{:?}",
            serde_json::to_string(&out.session).expect("session json"),
            out.work
        ));
    }
    log
}

#[test]
fn a_script_runs_byte_identically_twice() {
    let first = script(&mut Driver::example("flowchart.zen"));
    let second = script(&mut Driver::example("flowchart.zen"));
    assert_eq!(first.len(), second.len());
    for (a, b) in first.iter().zip(&second) {
        assert_eq!(a, b);
    }
    assert!(
        first.iter().all(|s| !s.contains("\nerror ")),
        "every step succeeds: {first:#?}"
    );
}
