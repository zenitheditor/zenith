//! `zenith schema write`: the example in the guide validates clean, and the
//! guide is reachable from the places an agent reads first.

use std::process::Command;

fn zenith(args: &[&str], dir: &std::path::Path) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .args(args)
        .current_dir(dir)
        .env("ZENITH_DATA_DIR", dir.join("data"))
        .output()
        .expect("run zenith");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8(out.stdout).expect("utf8"),
    )
}

#[test]
fn the_guide_example_validates_and_renders() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (code, text) = zenith(&["schema", "write"], dir.path());
    assert_eq!(code, 0, "{text}");
    let example: Vec<&str> = text
        .lines()
        .skip_while(|l| *l != "---")
        .skip(1)
        .take_while(|l| *l != "---")
        .collect();
    assert!(example.len() > 10, "no example block: {text}");
    std::fs::write(dir.path().join("ex.zen"), example.join("\n")).expect("write");
    let (code, out) = zenith(&["validate", "ex.zen"], dir.path());
    assert_eq!(code, 0, "{out}");
    let (code, out) = zenith(&["render", "ex.zen", "--png", "ex.png"], dir.path());
    assert_eq!(code, 0, "{out}");
}

#[test]
fn the_guide_names_the_common_kinds_and_json_matches() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (_, text) = zenith(&["schema", "write"], dir.path());
    for kind in [
        "page:", "frame:", "text:", "rect:", "ellipse:", "line:", "image:",
    ] {
        assert!(text.contains(kind), "missing {kind}");
    }
    let (code, json) = zenith(&["schema", "write", "--json"], dir.path());
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&json).expect("json");
    assert_eq!(v["kinds"].as_array().map(Vec::len), Some(7));
    assert!(
        v["example"]
            .as_str()
            .is_some_and(|e| e.contains("zenith version=1"))
    );
}

#[test]
fn the_guide_says_to_repeat_and_to_check_pages_together() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (_, guide) = zenith(&["schema", "write"], dir.path());
    for want in ["duplicate_page", "duplicate_node", "--contact-sheet"] {
        assert!(guide.contains(want), "the guide lacks {want}");
    }
}

#[test]
fn the_guide_carries_the_same_common_edits_as_tx_help() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (_, guide) = zenith(&["schema", "write"], dir.path());
    let (_, help) = zenith(&["tx", "--help"], dir.path());
    let edits: Vec<&str> = help
        .lines()
        .skip_while(|l| !l.starts_with("COMMON EDITS"))
        .skip(1)
        .take_while(|l| !l.is_empty())
        .collect();
    assert!(edits.len() >= 6, "{help}");
    for line in edits {
        assert!(guide.contains(line), "guide lacks `{line}`");
    }
}

#[test]
fn the_first_places_an_agent_reads_point_at_the_guide() {
    let dir = tempfile::tempdir().expect("tempdir");
    for args in [&["--help"][..], &["schema"][..]] {
        let (_, text) = zenith(args, dir.path());
        assert!(text.contains("schema write"), "{args:?}: {text}");
    }
    let (_, text) = zenith(&["new", "d.zen", "--starter", "card"], dir.path());
    assert!(text.contains("schema write"), "{text}");
}
