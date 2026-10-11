//! Pinned render hashes for the `zenith new --starter` layouts: every
//! [`Starter`] renders to the SHA-256 in `tests/starter_hashes.json` on every
//! target, at its natural size with its default theme and with the `sunset`
//! theme.
//!
//! Each starter document (before a `doc-id` is stamped) renders through the
//! pipeline `zenith render` uses, in process, as `tests/render_hashes.rs`
//! renders the examples:
//! - PNG of every page (`page-N.png`).
//! - SVG of every page (`page-N.svg`).
//! - One PDF of every page (`<kind>.pdf`).
//!
//! The host is hermetic. It reads no global or local config and no OS font
//! directory, so only the bundled fonts render. A target-dependent byte is a
//! determinism bug: fix it at the source, never add a per-target entry.
//!
//! Regenerate the manifest after an intended starter change:
//!
//! ```text
//! ZENITH_UPDATE_HASHES=1 cargo test -p zenith-tool --test starter_hashes
//! ```

use std::collections::BTreeMap;
use std::path::Path;

use clap::ValueEnum as _;
use sha2::{Digest, Sha256};
use zenith_cli::commands::new::{Starter, StarterSize, starter_document};
use zenith_cli::native::{NativeFs, ThreadPoolRunner};
use zenith_core::Diagnostic;
use zenith_pipeline::render::{render_pdf_pages, render_png_pages, render_svg_pages};
use zenith_pipeline::{Host, NoConfig, NoLocalFonts, PolicyFlags, RenderOptions};

/// The committed manifest, relative to the crate directory.
const MANIFEST: &str = "tests/starter_hashes.json";

/// The env var that rewrites the manifest from the current renders.
const UPDATE_VAR: &str = "ZENITH_UPDATE_HASHES";

/// The regenerate command, quoted in every error message.
const UPDATE_CMD: &str = "ZENITH_UPDATE_HASHES=1 cargo test -p zenith-tool --test starter_hashes";

/// Lowercase hex SHA-256 of `bytes`.
fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Panic when `diagnostics` holds an error: `zenith render` writes no file then.
fn assert_renders(kind: &str, format: &str, diagnostics: &[Diagnostic]) {
    assert!(
        !Diagnostic::has_errors(diagnostics),
        "{kind}: {format} render has error diagnostics: {diagnostics:?}"
    );
}

/// The non-default theme every starter is also pinned with.
const OTHER_THEME: &str = "sunset";

/// The hash of every output file of every starter, keyed
/// `<kind>.<theme>/page-N.png`, `<kind>.<theme>/page-N.svg`, and
/// `<kind>.<theme>/<kind>.pdf`. `<theme>` is `default` or [`OTHER_THEME`].
fn render_all() -> BTreeMap<String, String> {
    let config = NoConfig;
    let host = Host::new(&NativeFs, &config)
        .with_local_fonts(&NoLocalFonts)
        .with_runner(&ThreadPoolRunner);
    let dir = Some(Path::new("."));
    let flags = PolicyFlags::default();
    let opts = RenderOptions::new(&flags);

    let mut hashes = BTreeMap::new();
    for &starter in Starter::value_variants() {
        for theme in [None, Some(OTHER_THEME)] {
            let kind = starter.kind();
            let key = format!("{kind}.{}", theme.unwrap_or("default"));
            let bytes = starter_document(starter, theme, StarterSize::default())
                .unwrap_or_else(|e| panic!("{key}: build failed: {}", e.message));
            let src = String::from_utf8(bytes).expect("utf-8 starter");

            let png = render_png_pages(host, &src, dir, opts)
                .unwrap_or_else(|e| panic!("{key}: PNG render failed: {}", e.message));
            assert_renders(&key, "PNG", &png.diagnostics);
            for (i, bytes) in png.pages.iter().enumerate() {
                hashes.insert(format!("{key}/page-{}.png", i + 1), sha256(bytes));
            }

            let svg = render_svg_pages(host, &src, dir, opts)
                .unwrap_or_else(|e| panic!("{key}: SVG render failed: {}", e.message));
            assert_renders(&key, "SVG", &svg.diagnostics);
            for page in &svg.pages {
                hashes.insert(format!("{key}/page-{}.svg", page.page), sha256(&page.svg));
            }

            let pdf = render_pdf_pages(host, &src, dir, opts)
                .unwrap_or_else(|e| panic!("{key}: PDF render failed: {}", e.message));
            assert_renders(&key, "PDF", &pdf.diagnostics);
            hashes.insert(format!("{key}/{kind}.pdf"), sha256(&pdf.pdf));
        }
    }
    hashes
}

/// The manifest JSON: sorted keys, two-space indent, trailing newline.
fn encode(hashes: &BTreeMap<String, String>) -> String {
    let mut json = serde_json::to_string_pretty(hashes).expect("encode manifest");
    json.push('\n');
    json
}

/// One line per difference between `expected` and `actual`.
fn differences(
    expected: &BTreeMap<String, String>,
    actual: &BTreeMap<String, String>,
) -> Vec<String> {
    let target = format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS);
    let mut out = Vec::new();
    for (key, hash) in actual {
        match expected.get(key) {
            None => out.push(format!(
                "{key}: no manifest entry (actual {hash}). Run `{UPDATE_CMD}` and commit {MANIFEST}"
            )),
            Some(want) if want != hash => {
                out.push(format!("{key}: expected {want}, actual {hash} on {target}"))
            }
            Some(_) => {}
        }
    }
    for key in expected.keys().filter(|k| !actual.contains_key(*k)) {
        out.push(format!(
            "{key}: manifest entry has no rendered file. Run `{UPDATE_CMD}` and commit {MANIFEST}"
        ));
    }
    out
}

#[test]
fn every_starter_renders_to_the_pinned_hashes() {
    let actual = render_all();
    if std::env::var_os(UPDATE_VAR).is_some_and(|v| v == "1") {
        std::fs::write(MANIFEST, encode(&actual)).expect("write manifest");
        eprintln!("{MANIFEST}: wrote {} hashes", actual.len());
        return;
    }
    let text = std::fs::read_to_string(MANIFEST).unwrap_or_else(|e| {
        panic!("cannot read {MANIFEST}: {e}. Run `{UPDATE_CMD}` and commit the file")
    });
    let expected: BTreeMap<String, String> = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{MANIFEST} is not a JSON object of hashes: {e}"));
    assert_eq!(
        text,
        encode(&expected),
        "{MANIFEST} is not in canonical form. Run `{UPDATE_CMD}`"
    );
    let diffs = differences(&expected, &actual);
    assert!(
        diffs.is_empty(),
        "{} starter output(s) differ from {MANIFEST}:\n{}",
        diffs.len(),
        diffs.join("\n")
    );
}

/// Two builds and renders in one process give the same hashes.
#[test]
fn rendering_twice_gives_the_same_hashes() {
    assert_eq!(render_all(), render_all());
}
