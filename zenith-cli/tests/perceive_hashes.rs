//! Pinned perceive hashes: `zenith perceive page --all-pages --json` of every
//! `examples/*.zen` prints the SHA-256 in `tests/perceive_hashes.json` on
//! every target.
//!
//! Each example runs through the function the CLI calls, in process, and the
//! hash covers the exact stdout bytes: the JSON plus the trailing newline.
//! `cli_stdout_is_the_hashed_bytes` checks that the binary prints those
//! bytes.
//!
//! The host is hermetic, as in `render_hashes.rs`: project files from disk
//! through the CLI's `NativeFs`, pages on the CLI's thread pool, no config,
//! and no OS font directory. Only the bundled fonts render.
//!
//! The manifest holds one hash per example for all targets. A
//! target-dependent byte is a determinism bug: fix it at the source, never
//! add a per-target entry.
//!
//! Regenerate the manifest after an intended output change:
//!
//! ```text
//! ZENITH_UPDATE_HASHES=1 cargo test -p zenith-tool --test perceive_hashes
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};
use zenith_cli::commands::perceive::page;
use zenith_cli::native::{NativeFs, ThreadPoolRunner};
use zenith_pipeline::render::PageSelection;
use zenith_pipeline::{Host, NoConfig, NoLocalFonts};

/// The examples directory, relative to the crate directory (the test cwd).
const EXAMPLES: &str = "../examples";

/// The committed manifest, relative to the crate directory.
const MANIFEST: &str = "tests/perceive_hashes.json";

/// The env var that rewrites the manifest from the current output.
const UPDATE_VAR: &str = "ZENITH_UPDATE_HASHES";

/// The regenerate command, quoted in every error message.
const UPDATE_CMD: &str = "ZENITH_UPDATE_HASHES=1 cargo test -p zenith-tool --test perceive_hashes";

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

/// The stdout bytes of `zenith perceive page <doc> --all-pages --json` on the
/// hermetic host.
fn stdout_of(doc: &Path) -> String {
    let config = NoConfig;
    let host = Host::new(&NativeFs, &config)
        .with_local_fonts(&NoLocalFonts)
        .with_runner(&ThreadPoolRunner);
    let src = std::fs::read_to_string(doc).expect("read example");
    let outcome = page(
        host,
        &src,
        Some(Path::new(EXAMPLES)),
        PageSelection::All,
        true,
    )
    .unwrap_or_else(|e| panic!("{}: perceive page failed: {}", doc.display(), e.message));
    assert_eq!(outcome.exit_code, 0, "{}", doc.display());
    format!("{}\n", outcome.stdout)
}

/// The hash of every example's stdout, keyed by `<stem>.zen`.
fn perceive_all() -> BTreeMap<String, String> {
    let docs = examples();
    assert!(docs.len() >= 30, "found {} examples", docs.len());
    let mut hashes = BTreeMap::new();
    for doc in &docs {
        let name = doc
            .file_name()
            .and_then(|s| s.to_str())
            .expect("utf-8 example name")
            .to_owned();
        hashes.insert(name, sha256(stdout_of(doc).as_bytes()));
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
    let target = target();
    let mut out = Vec::new();
    for (key, hash) in actual {
        match expected.get(key) {
            None => out.push(format!(
                "{key}: no manifest entry (actual {hash}). Run `{UPDATE_CMD}` and commit {MANIFEST}"
            )),
            Some(want) if want != hash => {
                out.push(format!("{key}: expected {want}, actual {hash} on {target}"));
            }
            Some(_) => {}
        }
    }
    for key in expected.keys().filter(|k| !actual.contains_key(*k)) {
        out.push(format!(
            "{key}: manifest entry has no example. Run `{UPDATE_CMD}` and commit {MANIFEST}"
        ));
    }
    out
}

#[test]
fn every_example_perceives_to_the_pinned_hashes() {
    let actual = perceive_all();
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
        "{} of {} examples differ from {MANIFEST} on {}:\n{}",
        diffs.len(),
        actual.len(),
        target(),
        diffs.join("\n")
    );
}

/// Two runs in one process give the same hashes: no time, randomness, or
/// thread-order dependence.
#[test]
fn perceiving_twice_gives_the_same_hashes() {
    assert_eq!(perceive_all(), perceive_all());
}

/// The binary prints the bytes the manifest hashes. `rect.zen` has no text,
/// so the CLI's local fonts and config cannot change it.
#[test]
fn cli_stdout_is_the_hashed_bytes() {
    let doc = Path::new(EXAMPLES).join("rect.zen");
    let output = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .args(["perceive", "page"])
        .arg(&doc)
        .args(["--all-pages", "--json"])
        .output()
        .expect("run zenith perceive page");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    assert_eq!(stdout, stdout_of(&doc));
}
