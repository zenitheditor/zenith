//! `zenith fix` removes attributes with no effect: `validate --json` carries a
//! `remove_property` fix per ignored attribute, and `fix --apply` removes
//! them.

use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

/// An in-flow chip with stale x/y, and a free rect with an inert `min-w`.
const DOC: &str = r##"zenith version=1 {
  project id="proj.r" name="Remove"
  tokens format="zenith-token-v1" {
    token id="color.k" type="color" value="#000000"
  }
  styles {
  }
  document id="doc.r" title="Remove" {
    page id="page.r" w=(px)400 h=(px)400 {
      frame id="row" x=(px)10 y=(px)10 w=(px)300 h=(px)100 layout="row" gap=(px)8 {
        rect id="chip" x=(px)40 y=(px)60 w=(px)50 h=(px)20 fill=(token)"color.k"
      }
      rect id="free" x=(px)10 y=(px)200 w=(px)50 h=(px)50 min-w=(px)20 fill=(token)"color.k"
    }
  }
}
"##;

struct Env {
    dir: TempDir,
    data: TempDir,
}

impl Env {
    fn new() -> Self {
        let env = Self {
            dir: TempDir::new().expect("tempdir"),
            data: TempDir::new().expect("data tempdir"),
        };
        std::fs::write(env.dir.path().join("t.zen"), DOC).expect("write doc");
        env
    }

    fn zenith(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_zenith"))
            .args(args)
            .current_dir(self.dir.path())
            .env("ZENITH_DATA_DIR", self.data.path())
            .output()
            .expect("run zenith")
    }

    fn json(&self, args: &[&str]) -> Value {
        let out = self.zenith(args);
        serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
            panic!(
                "json: {e}\n{}\n{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            )
        })
    }

    fn read(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("t.zen")).expect("read doc")
    }
}

/// `(code, subject, fix)` for every diagnostic with `code` in `codes`.
fn fixes(report: &Value, codes: &[&str]) -> Vec<(String, String, Value)> {
    report["diagnostics"]
        .as_array()
        .expect("diagnostics array")
        .iter()
        .filter(|d| codes.contains(&d["code"].as_str().unwrap_or_default()))
        .map(|d| {
            (
                d["code"].as_str().unwrap_or_default().to_owned(),
                d["subject_id"].as_str().unwrap_or_default().to_owned(),
                d["fix"].clone(),
            )
        })
        .collect()
}

const CODES: &[&str] = &["layout.position_ignored", "layout.inert_attribute"];

#[test]
fn validate_json_carries_one_remove_property_fix_per_attribute() {
    let env = Env::new();
    let report = env.json(&["validate", "t.zen", "--json"]);
    let remove = |p: &str| serde_json::json!({"kind": "remove_property", "property": p});
    let mut got = fixes(&report, CODES);
    got.sort_by(|a, b| (&a.0, &a.1, a.2.to_string()).cmp(&(&b.0, &b.1, b.2.to_string())));
    assert_eq!(
        got,
        [
            (
                "layout.inert_attribute".to_owned(),
                "free".to_owned(),
                remove("min-w")
            ),
            (
                "layout.position_ignored".to_owned(),
                "chip".to_owned(),
                remove("x")
            ),
            (
                "layout.position_ignored".to_owned(),
                "chip".to_owned(),
                remove("y")
            ),
        ],
        "{report:#}"
    );
}

#[test]
fn fix_apply_removes_ignored_xy_and_inert_attribute() {
    let env = Env::new();
    let report = env.json(&["fix", "t.zen", "--json", "--apply"]);
    let applied: Vec<(String, String, String)> = report["applied"]
        .as_array()
        .expect("applied array")
        .iter()
        .map(|f| {
            (
                f["subject_id"].as_str().unwrap_or_default().to_owned(),
                f["property"].as_str().unwrap_or_default().to_owned(),
                f["to"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    assert_eq!(applied.len(), 3, "{report:#}");
    for (subject, property) in [("chip", "x"), ("chip", "y"), ("free", "min-w")] {
        assert!(
            applied.contains(&(
                subject.to_owned(),
                property.to_owned(),
                "(removed)".to_owned()
            )),
            "{subject} {property}: {report:#}"
        );
    }

    let src = env.read();
    let chip = src
        .lines()
        .find(|l| l.contains("id=\"chip\""))
        .expect("chip line");
    assert!(!chip.contains("x=") && !chip.contains("y="), "{chip}");
    assert!(!src.contains("min-w"), "{src}");

    let after = env.json(&["validate", "t.zen", "--json"]);
    assert!(fixes(&after, CODES).is_empty(), "{after:#}");
}

#[test]
fn fix_human_output_names_each_removal() {
    let env = Env::new();
    let out = env.zenith(&["fix", "t.zen"]);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("layout.position_ignored (chip) x: (px)40 → (removed)"),
        "{text}"
    );
    assert!(
        text.contains("layout.inert_attribute (free) min-w: (px)20 → (removed)"),
        "{text}"
    );
    assert_eq!(env.read(), DOC, "a dry-run writes nothing");
}
