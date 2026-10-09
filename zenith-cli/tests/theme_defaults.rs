//! Integration tests for the styles and `defaults` every theme ships: each
//! embedded pack validates clean and its defaults resolve, `new --theme`,
//! `theme apply`, and `theme new` carry them, and a bare node on a themed
//! document renders on-theme with no contrast diagnostics.

use std::path::{Path, PathBuf};

use tempfile::TempDir;
use zenith_cli::commands::new::{self, DEFAULT_PAGE};
use zenith_cli::commands::theme::{
    DISPLAY_SIZE_TOKEN_ID, DISPLAY_SIZE_TOKEN_PX, DISPLAY_SIZE_TOKEN_TYPE, HEADING_WEIGHT_TOKEN_ID,
    HEADING_WEIGHT_TOKEN_TYPE, HEADING_WEIGHT_TOKEN_VALUE, Shape, THEME_DEFAULTS, THEME_STYLE_IDS,
    ThemeInput, apply_run, kit_document_source, new as theme_new,
};
use zenith_cli::commands::validate;
use zenith_cli::library::{EMBEDDED_PACKS, resolve_theme_pack};
use zenith_core::theme::Scheme;
use zenith_core::{DefaultsKind, Document, KdlAdapter, KdlSource as _, default_provider};
use zenith_pipeline::PolicyFlags;
use zenith_session::StorePaths;

const STYLE_IDS: &[&str] = THEME_STYLE_IDS;

/// `(kind, style, text-style)` for every document `defaults` entry a theme ships.
fn expected_defaults() -> Vec<(DefaultsKind, &'static str, Option<&'static str>)> {
    THEME_DEFAULTS
        .iter()
        .map(|d| {
            let kind = DefaultsKind::from_name(d.kind).expect("kit kind is a defaults kind");
            (kind, d.style, d.text_style)
        })
        .collect()
}

fn theme_names() -> Vec<&'static str> {
    EMBEDDED_PACKS
        .iter()
        .filter_map(|(id, _)| id.strip_prefix("@zenith/theme."))
        .collect()
}

/// Assert `doc` carries exactly the theme style ids and `defaults` entries,
/// and that every token a style references is declared.
fn assert_theme_blocks(doc: &Document, label: &str) {
    let ids: Vec<&str> = doc.styles.styles.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, STYLE_IDS, "{label}: style ids");
    let entries: Vec<(DefaultsKind, &str, Option<&str>)> = doc
        .defaults
        .entries
        .iter()
        .map(|(k, e)| (*k, e.style.as_str(), e.text_style.as_deref()))
        .collect();
    assert_eq!(entries, expected_defaults(), "{label}: defaults entries");
    assert!(doc.defaults.rejected.is_empty(), "{label}: rejected rows");
    for style in &doc.styles.styles {
        assert!(
            style.unknown_props.is_empty(),
            "{label}: {} unknown keys",
            style.id
        );
        for value in style.properties.values() {
            let zenith_core::PropertyValue::TokenRef(id) = value else {
                panic!("{label}: style {} holds a non-token value", style.id);
            };
            assert!(
                doc.tokens.tokens.iter().any(|t| &t.id == id),
                "{label}: style {} references undeclared token {id}",
                style.id
            );
        }
    }
    assert!(
        doc.tokens
            .tokens
            .iter()
            .any(|t| t.id == HEADING_WEIGHT_TOKEN_ID),
        "{label}: {HEADING_WEIGHT_TOKEN_ID} token"
    );
    assert!(
        doc.tokens
            .tokens
            .iter()
            .any(|t| t.id == DISPLAY_SIZE_TOKEN_ID),
        "{label}: {DISPLAY_SIZE_TOKEN_ID} token"
    );
}

/// Validate `src` as the CLI does; returns (exit code, diagnostic codes).
fn validate_codes(src: &str, dir: Option<&Path>) -> (u8, Vec<String>) {
    let out = validate::run(src, dir, true, &PolicyFlags::default());
    let value: serde_json::Value =
        serde_json::from_str(&out.stdout).expect("validate --json output parses");
    let codes = value["diagnostics"]
        .as_array()
        .expect("diagnostics array")
        .iter()
        .map(|d| d["code"].as_str().unwrap_or_default().to_owned())
        .collect();
    (out.exit_code, codes)
}

fn themed_doc(tmp: &TempDir, theme: &str) -> (PathBuf, String) {
    let paths = StorePaths::new(tmp.path());
    let path = tmp.path().join("t.zen");
    new::run_in(&paths, &path, None, DEFAULT_PAGE, Some(theme)).expect("themed new");
    let src = std::fs::read_to_string(&path).expect("read scaffold");
    (path, src)
}

#[test]
fn every_pack_validates_clean_and_ships_styles_and_defaults() {
    let names = theme_names();
    assert_eq!(names.len(), 10);
    for name in names {
        let pack = resolve_theme_pack(None, name).expect("embedded pack resolves");
        assert_theme_blocks(&pack, name);
        let src = EMBEDDED_PACKS
            .iter()
            .find(|(id, _)| id.strip_prefix("@zenith/theme.") == Some(name))
            .map(|(_, src)| *src)
            .expect("pack source");
        let (code, codes) = validate_codes(src, None);
        assert_eq!(code, 0, "{name}: exit code; codes {codes:?}");
        assert!(
            codes.iter().all(|c| c == "token.set_partially_used"),
            "{name}: unexpected diagnostics {codes:?}"
        );
    }
}

#[test]
fn new_with_theme_copies_styles_and_defaults_in_canonical_order() {
    for name in theme_names() {
        let tmp = TempDir::new().unwrap();
        let (_, src) = themed_doc(&tmp, name);
        let doc = KdlAdapter.parse(src.as_bytes()).expect("scaffold parses");
        assert_theme_blocks(&doc, name);
        let tokens = src.find("  tokens ").expect("tokens block");
        let styles = src.find("  styles {").expect("styles block");
        let defaults = src.find("  defaults {").expect("defaults block");
        assert!(tokens < styles && styles < defaults, "{name}: block order");
    }
}

/// The brief's repro: bare nodes on a `cobalt` document validate with no
/// errors and no contrast diagnostics, and render on-theme.
#[test]
fn bare_nodes_on_cobalt_render_on_theme() {
    let tmp = TempDir::new().unwrap();
    let (path, src) = themed_doc(&tmp, "cobalt");
    let empty_page = "background=(token)\"color.base.100\" {\n    }";
    assert!(src.contains(empty_page), "scaffold page shape:\n{src}");
    let src = src.replace(
        empty_page,
        "background=(token)\"color.base.100\" {\n      \
         shape id=\"cta\" kind=\"process\" x=(px)120 y=(px)400 w=(px)240 h=(px)64 fill=(token)\"color.primary\" { span \"Buy now\" }\n      \
         text id=\"t\" x=(px)120 y=(px)120 w=(px)600 h=(px)80 { span \"Hello\" }\n    }",
    );

    let (code, codes) = validate_codes(&src, path.parent());
    assert_eq!(code, 0, "exit code; codes {codes:?}");
    assert!(
        !codes.iter().any(|c| c.contains("contrast")),
        "contrast diagnostics: {codes:?}"
    );

    let doc = KdlAdapter.parse(src.as_bytes()).expect("parse");
    let compiled = zenith_scene::compile(&doc, &default_provider());
    assert!(
        !compiled
            .diagnostics
            .iter()
            .any(|d| d.is_error() || d.code.contains("contrast")),
        "compile diagnostics: {:?}",
        compiled.diagnostics
    );
    let scene = serde_json::to_value(&compiled.scene).expect("scene serializes");
    let commands = scene["commands"].as_array().expect("commands");

    // cobalt: radius.field 4px, primary #605dff, primary.content #edf1fe,
    // base.content #0d1529, size.body 28px, font.body Noto Sans.
    let rect = commands
        .iter()
        .find(|c| c["op"] == "FillRoundedRect" && c["x"] == 120.0 && c["y"] == 400.0)
        .expect("shape body");
    assert_eq!(rect["radius"], 4.0, "shape radius = radius.field");
    let rgb = |c: &serde_json::Value| (c["r"].as_u64(), c["g"].as_u64(), c["b"].as_u64());
    assert_eq!(
        rgb(&rect["paint"]["color"]),
        (Some(96), Some(93), Some(255))
    );

    let runs: Vec<&serde_json::Value> = commands
        .iter()
        .filter(|c| c["op"] == "DrawGlyphRun")
        .collect();
    let label = runs
        .iter()
        .find(|r| r["y"].as_f64().is_some_and(|y| y > 400.0 && y < 464.0))
        .expect("label run");
    assert_eq!(
        rgb(&label["color"]),
        (Some(237), Some(241), Some(254)),
        "label fill = color.primary.content"
    );
    let text = runs
        .iter()
        .find(|r| r["y"].as_f64().is_some_and(|y| y > 120.0 && y < 200.0))
        .expect("text run");
    assert_eq!(
        rgb(&text["color"]),
        (Some(13), Some(21), Some(41)),
        "text fill = color.base.content"
    );
    for run in [label, text] {
        assert_eq!(run["font_size"], 28.0, "size.body");
        assert!(
            run["font_id"]
                .as_str()
                .is_some_and(|f| f.starts_with("noto-sans-")),
            "font.body = Noto Sans; got {}",
            run["font_id"]
        );
    }
}

const BARE_DOC: &str = r##"zenith version=1 {
  project id="proj.bare" name="Bare"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
  }
  styles {}
  document id="doc.bare" title="Bare" {
    page id="pg" w=(px)400 h=(px)300 background=(token)"color.bg" {}
  }
}
"##;

#[test]
fn apply_adds_styles_and_defaults_then_is_idempotent() {
    let tmp = TempDir::new().unwrap();
    let first = apply_run(Some(tmp.path()), "ember", BARE_DOC).expect("ember resolves");
    assert_eq!(first.exit_code, 0, "{}", first.human);
    assert_eq!(first.added_styles, STYLE_IDS);
    let kinds: Vec<&str> = THEME_DEFAULTS.iter().map(|d| d.kind).collect();
    assert_eq!(first.added_defaults, kinds);
    assert!(first.skipped_styles.is_empty());
    assert!(first.skipped_defaults.is_empty());
    let doc = KdlAdapter
        .parse(first.result.source_after.as_bytes())
        .expect("applied source parses");
    assert_theme_blocks(&doc, "ember applied");
    let (code, _) = validate_codes(&first.result.source_after, None);
    assert_eq!(code, 0);

    let json: serde_json::Value = serde_json::from_str(&first.json_str).expect("json");
    assert_eq!(
        json["added_styles"].as_array().map(Vec::len),
        Some(STYLE_IDS.len())
    );
    assert_eq!(
        json["added_defaults"].as_array().map(Vec::len),
        Some(THEME_DEFAULTS.len())
    );

    // Determinism: the same input yields the same bytes.
    let again = apply_run(Some(tmp.path()), "ember", BARE_DOC).expect("ember resolves");
    assert_eq!(again.result.source_after, first.result.source_after);

    let second =
        apply_run(Some(tmp.path()), "ember", &first.result.source_after).expect("re-apply");
    assert_eq!(
        second.result.source_before, second.result.source_after,
        "second apply changes nothing"
    );
    assert!(second.added_styles.is_empty());
    assert!(second.added_defaults.is_empty());
    assert_eq!(second.skipped_styles.len(), STYLE_IDS.len());
    assert!(
        second
            .skipped_defaults
            .iter()
            .all(|s| s.reason.label() == "exists")
    );
    assert!(
        second.human.contains("skipped defaults:"),
        "{}",
        second.human
    );
}

#[test]
fn apply_keeps_existing_style_and_kind() {
    let doc = BARE_DOC.replace(
        "  styles {}\n",
        "  styles {\n    style id=\"ui.body\" {\n      fill (token)\"color.bg\"\n    }\n  }\n  defaults {\n    text style=\"ui.body\"\n  }\n",
    );
    let tmp = TempDir::new().unwrap();
    let out = apply_run(Some(tmp.path()), "cobalt", &doc).expect("cobalt resolves");
    assert_eq!(out.exit_code, 0, "{}", out.human);
    assert!(!out.added_styles.contains(&"ui.body".to_owned()));
    assert_eq!(out.skipped_styles.len(), 1);
    assert_eq!(out.added_defaults, ["chart", "connector", "shape"]);
    assert_eq!(out.skipped_defaults.len(), 1);
    assert_eq!(out.skipped_defaults[0].id, "text");
    assert!(
        out.result
            .source_after
            .contains("style id=\"ui.body\" {\n      fill (token)\"color.bg\"\n    }"),
        "existing body style kept:\n{}",
        out.result.source_after
    );
}

#[test]
fn theme_new_emits_styles_and_defaults_deterministically() {
    for (scheme, depth) in [(Scheme::Light, false), (Scheme::Dark, true)] {
        let input = ThemeInput {
            name: "acme",
            scheme,
            primary: "#3b5bdb",
            secondary: None,
            accent: None,
            neutral: None,
            info: None,
            success: None,
            warning: None,
            error: None,
            shape: Shape {
                depth,
                ..Shape::default()
            },
        };
        let src = theme_new(&input).expect("theme new");
        assert_eq!(theme_new(&input).expect("theme new"), src, "deterministic");
        let doc = KdlAdapter.parse(src.as_bytes()).expect("theme parses");
        assert_theme_blocks(&doc, "theme new");
        let (code, _) = validate_codes(&src, None);
        assert_eq!(code, 0, "theme new validates:\n{src}");
    }
}

/// Theme style ids share the global id namespace with nodes, so they sit
/// under `ui.`: common node ids on a themed page stay unique.
#[test]
fn common_node_ids_do_not_collide_with_theme_styles() {
    let tmp = TempDir::new().unwrap();
    let (path, src) = themed_doc(&tmp, "cobalt");
    let empty_page = "background=(token)\"color.base.100\" {\n    }";
    let mut nodes = String::new();
    for (i, id) in [
        "body",
        "h1",
        "h2",
        "caption",
        "label",
        "control",
        "button",
        "card",
        "connector",
    ]
    .iter()
    .enumerate()
    {
        nodes.push_str(&format!(
            "      rect id=\"{id}\" x=(px)0 y=(px){} w=(px)10 h=(px)10 style=\"ui.card\"\n",
            i * 20
        ));
    }
    let src = src.replace(
        empty_page,
        &format!("background=(token)\"color.base.100\" {{\n{nodes}    }}"),
    );
    let (code, codes) = validate_codes(&src, path.parent());
    assert_eq!(code, 0, "codes {codes:?}");
    assert!(!codes.iter().any(|c| c == "id.duplicate"), "{codes:?}");
}

/// Comparable form of a pack's kit: styles, `defaults` rows, heading token.
type Kit = (
    Vec<(
        String,
        std::collections::BTreeMap<String, zenith_core::PropertyValue>,
    )>,
    Vec<(&'static str, String, Option<String>)>,
    Vec<(String, zenith_core::TokenType, zenith_core::TokenValue)>,
);

fn kit_of(doc: &Document) -> Kit {
    let styles = doc
        .styles
        .styles
        .iter()
        .map(|s| (s.id.clone(), s.properties.clone()))
        .collect();
    let defaults = doc
        .defaults
        .entries
        .iter()
        .map(|(k, e)| (k.name(), e.style.clone(), e.text_style.clone()))
        .collect();
    let tokens = doc
        .tokens
        .tokens
        .iter()
        .filter(|t| t.id == HEADING_WEIGHT_TOKEN_ID || t.id == DISPLAY_SIZE_TOKEN_ID)
        .map(|t| (t.id.clone(), t.token_type.clone(), t.value.clone()))
        .collect();
    (styles, defaults, tokens)
}

/// Name the first entry where `pack` differs from `kit`.
fn first_drift<T: std::fmt::Debug + PartialEq>(
    what: &str,
    pack: &[T],
    kit: &[T],
) -> Option<String> {
    for i in 0..pack.len().max(kit.len()) {
        if pack.get(i) != kit.get(i) {
            return Some(format!(
                "{what}[{i}]: pack {:?} vs kit {:?}",
                pack.get(i),
                kit.get(i)
            ));
        }
    }
    None
}

/// The hand-edited blocks in each embedded pack equal the kit `theme new`
/// emits, so the two cannot drift apart.
#[test]
fn every_pack_matches_the_kit() {
    let kit_doc = KdlAdapter
        .parse(kit_document_source().as_bytes())
        .expect("kit source parses");
    let kit = kit_of(&kit_doc);
    assert_eq!(
        kit.2.len(),
        2,
        "kit declares the display size and heading weight tokens"
    );
    assert_eq!(HEADING_WEIGHT_TOKEN_TYPE, "fontWeight");
    assert_eq!(HEADING_WEIGHT_TOKEN_VALUE, 700);
    assert_eq!(DISPLAY_SIZE_TOKEN_TYPE, "dimension");
    assert_eq!(DISPLAY_SIZE_TOKEN_PX, 112);
    for name in theme_names() {
        let pack = resolve_theme_pack(None, name).expect("embedded pack resolves");
        let got = kit_of(&pack);
        let drift = first_drift("styles", &got.0, &kit.0)
            .or_else(|| first_drift("defaults", &got.1, &kit.1))
            .or_else(|| first_drift("token", &got.2, &kit.2));
        assert!(drift.is_none(), "theme pack {name} drifted: {drift:?}");
    }
}

/// `theme new` output carries the same kit as the packs.
#[test]
fn theme_new_matches_the_kit() {
    let kit_doc = KdlAdapter
        .parse(kit_document_source().as_bytes())
        .expect("kit source parses");
    let input = ThemeInput {
        name: "acme",
        scheme: Scheme::Light,
        primary: "#3b5bdb",
        secondary: None,
        accent: None,
        neutral: None,
        info: None,
        success: None,
        warning: None,
        error: None,
        shape: Shape::default(),
    };
    let src = theme_new(&input).expect("theme new");
    let doc = KdlAdapter.parse(src.as_bytes()).expect("theme parses");
    assert_eq!(kit_of(&doc), kit_of(&kit_doc));
}
