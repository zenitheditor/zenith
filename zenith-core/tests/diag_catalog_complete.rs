//! Guards that every diagnostic code emitted anywhere in the workspace is in
//! the diagnostic catalog.
//!
//! The scan is deterministic and regex-free. It reads each crate's non-test
//! source, finds calls to the diagnostic constructors, and takes the first
//! argument: a string literal, or a `const NAME: &str = "…"` constant declared
//! anywhere in the scanned sources. A first argument that is a plain variable
//! (a code forwarded from a caller) is skipped.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use zenith_core::diag_catalog::{DIAGNOSTIC_CODES, lookup};

/// Crates whose `src/` is scanned. Everything except `zenith-core` is read-only.
const SCANNED_CRATES: &[&str] = &[
    "zenith-core",
    "zenith-scene",
    "zenith-tx",
    "zenith-cli",
    "zenith-layout",
    "zenith-render",
];

/// Constructors whose first argument is a diagnostic code. `RenderCmdErr::new`
/// wraps `Diagnostic::error` in the CLI render path.
const CONSTRUCTORS: &[&str] = &[
    "Diagnostic::error(",
    "Diagnostic::warning(",
    "Diagnostic::advisory(",
    "Diagnostic::new(",
    "RenderCmdErr::new(",
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Collect every `.rs` file under `dir` into `out`, skipping test directories
/// and files named `tests.rs` / `*_tests.rs`.
fn collect_sources(dir: &Path, out: &mut BTreeSet<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if name != "tests" {
                collect_sources(&path, out);
            }
        } else if name.ends_with(".rs") && name != "tests.rs" && !name.ends_with("_tests.rs") {
            out.insert(path);
        }
    }
}

/// Return the source with `//` comment lines and the trailing
/// `#[cfg(test)] mod …` section removed.
fn production_source(text: &str) -> String {
    let mut cut = text.len();
    let mut from = 0;
    while let Some(rel) = text[from..].find("#[cfg(test)]") {
        let at = from + rel;
        let after = text[at + "#[cfg(test)]".len()..].trim_start();
        if after.starts_with("mod ") {
            cut = at;
            break;
        }
        from = at + 1;
    }
    text[..cut]
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Read a `"…"` literal that starts at the beginning of `s`.
fn read_literal(s: &str) -> Option<&str> {
    let body = s.strip_prefix('"')?;
    let end = body.find('"')?;
    Some(&body[..end])
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == ':'
}

/// Find every `const NAME: &str = "literal"` in `src`.
fn collect_str_consts(src: &str, consts: &mut BTreeMap<String, BTreeSet<String>>) {
    let mut from = 0;
    while let Some(rel) = src[from..].find("const ") {
        let start = from + rel + "const ".len();
        from = start;
        let rest = &src[start..];
        let name_len = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        if name_len == 0 {
            continue;
        }
        let name = &rest[..name_len];
        let Some(after_colon) = rest[name_len..].strip_prefix(':') else {
            continue;
        };
        let ty = after_colon.trim_start();
        let Some(ty) = ty.strip_prefix('&') else {
            continue;
        };
        let ty = ty.trim_start();
        let ty = ty.strip_prefix("'static").map_or(ty, str::trim_start);
        let Some(after_str) = ty.strip_prefix("str") else {
            continue;
        };
        let Some(value) = after_str.trim_start().strip_prefix('=') else {
            continue;
        };
        if let Some(lit) = read_literal(value.trim_start()) {
            consts
                .entry(name.to_owned())
                .or_default()
                .insert(lit.to_owned());
        }
    }
}

/// Every code string (and the file it was found in) passed to a constructor.
fn emitted_codes() -> BTreeMap<String, BTreeSet<String>> {
    let root = workspace_root();
    let mut files = BTreeSet::new();
    for krate in SCANNED_CRATES {
        collect_sources(&root.join(krate).join("src"), &mut files);
    }
    let sources: Vec<(String, String)> = files
        .iter()
        .filter_map(|path| {
            let text = fs::read_to_string(path).ok()?;
            let shown = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned();
            Some((shown, production_source(&text)))
        })
        .collect();

    let mut consts: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (_, src) in &sources {
        collect_str_consts(src, &mut consts);
    }

    let mut found: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (file, src) in &sources {
        for ctor in CONSTRUCTORS {
            let mut from = 0;
            while let Some(rel) = src[from..].find(ctor) {
                let start = from + rel + ctor.len();
                from = start;
                let arg = src[start..].trim_start();
                let codes: Vec<String> = if let Some(lit) = read_literal(arg) {
                    vec![lit.to_owned()]
                } else {
                    let len = arg.find(|c: char| !is_ident_char(c)).unwrap_or(arg.len());
                    let ident = arg[..len].rsplit("::").next().unwrap_or("");
                    consts
                        .get(ident)
                        .map(|set| set.iter().cloned().collect())
                        .unwrap_or_default()
                };
                for code in codes {
                    found.entry(code).or_default().insert(file.clone());
                }
            }
        }
    }
    found
}

/// True for `<namespace>.<snake_event>`: a lowercase namespace (hyphens
/// allowed) and a lowercase snake_case event.
fn has_code_shape(code: &str) -> bool {
    let Some((ns, event)) = code.split_once('.') else {
        return false;
    };
    let ns_ok = !ns.is_empty()
        && ns
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
    let event_ok = !event.is_empty()
        && event
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    ns_ok && event_ok
}

#[test]
fn scanner_finds_known_codes() {
    let found = emitted_codes();
    assert!(
        found.len() > 150,
        "scanner found only {} codes; the source layout changed",
        found.len()
    );
    assert!(found.contains_key("layout.off_canvas"));
    assert!(
        found.contains_key("scene.text_outline_failed"),
        "constant lookup"
    );
    assert!(found.contains_key("light.unknown_kind"));
}

#[test]
fn every_emitted_code_is_catalogued() {
    let missing: Vec<String> = emitted_codes()
        .into_iter()
        .filter(|(code, _)| lookup(code).is_none())
        .map(|(code, files)| {
            format!(
                "{code} ({})",
                files.into_iter().collect::<Vec<_>>().join(", ")
            )
        })
        .collect();
    assert!(
        missing.is_empty(),
        "diagnostic codes emitted but missing from zenith-core/src/diag_catalog: {missing:#?}"
    );
}

#[test]
fn every_emitted_code_has_namespace_event_shape() {
    let bad: Vec<String> = emitted_codes()
        .into_keys()
        .filter(|code| !has_code_shape(code))
        .collect();
    assert!(
        bad.is_empty(),
        "codes not shaped <namespace>.<snake_event>: {bad:?}"
    );
}

#[test]
fn every_catalogued_code_has_namespace_event_shape() {
    let bad: Vec<&str> = DIAGNOSTIC_CODES
        .iter()
        .map(|info| info.code)
        .filter(|code| !has_code_shape(code))
        .collect();
    assert!(
        bad.is_empty(),
        "catalog codes not shaped <namespace>.<snake_event>: {bad:?}"
    );
}
