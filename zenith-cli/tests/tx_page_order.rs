//! `zenith tx` prints the page order before and after an op that adds,
//! removes, or moves pages, in human and `--json` output. `zenith tx --help`
//! carries runnable examples and no pointer to files an agent cannot see.

use std::process::{Command, Output};

/// A temp directory holding `t.zen`.
struct Env {
    dir: tempfile::TempDir,
}

impl Env {
    fn with_doc(src: &str) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("t.zen"), src).expect("write doc");
        Self { dir }
    }

    fn zenith(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_zenith"))
            .args(args)
            .current_dir(self.dir.path())
            .env("ZENITH_DATA_DIR", self.dir.path().join("data"))
            .output()
            .expect("run zenith")
    }

    fn tx(&self, tx_json: &str, extra: &[&str]) -> Output {
        std::fs::write(self.dir.path().join("tx.json"), tx_json).expect("write tx");
        let mut args = vec!["tx", "t.zen", "tx.json"];
        args.extend_from_slice(extra);
        self.zenith(&args)
    }

    fn read(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("t.zen")).expect("read doc")
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn json(out: &Output) -> serde_json::Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("tx json: {e}\n{}", stdout(out)))
}

const PAGES: &str = r##"zenith version=1 {
  project id="proj" name="P"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="d" title="P" {
    page id="p1" w=(px)100 h=(px)100 { }
    page id="p2" w=(px)100 h=(px)100 { }
    page id="p3" w=(px)100 h=(px)100 { }
  }
}
"##;

#[test]
fn human_output_shows_the_page_order_change() {
    let env = Env::with_doc(PAGES);
    let out = env.tx(r#"{"ops":[{"op":"swap_pages","a":"p1","b":"p3"}]}"#, &[]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(
        text.contains("\npages: p1, p2, p3 -> p3, p2, p1\n"),
        "{text}"
    );
}

#[test]
fn json_output_has_before_and_after() {
    let env = Env::with_doc(PAGES);
    let v = json(&env.tx(
        r#"{"ops":[{"op":"move_page","page":"p1","position":{"at":"last"}}]}"#,
        &["--apply", "--json"],
    ));
    assert_eq!(v["pages"]["before"], serde_json::json!(["p1", "p2", "p3"]));
    assert_eq!(v["pages"]["after"], serde_json::json!(["p2", "p3", "p1"]));
    assert!(env.read().find("p2").expect("p2") < env.read().find("p1").expect("p1"));
}

#[test]
fn a_rejected_page_op_shows_the_unchanged_order() {
    let env = Env::with_doc(PAGES);
    let out = env.tx(r#"{"ops":[{"op":"swap_pages","a":"p1","b":"p9"}]}"#, &[]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stdout(&out).contains("pages: p1, p2, p3 (unchanged)"));
}

#[test]
fn node_ops_print_no_page_order() {
    let env = Env::with_doc(PAGES);
    let v = json(&env.tx(
        r#"{"ops":[{"op":"set_page_size","page":"p1","w":"(px)200","h":"(px)100"}]}"#,
        &["--json"],
    ));
    assert!(v.get("pages").is_none(), "{v:#}");
}

#[test]
fn help_inlines_examples_and_names_no_example_files() {
    let env = Env::with_doc(PAGES);
    let help = stdout(&env.zenith(&["tx", "--help"]));
    assert!(!help.contains("examples/"), "{help}");
    for want in [
        "\"op\":\"move_page\"",
        "\"op\":\"set_override\"",
        "--page 3",
    ] {
        assert!(help.contains(want), "help lacks {want}");
    }
}
