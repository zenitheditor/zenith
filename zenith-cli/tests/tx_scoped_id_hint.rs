//! `tx.unknown_node` on a scoped id `<instance>/<child>`, as a node-scoped
//! diagnostic names a component child, hints both working fixes.

use std::process::{Command, Output};

const DOC: &str = r##"zenith version=1 {
  project id="proj" name="T"
  tokens format="zenith-token-v1" { }
  styles { }
  components {
    component id="card" {
      text id="card.body" x=(px)0 y=(px)0 w=(px)60 h=(px)12 { span "Body" }
    }
  }
  document id="doc" title="T" {
    page id="p2" w=(px)200 h=(px)100 {
      instance id="p2.c1" component="card"
      instance id="p2.c2" x=(px)0 y=(px)30 component="card" {
        override ref="card.body" { span "A very long body text that cannot fit in the small box" }
      }
    }
  }
}"##;

fn run(args: &[&str], dir: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_zenith"))
        .args(args)
        .current_dir(dir)
        .env("ZENITH_DATA_DIR", dir.join("data"))
        .output()
        .expect("run zenith")
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn scoped_id_from_a_diagnostic_gets_a_two_fix_hint() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("t.zen"), DOC).expect("write doc");

    let validate = text(&run(&["validate", "t.zen"], dir.path()));
    assert!(validate.contains("p2.c2/card.body"), "{validate}");

    std::fs::write(
        dir.path().join("tx.json"),
        r#"{"ops":[{"op":"set_geometry","node":"p2.c2/card.body","w":200}]}"#,
    )
    .expect("write tx");
    let out = text(&run(&["tx", "t.zen", "tx.json"], dir.path()));
    assert!(out.contains("tx.unknown_node"), "{out}");
    assert!(
        out.contains(r#"set_override with "instance":"p2.c2" and "ref":"card.body""#),
        "{out}"
    );
    assert!(out.contains(r#"use the node id "card.body""#), "{out}");
}
