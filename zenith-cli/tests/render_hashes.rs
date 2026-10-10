//! Pinned render hashes: every `examples/*.zen` renders to the SHA-256 in
//! `tests/render_hashes.json` on every target.
//!
//! Each example renders through the pipeline `zenith render` uses, in process:
//! - PNG of every page, as `--all-pages <DIR>` writes them (`page-N.png`).
//! - SVG of every page, as `--all-pages-svg <DIR>` writes them (`page-N.svg`).
//! - One PDF of every page, as `--pdf <OUT>` writes it.
//!
//! Options are the CLI defaults: scale 1, raster scale 1, subset PDF fonts
//! (no `--embed-full-fonts`), unlocked, no data, no policy flags.
//!
//! The host is hermetic. It reads project files from disk with the CLI's
//! `NativeFs` and runs pages on the CLI's thread pool. It reads no global or
//! local config and no OS font directory, so only the bundled fonts render.
//! A developer's installed fonts or `~/.config/zenith/config.kdl` never
//! change a hash.
//!
//! The manifest holds one hash per file for all targets. A target-dependent
//! byte is a determinism bug: fix it at the source, never add a per-target
//! entry.
//!
//! Regenerate the manifest after an intended output change:
//!
//! ```text
//! ZENITH_UPDATE_HASHES=1 cargo test -p zenith-tool --test render_hashes
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use zenith_cli::native::{NativeFs, ThreadPoolRunner};
use zenith_core::Diagnostic;
use zenith_pipeline::render::{render_pdf_pages, render_png_pages, render_svg_pages};
use zenith_pipeline::{Host, NoConfig, NoLocalFonts, PolicyFlags, RenderOptions};

/// The examples directory, relative to the crate directory (the test cwd).
const EXAMPLES: &str = "../examples";

/// The committed manifest, relative to the crate directory.
const MANIFEST: &str = "tests/render_hashes.json";

/// The env var that rewrites the manifest from the current renders.
const UPDATE_VAR: &str = "ZENITH_UPDATE_HASHES";

/// The regenerate command, quoted in every error message.
const UPDATE_CMD: &str = "ZENITH_UPDATE_HASHES=1 cargo test -p zenith-tool --test render_hashes";

/// The target this test binary runs on, for error messages.
fn target() -> String {
    format!(
        "{}-{}-{}",
        std::env::consts::ARCH,
        std::env::consts::OS,
        if cfg!(target_env = "msvc") {
            "msvc"
        } else if cfg!(target_env = "gnu") {
            "gnu"
        } else if cfg!(target_env = "musl") {
            "musl"
        } else {
            "none"
        }
    )
}

/// Every top-level `examples/*.zen`, sorted by path.
fn examples() -> Vec<PathBuf> {
    let mut docs: Vec<PathBuf> = std::fs::read_dir(EXAMPLES)
        .expect("read examples dir")
        .map(|entry| entry.expect("examples entry").path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "zen"))
        .collect();
    docs.sort();
    docs
}

/// Lowercase hex SHA-256 of `bytes`.
fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Panic when `diagnostics` holds an error: `zenith render` writes no file
/// then, so there is no output to pin.
fn assert_renders(name: &str, format: &str, diagnostics: &[Diagnostic]) {
    assert!(
        !Diagnostic::has_errors(diagnostics),
        "{name}: {format} render has error diagnostics, so `zenith render` writes no file: \
         {diagnostics:?}"
    );
}

/// The hash of every output file of every example, keyed
/// `<stem>/page-N.png`, `<stem>/page-N.svg`, and `<stem>/<stem>.pdf`.
fn render_all() -> BTreeMap<String, String> {
    let config = NoConfig;
    let host = Host::new(&NativeFs, &config)
        .with_local_fonts(&NoLocalFonts)
        .with_runner(&ThreadPoolRunner);
    let dir = Some(Path::new(EXAMPLES));
    let flags = PolicyFlags::default();
    let opts = RenderOptions::new(&flags);

    let docs = examples();
    assert!(docs.len() >= 30, "found {} examples", docs.len());
    let mut hashes = BTreeMap::new();
    for doc in &docs {
        let src = std::fs::read_to_string(doc).expect("read example");
        let stem = doc
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("utf-8 example name");
        let name = format!("{stem}.zen");

        let png = render_png_pages(host, &src, dir, opts)
            .unwrap_or_else(|e| panic!("{name}: PNG render failed: {}", e.message));
        assert_renders(&name, "PNG", &png.diagnostics);
        for (i, bytes) in png.pages.iter().enumerate() {
            hashes.insert(format!("{stem}/page-{}.png", i + 1), sha256(bytes));
        }

        let svg = render_svg_pages(host, &src, dir, opts)
            .unwrap_or_else(|e| panic!("{name}: SVG render failed: {}", e.message));
        assert_renders(&name, "SVG", &svg.diagnostics);
        for page in &svg.pages {
            hashes.insert(format!("{stem}/page-{}.svg", page.page), sha256(&page.svg));
        }

        let pdf = render_pdf_pages(host, &src, dir, opts)
            .unwrap_or_else(|e| panic!("{name}: PDF render failed: {}", e.message));
        assert_renders(&name, "PDF", &pdf.diagnostics);
        hashes.insert(format!("{stem}/{stem}.pdf"), sha256(&pdf.pdf));
    }
    hashes
}

/// The manifest JSON: sorted keys, two-space indent, trailing newline.
fn encode(hashes: &BTreeMap<String, String>) -> String {
    let mut json = serde_json::to_string_pretty(hashes).expect("encode manifest");
    json.push('\n');
    json
}

/// `<stem>.zen <file>` for the output keyed `<stem>/<file>`.
fn describe(key: &str) -> String {
    match key.split_once('/') {
        Some((stem, file)) => format!("{stem}.zen {file}"),
        None => key.to_owned(),
    }
}

/// One line per difference between `expected` and `actual`.
fn differences(
    expected: &BTreeMap<String, String>,
    actual: &BTreeMap<String, String>,
) -> Vec<String> {
    let target = target();
    let mut out = Vec::new();
    for (key, hash) in actual {
        match expected.get(key) {
            None => out.push(format!(
                "{}: no manifest entry (actual {hash}). Run `{UPDATE_CMD}` and commit {MANIFEST}",
                describe(key)
            )),
            Some(want) if want != hash => out.push(format!(
                "{}: expected {want}, actual {hash} on {target}",
                describe(key)
            )),
            Some(_) => {}
        }
    }
    for key in expected.keys().filter(|k| !actual.contains_key(*k)) {
        out.push(format!(
            "{}: manifest entry has no rendered file. Run `{UPDATE_CMD}` and commit {MANIFEST}",
            describe(key)
        ));
    }
    out
}

#[test]
fn every_example_renders_to_the_pinned_hashes() {
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
        "{} of {} rendered files differ from {MANIFEST} on {}:\n{}",
        diffs.len(),
        actual.len(),
        target(),
        diffs.join("\n")
    );
}

/// Two renders in one process give the same hashes: no time, randomness, or
/// thread-order dependence.
#[test]
fn rendering_twice_gives_the_same_hashes() {
    assert_eq!(render_all(), render_all());
}
