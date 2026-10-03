//! Shared harness for the `zenith tx` integration tests: a temp project
//! directory with one document, and a runner for the `zenith` binary.

use std::path::PathBuf;
use std::process::{Command, Output};

use tempfile::TempDir;

/// The document name inside the temp directory.
pub const DOC: &str = "t.zen";

/// A temp project directory and an isolated data directory.
pub struct Env {
    dir: TempDir,
    data: TempDir,
}

impl Env {
    /// A fresh environment with `t.zen` holding `src`.
    pub fn with_doc(src: &str) -> Self {
        let env = Self {
            dir: TempDir::new().expect("tempdir"),
            data: TempDir::new().expect("data tempdir"),
        };
        std::fs::write(env.path(DOC), src).expect("write doc");
        env
    }

    /// The path of `name` inside the temp directory.
    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// Write `tx.json` and run `zenith tx t.zen tx.json <extra>`.
    pub fn tx(&self, tx_json: &str, extra: &[&str]) -> Output {
        std::fs::write(self.path("tx.json"), tx_json).expect("write tx");
        let mut args = vec!["tx", DOC, "tx.json"];
        args.extend_from_slice(extra);
        self.zenith(&args)
    }

    /// Run the `zenith` binary in the temp directory.
    pub fn zenith(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_zenith"))
            .args(args)
            .current_dir(self.dir.path())
            .env("ZENITH_DATA_DIR", self.data.path())
            .output()
            .expect("run zenith")
    }

    /// The current text of `t.zen`.
    pub fn read(&self) -> String {
        std::fs::read_to_string(self.path(DOC)).expect("read doc")
    }
}

/// Stdout as text.
pub fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Parse stdout as JSON.
pub fn json(out: &Output) -> serde_json::Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("tx json: {e}\n{}", stdout(out)))
}

/// A document with a row frame `cards.row2` holding `c1` and `c2`, a plain
/// frame `free`, and a loose card `card.3`.
pub const CARDS: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.k" type="color" value="#000000"
    token id="color.w" type="color" value="#ffffff"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)800 h=(px)600 {
      frame id="cards.row2" x=(px)40 y=(px)300 w=(px)720 h=(px)140 layout="row" gap=(px)8 {
        rect id="c1" w=(px)200 h=(px)120 fill=(token)"color.k"
        rect id="c2" w=(px)200 h=(px)120 fill=(token)"color.k"
      }
      frame id="free" x=(px)40 y=(px)40 w=(px)400 h=(px)200 {
        rect id="f1" x=(px)10 y=(px)10 w=(px)50 h=(px)50 fill=(token)"color.k"
      }
      rect id="card.3" x=(px)526 y=(px)40 w=(px)219 h=(px)120 fill=(token)"color.k"
    }
  }
}
"##;

/// Reparent `card.3` to the front of the row frame.
pub const INTO_ROW: &str = r#"{"ops":[{"op":"reparent","node":"card.3","new_parent":"cards.row2","position":{"at":"first"}}]}"#;
