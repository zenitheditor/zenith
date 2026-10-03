//! Integration tests for the theme print scale of `zenith new`.
//!
//! A print `--format` with `--theme` rescales the copied `size.*` and
//! `radius.*` tokens. No format and `--format square` keep the pack values.

use std::path::{Path, PathBuf};

use tempfile::TempDir;
use zenith_cli::commands::new::{self, PaperFormat, resolve_page};
use zenith_cli::commands::validate;
use zenith_cli::config::CliPolicyFlags;
use zenith_cli::library::resolve_theme_pack;
use zenith_core::ast::{Dimension, Token, TokenLiteral, TokenType, TokenValue, Unit};
use zenith_core::{Document, KdlAdapter, KdlSource as _};
use zenith_scene::sizes_read_as_one;
use zenith_session::StorePaths;

/// Scaffold `--theme theme` with `format` and return the path and source.
fn scaffold(tmp: &TempDir, theme: &str, format: Option<PaperFormat>) -> (PathBuf, String) {
    let paths = StorePaths::new(tmp.path());
    let path = tmp.path().join("doc.zen");
    let page = resolve_page(format, None, None, false, 1).expect("page resolves");
    new::run_in(&paths, &path, Some("Doc"), page, Some(theme)).expect("themed new");
    let src = std::fs::read_to_string(&path).expect("read scaffold");
    (path, src)
}

fn parse(src: &str) -> Document {
    KdlAdapter.parse(src.as_bytes()).expect("scaffold parses")
}

/// `(id, value)` of every token, in order.
fn token_values(doc: &Document) -> Vec<(String, TokenValue)> {
    doc.tokens
        .tokens
        .iter()
        .map(|t| (t.id.clone(), t.value.clone()))
        .collect()
}

fn px(doc: &Document, id: &str) -> f64 {
    let token = doc
        .tokens
        .tokens
        .iter()
        .find(|t| t.id == id)
        .unwrap_or_else(|| panic!("token {id} present"));
    match &token.value {
        TokenValue::Literal(TokenLiteral::Dimension(Dimension {
            value,
            unit: Unit::Px,
        })) => *value,
        other => panic!("{id}: expected a px literal, got {other:?}"),
    }
}

fn cobalt_pack() -> Document {
    resolve_theme_pack(None, "cobalt").expect("cobalt resolves")
}

/// The `size.*` ids of cobalt, smallest original value first.
const TYPE_IDS: [&str; 5] = [
    "size.caption",
    "size.body",
    "size.h2",
    "size.h1",
    "size.display",
];

/// The scaled `size.*` values of `doc`, in [`TYPE_IDS`] order.
fn type_steps(doc: &Document) -> Vec<f64> {
    TYPE_IDS.iter().map(|id| px(doc, id)).collect()
}

/// Diagnostic codes of `validate --json` on `src`; asserts exit code 0.
fn validate_codes(src: &str, dir: Option<&Path>) -> Vec<String> {
    let out = validate::run(src, dir, true, &CliPolicyFlags::default());
    assert_eq!(out.exit_code, 0, "validates; got:\n{}", out.stdout);
    let value: serde_json::Value = serde_json::from_str(&out.stdout).expect("json");
    value["diagnostics"]
        .as_array()
        .expect("diagnostics array")
        .iter()
        .filter_map(|d| d["code"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn cobalt_a4_has_a_16px_body_and_scaled_headings() {
    let tmp = TempDir::new().unwrap();
    let (_, src) = scaffold(&tmp, "cobalt", Some(PaperFormat::A4));
    let doc = parse(&src);

    assert_eq!(px(&doc, "size.body"), 16.0);
    assert_eq!(px(&doc, "size.display"), 64.0);
    assert_eq!(px(&doc, "size.h1"), 37.0);
    assert_eq!(px(&doc, "size.h2"), 23.0);
    assert_eq!(px(&doc, "size.caption"), 10.0);
    assert_eq!(px(&doc, "radius.box"), 24.0);
    assert_eq!(px(&doc, "radius.field"), 3.0);
    assert_eq!(px(&doc, "radius.selector"), 6.0);
    assert_eq!(px(&doc, "space.unit"), 4.0);
    assert_eq!(px(&doc, "border.width"), 1.0);
}

#[test]
fn no_format_and_square_keep_the_pack_tokens() {
    let pack = token_values(&cobalt_pack());
    for format in [None, Some(PaperFormat::Square)] {
        let tmp = TempDir::new().unwrap();
        let (_, src) = scaffold(&tmp, "cobalt", format);
        assert_eq!(
            token_values(&parse(&src)),
            pack,
            "{format:?}: tokens must equal the pack"
        );
    }
}

#[test]
fn no_format_and_square_scaffolds_are_byte_identical() {
    let strip = |src: &str| {
        let mut doc = parse(src);
        doc.doc_id = None;
        KdlAdapter.format(&doc).expect("format")
    };
    let tmp_a = TempDir::new().unwrap();
    let tmp_b = TempDir::new().unwrap();
    let (_, none) = scaffold(&tmp_a, "cobalt", None);
    let (_, square) = scaffold(&tmp_b, "cobalt", Some(PaperFormat::Square));
    assert_eq!(strip(&none), strip(&square));
}

#[test]
fn letter_and_a5_scale_with_the_short_side() {
    let pack = cobalt_pack();
    let cases = [
        (PaperFormat::A4, 794.0, [10.0, 16.0, 23.0, 37.0, 64.0]),
        (PaperFormat::Letter, 816.0, [11.0, 16.0, 23.0, 38.0, 66.0]),
        (PaperFormat::A5, 559.0, [9.0, 11.0, 16.0, 26.0, 45.0]),
    ];
    for (format, short, expected) in cases {
        let tmp = TempDir::new().unwrap();
        let (_, src) = scaffold(&tmp, "cobalt", Some(format));
        let doc = parse(&src);
        assert_eq!(type_steps(&doc), expected, "{format:?} type steps");
        let radius = (px(&pack, "radius.box") * short / 1080.0).round().max(1.0);
        assert_eq!(px(&doc, "radius.box"), radius, "{format:?} radius.box");
    }
}

#[test]
fn a5_steps_are_strictly_increasing_and_distinct() {
    let tmp = TempDir::new().unwrap();
    let (_, src) = scaffold(&tmp, "cobalt", Some(PaperFormat::A5));
    let steps = type_steps(&parse(&src));
    for pair in steps.windows(2) {
        if let [lo, hi] = pair {
            assert!(hi > lo, "steps must increase: {steps:?}");
            assert!(
                !sizes_read_as_one(*lo, *hi),
                "steps must read as distinct: {steps:?}"
            );
        }
    }
}

#[test]
fn a4_scaffold_validates_with_no_small_text() {
    let tmp = TempDir::new().unwrap();
    let (path, src) = scaffold(&tmp, "cobalt", Some(PaperFormat::A4));
    let codes = validate_codes(&src, path.parent());
    assert!(
        !codes.iter().any(|c| c == "text.too_small"),
        "no text.too_small; got {codes:?}"
    );
}

/// One text node per `size.*` token, stacked down the page.
fn sample_texts(doc: &Document) -> String {
    let mut out = String::new();
    let mut y = 20.0;
    for id in TYPE_IDS {
        let h = (px(doc, id) * 1.6).ceil();
        out.push_str(&format!(
            "      text id=\"t.{}\" x=(px)20 y=(px){y} w=(px)480 h=(px){h} font-size=(token)\"{id}\" {{ span \"Sample\" }}\n",
            id.replace('.', "-")
        ));
        y += h + 16.0;
    }
    out
}

/// Control: the same setup does report a near-duplicate pair, so the check
/// below is not vacuous.
#[test]
fn near_duplicate_sizes_are_reported_in_this_setup() {
    let tmp = TempDir::new().unwrap();
    let (path, src) = scaffold(&tmp, "cobalt", Some(PaperFormat::A4));
    // Add a 17 px token next to the 16 px body: 1 px apart reads as one size.
    let mut doc = parse(&src);
    doc.tokens.tokens.push(Token {
        id: "size.near".to_owned(),
        token_type: TokenType::Dimension,
        value: TokenValue::Literal(TokenLiteral::Dimension(Dimension {
            value: 17.0,
            unit: Unit::Px,
        })),
        set: None,
        source_span: None,
    });
    let src = String::from_utf8(KdlAdapter.format(&doc).expect("format")).expect("utf-8");
    let marker = "background=(token)\"color.base.100\" {\n";
    let at = src.find(marker).expect("page open") + marker.len();
    let mut full = src.clone();
    full.insert_str(
        at,
        "      text id=\"t.a\" x=(px)20 y=(px)20 w=(px)480 h=(px)40 font-size=(token)\"size.body\" { span \"Sample\" }\n      text id=\"t.b\" x=(px)20 y=(px)80 w=(px)480 h=(px)40 font-size=(token)\"size.near\" { span \"Sample\" }\n",
    );
    let codes = validate_codes(&full, path.parent());
    assert!(
        codes.iter().any(|c| c == "type.near_duplicate_size"),
        "control must report; got {codes:?}"
    );
}

#[test]
fn every_size_token_in_use_gives_no_small_or_duplicate_size() {
    for format in [PaperFormat::A4, PaperFormat::Letter, PaperFormat::A5] {
        let tmp = TempDir::new().unwrap();
        let (path, src) = scaffold(&tmp, "cobalt", Some(format));
        let marker = "background=(token)\"color.base.100\" {\n";
        let at = src.find(marker).expect("page open") + marker.len();
        let mut full = src.clone();
        full.insert_str(at, &sample_texts(&parse(&src)));
        let codes = validate_codes(&full, path.parent());
        for banned in ["text.too_small", "type.near_duplicate_size"] {
            assert!(
                !codes.iter().any(|c| c == banned),
                "{format:?}: no {banned}; got {codes:?}"
            );
        }
    }
}
