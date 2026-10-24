//! `zenith tx` names the ids a duplicate op creates, in human and `--json`
//! output. `tx.unknown_node` suggests the closest existing ids.

use std::process::{Command, Output};

/// A temp directory holding a copy of `examples/multipage.zen` as `t.zen`.
struct Env {
    dir: tempfile::TempDir,
}

impl Env {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let src =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/multipage.zen");
        std::fs::copy(src, dir.path().join("t.zen")).expect("copy example");
        Self { dir }
    }

    fn tx(&self, tx_json: &str, extra: &[&str]) -> Output {
        std::fs::write(self.dir.path().join("tx.json"), tx_json).expect("write tx");
        let mut args = vec!["tx", "t.zen", "tx.json"];
        args.extend_from_slice(extra);
        Command::new(env!("CARGO_BIN_EXE_zenith"))
            .args(&args)
            .current_dir(self.dir.path())
            .env("ZENITH_DATA_DIR", self.dir.path().join("data"))
            .output()
            .expect("run zenith")
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn json(out: &Output) -> serde_json::Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("tx json: {e}\n{}", stdout(out)))
}

const DUPLICATE_PAGE: &str =
    r#"{"ops":[{"op":"duplicate_page","page":"page.one","new_id":"page.three","id_suffix":".3"}]}"#;

#[test]
fn dry_run_duplicate_page_names_the_created_ids() {
    let out = Env::new().tx(DUPLICATE_PAGE, &[]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(
        text.contains("\ncreated: page.three, rect.bg1.3, text.one.3\n"),
        "{text}"
    );
}

#[test]
fn json_has_a_created_array() {
    let v = json(&Env::new().tx(DUPLICATE_PAGE, &["--apply", "--json"]));
    assert_eq!(
        v["created"],
        serde_json::json!(["page.three", "rect.bg1.3", "text.one.3"])
    );
}

#[test]
fn duplicate_node_names_the_copy() {
    let v = json(&Env::new().tx(
        r#"{"ops":[{"op":"duplicate_node","node":"rect.bg1","new_id":"rect.copy"}]}"#,
        &["--json"],
    ));
    assert_eq!(v["created"], serde_json::json!(["rect.copy"]));
}

#[test]
fn other_ops_print_no_created_ids() {
    let v = json(&Env::new().tx(
        r#"{"ops":[{"op":"set_visible","node":"rect.bg1","visible":false}]}"#,
        &["--json"],
    ));
    assert!(v.get("created").is_none(), "{v:#}");
}

#[test]
fn unknown_node_suggests_the_closest_ids() {
    let out = Env::new().tx(
        r#"{"ops":[{"op":"set_visible","node":"rect.bg","visible":false}]}"#,
        &[],
    );
    assert_eq!(out.status.code(), Some(1));
    let text = stdout(&out);
    assert!(text.contains("did you mean: rect.bg1, rect.bg2"), "{text}");
}

#[test]
fn unknown_node_far_from_every_id_has_no_hint() {
    let out = Env::new().tx(
        r#"{"ops":[{"op":"set_visible","node":"nothing.alike.at.all","visible":false}]}"#,
        &[],
    );
    assert_eq!(out.status.code(), Some(1));
    let text = stdout(&out);
    assert!(text.contains("tx.unknown_node"), "{text}");
    assert!(!text.contains("did you mean"), "{text}");
}
