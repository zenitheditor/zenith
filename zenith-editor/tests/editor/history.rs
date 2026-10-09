//! Undo / redo give the exact texts back; typing bursts coalesce; the
//! history stays bounded.

use serde_json::json;
use zenith_editor::{DeltaOut, HistoryLimit};

use crate::common::{Driver, doc};

const BODY: &str = r#"      // keep me
      rect id="r" x=(px)40 y=(px)50 w=(px)100 h=(px)60 fill=(token)"color.ink"
      rect id="s" x=(px)200 y=(px)50 w=(px)100 h=(px)60 fill=(token)"color.ink""#;

/// Apply a reply delta to `text` as the page would (UTF-16 offsets).
fn apply_utf16(text: &str, delta: &DeltaOut) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut out: Vec<u16> = units[..delta.from].to_vec();
    out.extend(delta.insert.encode_utf16());
    out.extend(&units[delta.to..]);
    String::from_utf16(&out).expect("utf16")
}

#[test]
fn commit_undo_redo_returns_the_exact_texts() {
    let original = doc(BODY);
    let mut d = Driver::open(&original);
    let mut page_text = original.clone();
    let commit = d.ok("gesture.commit", json!({ "node": "r", "dx": 13, "dy": -7 }));
    let delta: DeltaOut = serde_json::from_value(commit["delta"].clone()).expect("delta");
    page_text = apply_utf16(&page_text, &delta);
    assert_eq!(
        page_text, d.session.text,
        "the page's copy follows the delta"
    );
    let after = d.session.text.clone();
    let undo = d.ok("history.undo", json!({}));
    assert_eq!(d.session.text, original);
    page_text = apply_utf16(
        &page_text,
        &serde_json::from_value(undo["delta"].clone()).expect("delta"),
    );
    assert_eq!(page_text, original);
    assert_eq!(undo["selection"], json!([]), "selection before the commit");
    let redo = d.ok("history.redo", json!({}));
    assert_eq!(d.session.text, after);
    assert_eq!(redo["selection"], json!(["r"]));
    d.ok("history.undo", json!({}));
    assert_eq!(d.session.text, original);
    assert_eq!(
        d.err("history.undo", json!({})).code,
        "editor.nothing_to_undo"
    );
    // A new edit clears the redo stack.
    d.ok("gesture.commit", json!({ "node": "s", "dx": 1 }));
    assert_eq!(
        d.err("history.redo", json!({})).code,
        "editor.nothing_to_redo"
    );
}

#[test]
fn typing_bursts_coalesce_into_one_undo() {
    let original = doc(BODY);
    let mut d = Driver::open(&original);
    let mut text = original.clone();
    let at = text.find("// keep me").expect("comment") + "// keep me".len();
    for ch in [" ", "n", "o", "w"] {
        let line_end = text[at..].find('\n').map_or(text.len(), |i| at + i);
        text.insert_str(line_end, ch);
        d.ok("buffer.set", json!({ "text": text }));
    }
    assert!(d.session.text.contains("// keep me now"));
    assert_eq!(d.session.history.undo.len(), 1, "one burst");
    d.ok("history.undo", json!({}));
    assert_eq!(d.session.text, original);
    // A burst after an undo starts a new entry; coalesce=false splits.
    let typed = original.replace("// keep me", "// keep me!");
    d.ok("buffer.set", json!({ "text": typed }));
    let more = typed.replace("// keep me!", "// keep me!!");
    d.ok("buffer.set", json!({ "text": more, "coalesce": false }));
    assert_eq!(d.session.history.undo.len(), 2);
}

#[test]
fn history_stays_within_its_bounds() {
    let mut d = Driver::open(&doc(BODY));
    d.session.history.limit = HistoryLimit {
        entries: 3,
        bytes: 1 << 20,
    };
    for i in 0..6 {
        d.ok("gesture.commit", json!({ "node": "r", "dx": 1 + i }));
    }
    assert_eq!(d.session.history.undo.len(), 3);
    for _ in 0..3 {
        d.ok("history.undo", json!({}));
    }
    assert_eq!(
        d.err("history.undo", json!({})).code,
        "editor.nothing_to_undo"
    );
    // The default bound is 200 entries and 1 MiB.
    let fresh = Driver::open(&doc(BODY));
    assert_eq!(fresh.session.history.limit, HistoryLimit::default());
    assert_eq!(HistoryLimit::default().entries, 200);
    assert_eq!(HistoryLimit::default().bytes, 1 << 20);
}

#[test]
fn undo_survives_invalid_text_and_typing_mixes_with_edits() {
    let original = doc(BODY);
    let mut d = Driver::open(&original);
    d.ok("gesture.commit", json!({ "node": "r", "dx": 5 }));
    let moved = d.session.text.clone();
    d.ok("buffer.set", json!({ "text": format!("{moved}}}") }));
    assert!(!d.session.valid);
    d.ok("history.undo", json!({}));
    assert_eq!(d.session.text, moved);
    assert!(d.session.valid);
    d.ok("history.undo", json!({}));
    assert_eq!(d.session.text, original);
}
