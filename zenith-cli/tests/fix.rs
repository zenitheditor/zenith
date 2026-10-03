//! Integration tests for `zenith fix`: a loose agent draft reaches a valid
//! document in one step, the result is canonical and idempotent, dry-run
//! writes nothing, and output is deterministic.

use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

/// The draft body an agent writes into a fresh `--theme cobalt` page.
const DRAFT: &str = r##"      rect id="card" x=(px)80 y=(px)80 w=(px)920 h=(px)400 fill="#ffffff" radius=(px)24
      text id="title" x=(px)120 y=(px)120 w=(px)400 h=(px)60 fill=(token)"color.base.900" font-size=(px)96 font-wieght=700 { span "A very long headline that overflows" }
      shape id="cta" kind="process" x=(px)120 y=(px)400 w=(px)240 h=(px)64 fill=(token)"color.primary.500" { span "Buy now" }
"##;

struct Env {
    dir: TempDir,
    data: TempDir,
}

impl Env {
    fn new() -> Self {
        Self {
            dir: TempDir::new().expect("tempdir"),
            data: TempDir::new().expect("data tempdir"),
        }
    }

    fn doc(&self) -> std::path::PathBuf {
        self.dir.path().join("t.zen")
    }

    fn zenith(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_zenith"))
            .args(args)
            .current_dir(self.dir.path())
            .env("ZENITH_DATA_DIR", self.data.path())
            .output()
            .expect("run zenith")
    }

    /// Scaffold `t.zen` with `zenith new --theme cobalt`, then add the draft.
    fn draft(&self) {
        let out = self.zenith(&["new", "t.zen", "--theme", "cobalt"]);
        assert!(out.status.success(), "{}", stderr(&out));
        let src = std::fs::read_to_string(self.doc()).expect("read scaffold");
        let page_line_end = src
            .find("page id=")
            .and_then(|i| src[i..].find('\n').map(|j| i + j + 1))
            .expect("scaffold has a page");
        let mut drafted = src.clone();
        drafted.insert_str(page_line_end, DRAFT);
        std::fs::write(self.doc(), drafted).expect("write draft");
    }

    fn read(&self) -> String {
        std::fs::read_to_string(self.doc()).expect("read doc")
    }

    fn validate_json(&self) -> Value {
        let out = self.zenith(&["validate", "t.zen", "--json"]);
        serde_json::from_slice(&out.stdout).expect("validate json")
    }

    fn fix_json(&self, apply: bool) -> Value {
        let mut args = vec!["fix", "t.zen", "--json"];
        if apply {
            args.push("--apply");
        }
        let out = self.zenith(&args);
        serde_json::from_slice(&out.stdout)
            .unwrap_or_else(|e| panic!("fix json: {e}\n{}", stdout(&out)))
    }
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn errors(diags: &Value) -> Vec<(String, String)> {
    diags
        .as_array()
        .expect("diagnostics array")
        .iter()
        .filter(|d| d["severity"] == "error")
        .map(|d| {
            (
                d["code"].as_str().unwrap_or_default().to_owned(),
                d["subject_id"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

fn applied_targets(report: &Value) -> Vec<(String, String, String)> {
    report["applied"]
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
        .collect()
}

#[test]
fn draft_has_six_errors_before_fix() {
    let env = Env::new();
    env.draft();
    let report = env.validate_json();
    assert_eq!(errors(&report["diagnostics"]).len(), 6, "{report:#}");
}

#[test]
fn apply_fixes_draft_leaving_only_the_ambiguous_reference() {
    let env = Env::new();
    env.draft();
    let report = env.fix_json(true);

    let mut applied = applied_targets(&report);
    applied.sort();
    let expected: Vec<(String, String, String)> = [
        ("card", "fill", "(token)\"color.custom.ffffff\""),
        ("card", "radius", "(token)\"radius.24\""),
        ("cta", "fill", "(token)\"color.primary\""),
        ("title", "font-size", "(token)\"size.96\""),
        ("title", "font-weight", "(token)\"font.weight.heading\""),
        ("title", "font-wieght", "font-weight"),
    ]
    .iter()
    .map(|(a, b, c)| ((*a).to_owned(), (*b).to_owned(), (*c).to_owned()))
    .collect();
    assert_eq!(applied, expected, "{report:#}");

    // `color.base.900` ties three ids at one edit: it stays for the author.
    assert_eq!(
        errors(&report["remaining"]),
        vec![("token.unknown_reference".to_owned(), "title".to_owned())],
        "{report:#}"
    );
    let after = env.validate_json();
    assert_eq!(
        errors(&after["diagnostics"]),
        vec![("token.unknown_reference".to_owned(), "title".to_owned())],
        "{after:#}"
    );
    assert_eq!(
        report["remaining"], after["diagnostics"],
        "fix remaining must match validate"
    );

    let src = env.read();
    for minted in [
        r##"token id="color.custom.ffffff" type="color" value="#ffffff""##,
        r#"token id="radius.24" type="dimension" value=(px)24"#,
        r#"token id="size.96" type="dimension" value=(px)96"#,
    ] {
        assert!(src.contains(minted), "missing {minted}\n{src}");
    }
    // The theme ships `font.weight.heading` (700), so the weight reuses it.
    assert!(!src.contains("weight.700"), "{src}");
    // Minted tokens sit with their group.
    let pos = |needle: &str| src.find(needle).expect(needle);
    assert!(pos("color.custom.ffffff") < pos("radius.box"), "{src}");
    assert!(pos("radius.24") < pos("border.width"), "{src}");
    assert!(pos("size.96") > pos("size.caption"), "{src}");
}

#[test]
fn applied_result_is_canonical_and_idempotent() {
    let env = Env::new();
    env.draft();
    env.fix_json(true);
    let first = env.read();

    let fmt = env.zenith(&["fmt", "t.zen", "--json"]);
    let fmt: Value = serde_json::from_slice(&fmt.stdout).expect("fmt json");
    assert_eq!(fmt["changed"], false, "fix output must be canonical");

    let again = env.fix_json(true);
    assert_eq!(again["applied"], Value::Array(Vec::new()), "{again:#}");
    assert_eq!(env.read(), first, "second run must not change the file");
}

#[test]
fn dry_run_leaves_file_unchanged_and_prints_diff() {
    let env = Env::new();
    env.draft();
    let before = env.read();
    let out = env.zenith(&["fix", "t.zen"]);
    assert_eq!(env.read(), before);
    let text = stdout(&out);
    assert!(text.contains("dry-run"), "{text}");
    assert!(text.contains("--- a/t.zen"), "{text}");
    assert!(text.contains("+++ b/t.zen"), "{text}");
    assert!(
        text.contains(r##"+    token id="color.custom.ffffff" type="color" value="#ffffff""##),
        "{text}"
    );
    assert_eq!(out.status.code(), Some(1), "an error remains");
}

#[test]
fn json_envelope_shape() {
    let env = Env::new();
    env.draft();
    let report = env.fix_json(false);
    assert_eq!(report["schema"], "zenith-fix-v1");
    let applied = report["applied"].as_array().expect("applied");
    assert!(!applied.is_empty());
    for fix in applied {
        let mut keys: Vec<&str> = fix
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["code", "from", "property", "subject_id", "to"],
            "{fix:#}"
        );
    }
    let rename = applied
        .iter()
        .find(|f| f["code"] == "node.unknown_property")
        .expect("rename fix");
    assert_eq!(rename["from"], "font-wieght");
    assert_eq!(rename["to"], "font-weight");
    let raw = applied
        .iter()
        .find(|f| f["subject_id"] == "card" && f["property"] == "fill")
        .expect("card fill fix");
    assert_eq!(raw["code"], "token.raw_visual_literal");
    assert_eq!(raw["from"], "\"#ffffff\"");
    assert!(report["remaining"].is_array());
}

#[test]
fn fix_is_deterministic() {
    let a = Env::new();
    a.draft();
    let b = Env::new();
    b.draft();
    // Same draft body; doc-ids differ, so compare two runs on one file and
    // the applied lists across files.
    let first = a.zenith(&["fix", "t.zen", "--json"]);
    let second = a.zenith(&["fix", "t.zen", "--json"]);
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(a.fix_json(false)["applied"], b.fix_json(false)["applied"]);

    a.fix_json(true);
    b.fix_json(true);
    let strip_id = |s: String| {
        s.lines()
            .filter(|l| !l.contains("doc-id=") && !l.contains("project id="))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(strip_id(a.read()), strip_id(b.read()));
}

#[test]
fn parse_error_exits_two() {
    let env = Env::new();
    std::fs::write(env.doc(), "zenith {{{").expect("write");
    let out = env.zenith(&["fix", "t.zen", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    let report: Value = serde_json::from_slice(&out.stdout).expect("error json");
    assert_eq!(
        report["diagnostics"][0]["code"], "parse.error",
        "{report:#}"
    );
    assert_eq!(
        std::fs::read_to_string(env.doc()).expect("read"),
        "zenith {{{",
        "a parse error writes nothing"
    );
}

#[test]
fn mcp_fix_tool_dry_runs_then_applies() {
    use zenith_cli::mcp::handle_message;

    let env = Env::new();
    env.draft();
    let before = env.read();
    let doc = env.doc().to_string_lossy().into_owned();
    let call = |apply: bool| -> Value {
        let line = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "zenith_fix", "arguments": { "doc": doc, "apply": apply } }
        });
        handle_message(&line.to_string()).expect("response")
    };

    let dry = call(false);
    let result = &dry["result"]["structuredContent"];
    assert_eq!(dry["result"]["isError"], false, "{dry:#}");
    assert_eq!(result["changed"], true, "{dry:#}");
    assert_eq!(result["applied"].as_array().map(Vec::len), Some(6));
    assert_eq!(result["error_count"], 1);
    assert_eq!(env.read(), before, "dry-run writes nothing");

    let applied = call(true);
    assert_eq!(applied["result"]["isError"], false, "{applied:#}");
    assert_ne!(env.read(), before);
    let again = call(true);
    assert_eq!(again["result"]["structuredContent"]["changed"], false);
}

#[test]
fn clean_fix_reports_compile_stage_diagnostics_like_validate() {
    let env = Env::new();
    env.draft();
    // Drop the one ambiguous reference so every error is machine-fixable.
    let src = env.read().replace("color.base.900", "color.base.content");
    std::fs::write(env.doc(), src).expect("write");

    let report = env.fix_json(true);
    assert!(errors(&report["remaining"]).is_empty(), "{report:#}");
    let after = env.validate_json();
    assert_eq!(report["remaining"], after["diagnostics"]);
    let codes: Vec<&str> = after["diagnostics"]
        .as_array()
        .expect("array")
        .iter()
        .filter_map(|d| d["code"].as_str())
        .collect();
    assert!(
        codes.contains(&"text.overflow"),
        "compile-stage checks run once no error remains: {codes:?}"
    );
}

#[test]
fn validate_json_carries_fix_hints() {
    let env = Env::new();
    env.draft();
    let report = env.validate_json();
    let diags = report["diagnostics"].as_array().expect("array");
    let fix_of = |subject: &str, code: &str| {
        diags
            .iter()
            .find(|d| d["subject_id"] == subject && d["code"] == code)
            .map(|d| d["fix"].clone())
            .unwrap_or(Value::Null)
    };
    let raw = diags
        .iter()
        .find(|d| {
            d["subject_id"] == "card" && d["message"].as_str().is_some_and(|m| m.contains("'fill'"))
        })
        .map(|d| d["fix"].clone())
        .expect("card fill diagnostic");
    assert_eq!(raw["kind"], "raw_literal", "{raw:#}");
    assert_eq!(raw["literal"], "#ffffff");
    assert_eq!(raw["token_type"], "color");
    assert_eq!(raw["exact_match"], Value::Null);
    assert_eq!(raw["nearest"], "color.base.100");
    let rename = fix_of("title", "node.unknown_property");
    assert_eq!(rename["kind"], "rename_property");
    assert_eq!(rename["to"], "font-weight");
    let token_ref = fix_of("cta", "token.unknown_reference");
    assert_eq!(token_ref["to"], "color.primary");
    assert_eq!(
        fix_of("title", "token.unknown_reference"),
        Value::Null,
        "an ambiguous reference carries no fix"
    );
}

/// Two headlines whose glyph ink collides.
const OVERLAP_DOC: &str = r##"zenith version=1 {
  project id="proj.o" name="Overlap"
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#111111"
    token id="color.paper" type="color" value="#ffffff"
    token id="size.type" type="dimension" value=(px)32
  }
  styles {}
  document id="doc.o" title="Overlap" {
    page id="p" w=(px)800 h=(px)600 background=(token)"color.paper" {
      text id="a" x=(px)40 y=(px)100 w=(px)600 h=(px)50 font-size=(token)"size.type" fill=(token)"color.ink" {
        span "Overlapping headline"
      }
      text id="b" x=(px)40 y=(px)110 w=(px)600 h=(px)50 font-size=(token)"size.type" fill=(token)"color.ink" {
        span "Overlapping headline"
      }
    }
  }
}
"##;

#[test]
fn fix_applies_the_compile_stage_ink_overlap_move() {
    let env = Env::new();
    std::fs::write(env.doc(), OVERLAP_DOC).expect("write doc");
    let before = env.validate_json();
    let hint = before["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .find(|d| d["code"] == "text.ink_overlap")
        .unwrap_or_else(|| panic!("{before:#}"));
    assert_eq!(hint["fix"]["kind"], "set_property", "{hint:#}");
    let to = hint["fix"]["to"].as_str().expect("to").to_owned();

    let report = env.fix_json(true);
    let applied = report["applied"].as_array().expect("applied");
    assert!(
        applied
            .iter()
            .any(|f| f["code"] == "text.ink_overlap" && f["subject_id"] == "b" && f["to"] == to),
        "{report:#}"
    );
    assert!(env.read().contains(&format!("y={to}")), "{}", env.read());
    let after = env.validate_json();
    assert!(
        after["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .all(|d| d["code"] != "text.ink_overlap"),
        "{after:#}"
    );
}
