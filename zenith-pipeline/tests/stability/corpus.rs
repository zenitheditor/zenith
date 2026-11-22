//! Every frozen corpus document still parses, validates, formats, and renders
//! as pinned in `<corpus>/pins.json`.
//!
//! Per document:
//! - The parse succeeds.
//! - `validate_source`, the stage list `zenith validate` runs, reports no
//!   Error.
//! - The canonical format is idempotent: formatting the formatted text
//!   reproduces it.
//! - The file is canonical (its bytes equal its format) exactly when
//!   `pins.json` lists it under `canonical`.
//! - Each page renders to the PNG SHA-256 pinned under `png`, keyed
//!   `<document path without .zen>/page-N.png`.
//!
//! The host is hermetic: corpus files from disk, no config, no OS fonts.
//!
//! Re-pin `pins.json` after a declared change, then commit it:
//!
//! ```text
//! ZENITH_UPDATE_HASHES=1 cargo test -p zenith-pipeline --test stability
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zenith_core::{Diagnostic, KdlAdapter, KdlSource, Severity};
use zenith_pipeline::render::render_png_pages;
use zenith_pipeline::{Host, NoConfig, NoLocalFonts, PolicyFlags, RenderOptions, validate_source};

use super::disk_fs::DiskFs;
use super::frozen::{corpus_dir, documents, relative_name};

/// The env var that rewrites `pins.json` from the current corpus.
const UPDATE_VAR: &str = "ZENITH_UPDATE_HASHES";

/// The re-pin command, quoted in every error message.
const UPDATE_CMD: &str = "ZENITH_UPDATE_HASHES=1 cargo test -p zenith-pipeline --test stability";

/// The fewest documents a corpus holds. A lower count means files went missing.
const MIN_DOCUMENTS: usize = 30;

/// The pinned state of the corpus, as `pins.json` stores it.
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
struct Pins {
    /// The documents whose bytes are already canonical.
    canonical: BTreeSet<String>,
    /// The SHA-256 of every rendered page, keyed `<stem path>/page-N.png`.
    png: BTreeMap<String, String>,
}

/// Lowercase hex SHA-256 of `bytes`.
fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Parse `bytes` and format the document canonically.
fn format(name: &str, stage: &str, bytes: &[u8]) -> Vec<u8> {
    let doc = KdlAdapter
        .parse(bytes)
        .unwrap_or_else(|e| panic!("{name}: {stage} parse failed: {}", e.message));
    KdlAdapter
        .format(&doc)
        .unwrap_or_else(|e| panic!("{name}: {stage} format failed: {}", e.message))
}

/// The Error diagnostics in `diagnostics`.
fn errors(diagnostics: &[Diagnostic]) -> Vec<&Diagnostic> {
    diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect()
}

/// Check every document and collect its pins. Panics on a parse, validate,
/// idempotence, or render error.
fn check_all(dir: &Path) -> Pins {
    let config = NoConfig;
    let host = Host::new(&DiskFs, &config).with_local_fonts(&NoLocalFonts);
    let flags = PolicyFlags::default();
    let opts = RenderOptions::new(&flags);

    let docs = documents(dir);
    assert!(
        docs.len() >= MIN_DOCUMENTS,
        "{}: found {} documents, expected at least {MIN_DOCUMENTS}",
        dir.display(),
        docs.len()
    );
    let mut pins = Pins::default();
    for doc in &docs {
        let name = relative_name(dir, doc);
        let bytes = std::fs::read(doc).unwrap_or_else(|e| panic!("{name}: cannot read: {e}"));
        let src = String::from_utf8(bytes).unwrap_or_else(|e| panic!("{name}: not UTF-8: {e}"));
        let project_dir = doc.parent();

        let validation = validate_source(host, &src, project_dir, &flags);
        let found = errors(&validation.diagnostics);
        assert!(
            found.is_empty(),
            "{name}: validate reports {} Error diagnostic(s): {found:?}",
            found.len()
        );

        let once = format(&name, "source", src.as_bytes());
        let twice = format(&name, "formatted", &once);
        assert!(
            once == twice,
            "{name}: canonical format is not idempotent. Formatting the formatted text \
             changes it"
        );
        let normalized_src = src.replace("\r\n", "\n");
        if once == normalized_src.as_bytes() {
            pins.canonical.insert(name.clone());
        }

        let stem = name.strip_suffix(".zen").unwrap_or(&name);
        let png = render_png_pages(host, &src, project_dir, opts)
            .unwrap_or_else(|e| panic!("{name}: PNG render failed: {}", e.message));
        let found = errors(&png.diagnostics);
        assert!(
            found.is_empty(),
            "{name}: PNG render reports {} Error diagnostic(s): {found:?}",
            found.len()
        );
        for (i, page) in png.pages.iter().enumerate() {
            pins.png
                .insert(format!("{stem}/page-{}.png", i + 1), sha256(page));
        }
    }
    pins
}

/// The pins JSON: sorted keys, two-space indent, trailing newline.
fn encode(pins: &Pins) -> String {
    let mut json = serde_json::to_string_pretty(pins).expect("encode pins");
    json.push('\n');
    json
}

/// One line per difference between `expected` and `actual`.
fn differences(expected: &Pins, actual: &Pins) -> Vec<String> {
    let mut out = Vec::new();
    for name in actual.canonical.difference(&expected.canonical) {
        out.push(format!(
            "{name}: is canonical now, but pins.json does not list it"
        ));
    }
    for name in expected.canonical.difference(&actual.canonical) {
        out.push(format!(
            "{name}: pins.json lists it as canonical, but formatting changes its bytes"
        ));
    }
    for (key, hash) in &actual.png {
        match expected.png.get(key) {
            None => out.push(format!("{key}: no pinned hash (actual {hash})")),
            Some(want) if want != hash => {
                out.push(format!("{key}: pinned {want}, actual {hash}"));
            }
            Some(_) => {}
        }
    }
    for key in expected.png.keys().filter(|k| !actual.png.contains_key(*k)) {
        out.push(format!("{key}: pinned, but no page renders it"));
    }
    out
}

#[test]
fn frozen_corpus_parses_validates_formats_and_renders_as_pinned() {
    let dir = corpus_dir();
    let manifest = dir.join("pins.json");
    let actual = check_all(&dir);
    if std::env::var_os(UPDATE_VAR).is_some_and(|v| v == "1") {
        std::fs::write(&manifest, encode(&actual)).expect("write pins");
        eprintln!(
            "{}: wrote {} canonical names and {} PNG hashes",
            manifest.display(),
            actual.canonical.len(),
            actual.png.len()
        );
        return;
    }
    let text = std::fs::read_to_string(&manifest).unwrap_or_else(|e| {
        panic!(
            "cannot read {}: {e}. Run `{UPDATE_CMD}`",
            manifest.display()
        )
    });
    let expected: Pins = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{} is not a pins object: {e}", manifest.display()));
    assert_eq!(
        text.replace("\r\n", "\n"),
        encode(&expected),
        "{} is not in canonical form. Run `{UPDATE_CMD}`",
        manifest.display()
    );
    let diffs = differences(&expected, &actual);
    assert!(
        diffs.is_empty(),
        "{} differences from {}. A change to a frozen document is a declared change: \
         see README.md `## Stability`, then run `{UPDATE_CMD}`:\n{}",
        diffs.len(),
        manifest.display(),
        diffs.join("\n")
    );
}
