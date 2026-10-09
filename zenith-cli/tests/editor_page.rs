//! The browser editor page served by `zenith edit`: the vendored
//! CodeMirror bundle against its manifest, the Node unit tests of the page
//! modules, and the end-to-end runs in headless Chromium (the page suite on
//! `examples/stack.zen`, the canvas gesture suite on
//! `tests/editor_e2e/gestures.zen`, the selection, marquee, snapping, and
//! inspector suite on `tests/editor_e2e/canvas.zen`). The static host (the
//! same page over the wasm engine, no server) runs the three suites again, plus a cross-target check
//! of native against wasm on every example and the Node tests of
//! `WasmEngine`.
//!
//! The Node and Chromium checks skip with a message when `node` or a
//! Chromium binary is not on `PATH`. Set `ZENITH_E2E_CHROMIUM` to pick the
//! browser binary. The static-host checks also need the wasm module
//! (`cargo build --target wasm32-wasip1 -p zenith-editor-wasm --profile
//! release-wasm`): they read it from `ZENITH_E2E_WASM`, else from
//! `$CARGO_TARGET_DIR` (default `target/`) `/wasm32-wasip1/release-wasm/`, and
//! skip with a message when it is missing. Set `ZENITH_E2E_REQUIRED` to turn
//! every skip into an error (CI job `editor-wasm`). Screenshots land in
//! `conformance/editor/` (static host: `conformance/editor/static/`).

use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Report a missing prerequisite. Prints `SKIP <msg>` and returns, unless the
/// environment variable `ZENITH_E2E_REQUIRED` is set (the CI job `editor-wasm`
/// sets it). Then a missing prerequisite is an error, never a silent skip.
fn skip(msg: &str) {
    assert!(
        std::env::var_os("ZENITH_E2E_REQUIRED").is_none(),
        "ZENITH_E2E_REQUIRED is set but a prerequisite is missing: {msg}"
    );
    eprintln!("SKIP {msg}");
}

/// The first executable named one of `names` on `PATH`.
fn on_path(names: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .flat_map(|dir| names.iter().map(move |n| dir.join(n)))
        .find(|p| p.is_file())
}

fn node() -> Option<PathBuf> {
    on_path(&["node"])
}

fn chromium() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("ZENITH_E2E_CHROMIUM") {
        return Some(PathBuf::from(p));
    }
    on_path(&[
        "chromium",
        "chromium-browser",
        "google-chrome",
        "google-chrome-stable",
    ])
}

/// Run `node <script> <args>` and require exit code 0; print its output.
fn run_node(node: &Path, script: &Path, args: &[&std::ffi::OsStr]) {
    let out = Command::new(node)
        .arg(script)
        .args(args)
        .output()
        .expect("spawn node");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    println!("{stdout}");
    eprintln!("{stderr}");
    assert!(
        out.status.success(),
        "{} failed ({}):\n{stdout}\n{stderr}",
        script.display(),
        out.status
    );
}

#[test]
fn codemirror_bundle_matches_its_manifest() {
    let vendor = crate_dir().join("assets/editor/vendor");
    let manifest = std::fs::read_to_string(vendor.join("MANIFEST")).expect("read MANIFEST");
    let field = |key: &str| {
        manifest
            .lines()
            .filter(|l| !l.starts_with('#'))
            .find_map(|l| l.strip_prefix(key)?.strip_prefix(' '))
            .map(str::trim)
            .unwrap_or_else(|| panic!("MANIFEST has no '{key}' line"))
    };
    let file = field("file");
    let bundle = std::fs::read(vendor.join(file)).expect("read bundle");
    let bytes: usize = field("bytes").parse().expect("bytes is a number");
    assert_eq!(bundle.len(), bytes, "{file} size differs from MANIFEST");
    let sha = format!("{:x}", Sha256::digest(&bundle));
    assert_eq!(
        sha,
        field("sha256"),
        "{file} differs from the bundle MANIFEST names; rebuild it with the MANIFEST steps \
         or update bytes and sha256 with the new pinned versions"
    );
    assert!(
        vendor.join(field("license")).is_file(),
        "license file missing"
    );
    for package in [
        "@codemirror/state",
        "@codemirror/view",
        "@codemirror/language",
    ] {
        assert!(
            manifest
                .lines()
                .any(|l| l.starts_with(&format!("package {package} "))),
            "MANIFEST pins no version of {package}"
        );
    }
}

#[test]
fn page_modules_pass_their_node_unit_tests() {
    let Some(node) = node() else {
        skip("page_modules_pass_their_node_unit_tests: `node` is not on PATH");
        return;
    };
    let script = crate_dir().join("tests/editor_e2e/unit.js");
    run_node(&node, &script, &[]);
}

/// The built wasm engine module, when there is one.
fn wasm_module() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("ZENITH_E2E_WASM") {
        return Some(PathBuf::from(p)).filter(|p| p.is_file());
    }
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate_dir().join("../target"));
    Some(target.join("wasm32-wasip1/release-wasm/zenith-editor-wasm.wasm")).filter(|p| p.is_file())
}

/// Which engine the page runs on.
#[derive(Clone, Copy)]
enum Host {
    /// `zenith edit`, the native engine over HTTP.
    Server,
    /// The static site, the engine in a wasm Worker.
    Static,
}

/// Run the e2e step file `steps` on `example` in headless Chromium against
/// `host`, or skip with a message when `node`, Chromium, or (static) the
/// wasm module is missing.
fn end_to_end(test: &str, host: Host, steps: &str, example: &Path) {
    let (Some(node), Some(chromium)) = (node(), chromium()) else {
        skip(&format!(
            "{test}: needs `node` and a Chromium binary (chromium, chromium-browser, \
             google-chrome, or ZENITH_E2E_CHROMIUM) on PATH"
        ));
        return;
    };
    let root = crate_dir().join("..");
    let script = crate_dir().join("tests/editor_e2e/run.js");
    let zenith = PathBuf::from(env!("CARGO_BIN_EXE_zenith"));
    let wasm = wasm_module();
    let (out, engine_flag, engine_path): (PathBuf, &str, &Path) = match host {
        Host::Server => (
            root.join("conformance/editor"),
            "--zenith",
            zenith.as_path(),
        ),
        Host::Static => {
            let Some(wasm) = wasm.as_deref() else {
                skip(&format!(
                    "{test}: the wasm module is not built (see the header of this file)"
                ));
                return;
            };
            (root.join("conformance/editor/static"), "--wasm", wasm)
        }
    };
    std::fs::create_dir_all(&out).expect("create the screenshot directory");
    run_node(
        &node,
        &script,
        &[
            engine_flag.as_ref(),
            engine_path.as_os_str(),
            "--chromium".as_ref(),
            chromium.as_os_str(),
            "--example".as_ref(),
            example.as_os_str(),
            "--out".as_ref(),
            out.as_os_str(),
            "--steps".as_ref(),
            steps.as_ref(),
        ],
    );
}

#[test]
fn editor_page_end_to_end_in_chromium() {
    let example = crate_dir().join("../examples/stack.zen");
    end_to_end(
        "editor_page_end_to_end_in_chromium",
        Host::Server,
        "steps.js",
        &example,
    );
}

/// Canvas gestures: each edit's source diff touches only the edited node,
/// and the canvas equals the engine render of the new source.
#[test]
fn editor_gestures_end_to_end_in_chromium() {
    let example = crate_dir().join("tests/editor_e2e/gestures.zen");
    end_to_end(
        "editor_gestures_end_to_end_in_chromium",
        Host::Server,
        "gesture_steps.js",
        &example,
    );
}

/// Selections, marquee, snapping, path points, and inspector fields: each
/// edit's source diff touches only the edited nodes (and the tokens it
/// creates), and the canvas equals the engine render of the new source.
#[test]
fn editor_canvas_end_to_end_in_chromium() {
    let example = crate_dir().join("tests/editor_e2e/canvas.zen");
    end_to_end(
        "editor_canvas_end_to_end_in_chromium",
        Host::Server,
        "canvas_steps.js",
        &example,
    );
}

/// The selection and inspector suite on the static site.
#[test]
fn static_canvas_end_to_end_in_chromium() {
    let example = crate_dir().join("tests/editor_e2e/canvas.zen");
    end_to_end(
        "static_canvas_end_to_end_in_chromium",
        Host::Static,
        "canvas_steps.js",
        &example,
    );
}

/// The page suite on the static site: the same steps over the wasm engine,
/// plus the file open and save, fallback, font, and project-folder steps.
#[test]
fn static_page_end_to_end_in_chromium() {
    let example = crate_dir().join("../examples/stack.zen");
    end_to_end(
        "static_page_end_to_end_in_chromium",
        Host::Static,
        "steps.js",
        &example,
    );
}

/// The gesture suite on the static site.
#[test]
fn static_gestures_end_to_end_in_chromium() {
    let example = crate_dir().join("tests/editor_e2e/gestures.zen");
    end_to_end(
        "static_gestures_end_to_end_in_chromium",
        Host::Static,
        "gesture_steps.js",
        &example,
    );
}

/// `WasmEngine` over the real module in Node: envelope, events, stale
/// versions, lazy fonts, and file save, reload, and conflict.
#[test]
fn wasm_engine_passes_node_tests() {
    let (Some(node), Some(wasm)) = (node(), wasm_module()) else {
        skip("wasm_engine_passes_node_tests: needs `node` and the built wasm module");
        return;
    };
    let script = crate_dir().join("tests/editor_e2e/unit_wasm.js");
    let out = Command::new(&node)
        .arg(&script)
        .env("ZENITH_EDITOR_WASM", &wasm)
        .output()
        .expect("spawn node");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    println!("{stdout}");
    eprintln!("{stderr}");
    assert!(
        out.status.success(),
        "unit_wasm.js failed:\n{stdout}\n{stderr}"
    );
}

/// Native `zenith edit` and the wasm engine in Chromium give equal results
/// on every example: the `doc.render` PNG SHA-256 (and the PNG blob the page
/// builds), diagnostics, a viewport render, and a script of gestures with the
/// text after each edit.
#[test]
fn wasm_matches_native_on_every_example() {
    let (Some(node), Some(chromium)) = (node(), chromium()) else {
        skip("wasm_matches_native_on_every_example: needs `node` and a Chromium binary");
        return;
    };
    let Some(wasm) = wasm_module() else {
        skip("wasm_matches_native_on_every_example: the wasm module is not built");
        return;
    };
    let script = crate_dir().join("tests/editor_e2e/cross_target.js");
    let zenith = PathBuf::from(env!("CARGO_BIN_EXE_zenith"));
    let examples = crate_dir().join("../examples");
    run_node(
        &node,
        &script,
        &[
            "--zenith".as_ref(),
            zenith.as_os_str(),
            "--wasm".as_ref(),
            wasm.as_os_str(),
            "--chromium".as_ref(),
            chromium.as_os_str(),
            "--examples".as_ref(),
            examples.as_os_str(),
        ],
    );
}
