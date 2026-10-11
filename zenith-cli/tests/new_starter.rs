//! Integration tests for `zenith new --starter <kind>`.
//!
//! Runs the `zenith` binary in a temp project directory with an isolated
//! `ZENITH_DATA_DIR`, the same harness pattern as `tests/common/mod.rs`. Every
//! starter must write, validate with no diagnostics at its natural size and
//! default theme, render every page at its size, and carry real text. Each
//! starter also combines with `--theme` and the size flags: those combinations
//! must have no Error diagnostic and render at the requested size. `--name`
//! retitles the document, `--pages` conflicts, and an unknown kind lists the
//! kinds.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use clap::ValueEnum as _;
use tempfile::TempDir;
use zenith_cli::commands::new::Starter;
use zenith_core::{KdlAdapter, KdlSource as _, Node};

/// A temp project directory and an isolated data directory.
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

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// Run the `zenith` binary in the temp directory.
    fn zenith(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_zenith"))
            .args(args)
            .current_dir(self.dir.path())
            .env("ZENITH_DATA_DIR", self.data.path())
            .output()
            .expect("run zenith")
    }

    /// `zenith new <kind>.zen --starter <kind> [extra]`, asserting success.
    fn scaffold(&self, starter: Starter, extra: &[&str]) -> PathBuf {
        let file = format!("{}.zen", starter.kind());
        let mut args = vec!["new", file.as_str(), "--starter", starter.kind()];
        args.extend_from_slice(extra);
        let out = self.zenith(&args);
        assert!(
            out.status.success(),
            "{}: new failed: {}",
            starter.kind(),
            stderr(&out)
        );
        self.path(&file)
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn json(out: &Output) -> serde_json::Value {
    serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("json: {e}\n{}{}", stdout(out), stderr(out)))
}

fn parse(path: &Path) -> zenith_core::Document {
    let bytes = std::fs::read(path).expect("read starter");
    KdlAdapter.parse(&bytes).expect("starter parses")
}

/// `(width, height)` from a PNG's IHDR chunk.
fn png_size(path: &Path) -> (u32, u32) {
    let bytes = std::fs::read(path).expect("read png");
    assert_eq!(
        &bytes[..8],
        b"\x89PNG\r\n\x1a\n",
        "{} is a PNG",
        path.display()
    );
    let be = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().expect("4 bytes"));
    (be(16), be(20))
}

/// Every span string under `nodes`, depth first.
fn collect_text(nodes: &[Node], out: &mut Vec<String>) {
    for node in nodes {
        if let Node::Text(text) = node {
            out.extend(text.spans.iter().map(|s| s.text.clone()));
        }
        if let Node::Table(table) = node {
            for cell in table.rows.iter().flat_map(|row| &row.cells) {
                collect_text(&cell.children, out);
            }
        }
        if let Some(children) = node.children() {
            collect_text(children, out);
        }
    }
}

#[test]
fn every_starter_writes_with_a_doc_id_and_validates_clean() {
    let env = Env::new();
    for &starter in Starter::value_variants() {
        let path = env.scaffold(starter, &[]);
        let doc = parse(&path);
        assert_eq!(
            doc.doc_id.as_deref().map(str::len),
            Some(26),
            "{}: doc-id is a minted ULID",
            starter.kind()
        );
        let out = env.zenith(&["validate", path.to_str().expect("utf-8"), "--json"]);
        let report = json(&out);
        assert_eq!(report["valid"], true, "{}: {report}", starter.kind());
        assert_eq!(
            report["diagnostics"],
            serde_json::json!([]),
            "{}: diagnostics must be empty",
            starter.kind()
        );
        assert!(out.status.success(), "{}", starter.kind());
    }
}

#[test]
fn every_starter_renders_all_pages_at_its_size() {
    let env = Env::new();
    for &starter in Starter::value_variants() {
        let path = env.scaffold(starter, &[]);
        let dir = env.path(&format!("out-{}", starter.kind()));
        let out = env.zenith(&[
            "render",
            path.to_str().expect("utf-8"),
            "--all-pages",
            dir.to_str().expect("utf-8"),
        ]);
        assert!(
            out.status.success(),
            "{}: render failed: {}",
            starter.kind(),
            stderr(&out)
        );
        let mut pngs: Vec<PathBuf> = std::fs::read_dir(&dir)
            .expect("read out dir")
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|e| e == "png"))
            .collect();
        pngs.sort();
        assert_eq!(
            pngs.len(),
            starter.page_count() as usize,
            "{}: page count",
            starter.kind()
        );
        for png in &pngs {
            assert_eq!(png_size(png), starter.page_size(), "{}", png.display());
        }
    }
}

#[test]
fn every_starter_carries_real_text() {
    let env = Env::new();
    for &starter in Starter::value_variants() {
        let doc = parse(&env.scaffold(starter, &[]));
        let mut text = Vec::new();
        for page in &doc.body.pages {
            collect_text(&page.children, &mut text);
        }
        let chars: usize = text.iter().map(|s| s.chars().count()).sum();
        assert!(
            text.len() >= 6 && chars >= 100,
            "{}: {} spans, {chars} chars",
            starter.kind(),
            text.len()
        );
    }
}

#[test]
fn name_retitles_the_starter() {
    let env = Env::new();
    let path = env.scaffold(Starter::Menu, &["--name", "Harbour Kitchen"]);
    let doc = parse(&path);
    let project = doc.project.expect("project");
    assert_eq!(project.name, "Harbour Kitchen");
    assert_eq!(project.id, "proj.harbour-kitchen");
    assert_eq!(doc.body.id, "doc.harbour-kitchen");
    assert_eq!(doc.body.title.as_deref(), Some("Harbour Kitchen"));
    let out = env.zenith(&["validate", path.to_str().expect("utf-8"), "--json"]);
    assert_eq!(json(&out)["diagnostics"], serde_json::json!([]));
}

#[test]
fn without_name_the_ids_come_from_the_file_and_the_title_is_untitled() {
    let env = Env::new();
    let out = env.zenith(&["new", "My Spring Flyer.zen", "--starter", "card"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let doc = parse(&env.path("My Spring Flyer.zen"));
    assert_eq!(doc.body.id, "doc.my-spring-flyer");
    assert_eq!(doc.body.title.as_deref(), Some("Untitled"));
    let project = doc.project.expect("project");
    assert_eq!(project.id, "proj.my-spring-flyer");
    assert_eq!(project.name, "Untitled");
    // The printed commands quote a path with a space.
    let text = String::from_utf8(out.stdout).expect("utf8");
    assert!(
        text.contains("zenith tx 'My Spring Flyer.zen' --apply"),
        "{text}"
    );
}

#[test]
fn pages_conflicts_with_starter() {
    let env = Env::new();
    let out = env.zenith(&["new", "c.zen", "--starter", "flyer", "--pages", "2"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("cannot be used with"));
    assert!(!env.path("c.zen").exists());
}

/// The flags of one combination and the page size it must render at, or
/// `None` for the starter's natural size.
type Combo = (&'static [&'static str], Option<(u32, u32)>);

/// Every off-natural combination except `--landscape`, which depends on the
/// starter's own size: the wider side becomes the width.
const COMBOS: &[Combo] = &[
    (&["--format", "a4"], Some((794, 1123))),
    (&["--format", "a5"], Some((559, 794))),
    (&["--format", "letter"], Some((816, 1056))),
    (&["--width", "1080", "--height", "1080"], Some((1080, 1080))),
    (&["--theme", "cobalt"], None),
    (&["--theme", "sunset"], None),
    (&["--theme", "pine"], None),
    (&["--theme", "sunset", "--format", "a4"], Some((794, 1123))),
];

/// Every starter combines with themes and size flags: no Error diagnostic,
/// and every page renders at the requested size. The diagnostic count of
/// each combination is printed (`--nocapture`) as a record.
#[test]
fn every_starter_combines_with_theme_and_size_flags() {
    let env = Env::new();
    let landscape: &[&str] = &["--landscape"];
    for &starter in Starter::value_variants() {
        let (w, h) = starter.page_size();
        let mut combos: Vec<(&[&str], (u32, u32))> = COMBOS
            .iter()
            .map(|(flags, size)| (*flags, size.unwrap_or((w, h))))
            .collect();
        combos.push((landscape, (w.max(h), w.min(h))));
        for (i, (flags, size)) in combos.into_iter().enumerate() {
            let file = format!("{}-{i}.zen", starter.kind());
            let mut args = vec!["new", file.as_str(), "--starter", starter.kind()];
            args.extend_from_slice(flags);
            let out = env.zenith(&args);
            assert!(out.status.success(), "{args:?}: {}", stderr(&out));

            let report = json(&env.zenith(&["validate", file.as_str(), "--json"]));
            let diagnostics = report["diagnostics"].as_array().expect("diagnostics array");
            let errors: Vec<&serde_json::Value> = diagnostics
                .iter()
                .filter(|d| d["severity"] == "error")
                .collect();
            assert!(errors.is_empty(), "{args:?}: errors {errors:?}");
            let codes: Vec<&str> = diagnostics
                .iter()
                .filter_map(|d| d["code"].as_str())
                .collect();
            eprintln!(
                "{} {flags:?}: {} diagnostics {codes:?}",
                starter.kind(),
                codes.len()
            );

            let dir = format!("out-{}-{i}", starter.kind());
            let out = env.zenith(&["render", file.as_str(), "--all-pages", dir.as_str()]);
            assert!(out.status.success(), "{args:?}: render {}", stderr(&out));
            for n in 1..=starter.page_count() {
                let png = env.path(&format!("{dir}/page-{n}.png"));
                assert_eq!(png_size(&png), size, "{args:?} page {n}");
            }
        }
    }
}

#[test]
fn unknown_kind_exits_2_and_lists_the_kinds() {
    let env = Env::new();
    let out = env.zenith(&["new", "x.zen", "--starter", "brochure"]);
    assert_eq!(out.status.code(), Some(2));
    let err = stderr(&out);
    for &starter in Starter::value_variants() {
        assert!(
            err.contains(starter.kind()),
            "{} missing: {err}",
            starter.kind()
        );
    }
    assert!(!env.path("x.zen").exists());
}

#[test]
fn starter_refuses_to_overwrite() {
    let env = Env::new();
    std::fs::write(env.path("card.zen"), b"ORIGINAL").expect("write");
    let out = env.zenith(&["new", "card.zen", "--starter", "card"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(
        std::fs::read(env.path("card.zen")).expect("read"),
        b"ORIGINAL"
    );
}

#[test]
fn hints_point_at_starters_and_say_how_to_adapt_one() {
    let dir = tempfile::tempdir().expect("tempdir");
    let run = |args: &[&str]| {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_zenith"))
            .args(args)
            .current_dir(dir.path())
            .env("ZENITH_DATA_DIR", dir.path().join("data"))
            .output()
            .expect("run zenith");
        assert_eq!(out.status.code(), Some(0), "{args:?}");
        String::from_utf8(out.stdout).expect("utf8")
    };
    let blank = run(&["new", "blank.zen"]);
    assert!(blank.contains("--starter flyer|"), "{blank}");
    let themed = run(&["new", "themed.zen", "--theme", "cobalt"]);
    assert!(!themed.contains("tip:"), "{themed}");
    let starter = run(&["new", "s.zen", "--starter", "card"]);
    assert!(
        starter.contains("finished layout") && starter.contains("--ops"),
        "{starter}"
    );
    let schema = run(&["schema"]);
    assert!(schema.contains("new <path> --starter <kind>"), "{schema}");
}

#[test]
fn starter_output_lists_the_text_slots_and_the_batched_edit() {
    for kind in [
        "flyer",
        "letterhead",
        "handout",
        "deck",
        "poster",
        "menu",
        "card",
        "report",
    ] {
        let dir = tempfile::tempdir().expect("tempdir");
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_zenith"))
            .args(["new", "d.zen", "--starter", kind])
            .current_dir(dir.path())
            .env("ZENITH_DATA_DIR", dir.path().join("data"))
            .output()
            .expect("run zenith");
        assert_eq!(out.status.code(), Some(0), "{kind}");
        let text = String::from_utf8(out.stdout).expect("utf8");
        assert!(
            text.contains("text slots (node id: current text)"),
            "{kind}: {text}"
        );
        assert!(text.contains("\"op\":\"set_span_text\""), "{kind}: {text}");
        assert!(text.contains("<new text>"), "{kind}: {text}");
        assert!(text.contains("\"op\":\"set_fill\""), "{kind}: {text}");
        let slot_lines = text
            .lines()
            .filter(|l| l.starts_with("  ") && l.contains(": \""))
            .count();
        assert!(slot_lines >= 4, "{kind} lists {slot_lines} slots: {text}");
    }
}

#[test]
fn top_level_help_leads_with_every_starter() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_zenith"))
        .arg("--help")
        .output()
        .expect("run zenith");
    let text = String::from_utf8(out.stdout).expect("utf8");
    for kind in [
        "flyer",
        "letterhead",
        "handout",
        "deck",
        "poster",
        "menu",
        "card",
        "report",
    ] {
        assert!(text.contains(kind), "help omits starter {kind}: {text}");
    }
    let starter = text
        .find("--starter flyer")
        .expect("quick start names a starter");
    let blank = text
        .find("--name \"My Poster\"")
        .expect("quick start names a blank page");
    assert!(
        starter < blank,
        "the starter comes before the blank page: {text}"
    );
}
