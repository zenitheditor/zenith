//! `commands.batch`: several commands in one call, each exactly as sent
//! alone, with one parse per text.

use serde_json::{Value, json};
use zenith_editor::{Request, Session, Work};

use crate::common::Driver;

/// The keystroke frame the page sends: a text change, a viewport render,
/// the outline, the color tokens, and the node under the cursor.
fn frame(session: &Session) -> Vec<Request> {
    let text = session.text.replacen("span \"", "span \"x", 1);
    let cursor = text.find("span \"x").expect("span") + 7;
    vec![
        Request::new("buffer.set", json!({ "text": text })).at(session.version),
        Request::new(
            "doc.render",
            json!({ "scale": 1.25, "viewport": { "x": 10, "y": 10, "w": 300, "h": 200 } }),
        ),
        Request::new("doc.outline", json!({})),
        Request::new("doc.tokens", json!({ "type": "color" })),
        Request::new("select.at_offset", json!({ "offset": cursor })),
    ]
}

fn batch(steps: &[Request]) -> Value {
    json!({ "steps": steps })
}

#[test]
fn batch_equals_the_same_requests_sent_one_by_one() {
    for name in ["stack.zen", "multipage.zen", "flowchart.zen", "table.zen"] {
        let mut one = Driver::example(name);
        let steps = frame(&one.session);
        let mut many = Driver::example(name);
        let start = one.session.clone();

        let mut work = Work::default();
        let mut replies = Vec::new();
        let mut image = None;
        for step in &steps {
            let out = one.send(step);
            work += out.work;
            if out.image.is_some() {
                image = out.image;
            }
            replies.push(out.result.expect("step"));
        }

        let out = many.outcome("commands.batch", batch(&steps));
        let reply = out.result.expect("batch");
        assert_eq!(many.session, one.session, "{name}: session");
        assert_eq!(out.image, image, "{name}: image");
        assert_eq!(reply["image_step"], 1, "{name}");
        let got = reply["steps"].as_array().expect("steps");
        assert_eq!(got.len(), steps.len());
        for ((step, want), got) in steps.iter().zip(&replies).zip(got) {
            assert_eq!(got["command"], step.command.as_str());
            assert_eq!(got["ok"], true, "{name}: {got}");
            assert_eq!(&got["result"], want, "{name}: {}", step.command);
        }
        // One parse of the new text serves every step; one by one, each
        // call parses it again.
        assert_eq!(work.parses, 5, "{name}: one by one");
        assert_eq!(
            out.work,
            Work {
                parses: 1,
                validations: 1,
                tx_runs: 0,
                patches: 0,
                compiles: 1,
                rasters: 1,
            },
            "{name}: batch"
        );
        assert_ne!(start, many.session);
    }
}

#[test]
fn failed_text_step_stops_the_batch() {
    let mut d = Driver::example("stack.zen");
    let before = d.session.clone();
    let mut steps = frame(&d.session);
    steps[0].version = Some(before.version + 7);
    let out = d.outcome("commands.batch", batch(&steps));
    let reply = out.result.expect("batch");
    let got = reply["steps"].as_array().expect("steps");
    assert_eq!(got[0]["ok"], false);
    assert_eq!(got[0]["error"]["code"], "editor.stale_version");
    for later in &got[1..] {
        assert_eq!(later["ok"], false);
        assert_eq!(later["error"]["code"], "editor.skipped", "{later}");
    }
    assert!(reply.get("image_step").is_none());
    assert!(out.image.is_none());
    assert_eq!(d.session, before);
}

#[test]
fn failed_query_step_keeps_going() {
    let mut d = Driver::example("stack.zen");
    let len = d.session.text.len();
    let steps = [
        Request::new("select.at_offset", json!({ "offset": len + 1 })),
        Request::new("doc.outline", json!({})),
    ];
    let reply = d.ok("commands.batch", batch(&steps));
    assert_eq!(reply["steps"][0]["error"]["code"], "editor.invalid_params");
    assert_eq!(reply["steps"][1]["ok"], true);
    assert!(reply["steps"][1]["result"]["pages"].is_array());
}

#[test]
fn nested_long_or_two_image_batches_are_refused() {
    let mut d = Driver::example("stack.zen");
    let render = Request::new("doc.render", json!({ "scale": 1 }));
    let nested = Request::new("commands.batch", json!({ "steps": [] }));
    let outline = Request::new("doc.outline", json!({}));
    for steps in [
        vec![nested],
        vec![render.clone(), render],
        vec![outline; 17],
    ] {
        let e = d.err("commands.batch", batch(&steps));
        assert_eq!(e.code, "editor.invalid_params", "{}", e.message);
    }
    let empty = d.ok("commands.batch", json!({ "steps": [] }));
    assert_eq!(empty, json!({ "steps": [] }));
}
