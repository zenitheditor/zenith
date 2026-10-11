//! `zenith schema node <kind>` prints a brief by default: attribute families
//! fold into their base row and long prose is cut at a sentence. `--notes`
//! prints everything. JSON is always whole.

use std::process::Command;

fn schema_node(kind: &str, extra: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .args(["schema", "node", kind])
        .args(extra)
        .output()
        .expect("run zenith");
    assert_eq!(out.status.code(), Some(0));
    String::from_utf8(out.stdout).expect("utf8")
}

#[test]
fn default_output_folds_families_and_names_notes() {
    let brief = schema_node("frame", &[]);
    assert!(
        brief.contains(
            "(also padding-x, padding-y, padding-top, padding-right, padding-bottom, padding-left)"
        ),
        "{brief}"
    );
    assert!(
        !brief.contains("\n  padding-x "),
        "family member row remains:\n{brief}"
    );
    assert!(
        brief.contains("zenith schema node frame --notes"),
        "{brief}"
    );
    assert!(
        brief.contains("layout=\"row\""),
        "layout rules were cut:\n{brief}"
    );
}

#[test]
fn notes_prints_every_row_and_the_whole_prose() {
    let brief = schema_node("text", &[]);
    let full = schema_node("text", &["--notes"]);
    assert!(
        full.len() > brief.len() * 3 / 2,
        "{} vs {}",
        full.len(),
        brief.len()
    );
    assert!(full.contains("\n  margin-top "), "{full}");
    assert!(!full.contains("Shortened:"), "{full}");
}

#[test]
fn a_short_kind_is_not_marked_shortened() {
    assert!(!schema_node("line", &[]).contains("Shortened:"));
}

#[test]
fn json_stays_whole() {
    let out = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .args(["schema", "node", "frame", "--json"])
        .output()
        .expect("run zenith");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let names: Vec<&str> = v["node"]["attributes"]
        .as_array()
        .expect("attributes")
        .iter()
        .filter_map(|a| a["name"].as_str())
        .collect();
    assert!(
        names.contains(&"padding-top") && names.contains(&"margin-left"),
        "{names:?}"
    );
}
