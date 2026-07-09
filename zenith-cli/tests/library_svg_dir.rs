//! Integration tests for SVG icon libraries — the plug-and-install pack format.
//!
//! A directory of `*.svg` files under `<project>/libraries/` IS a pack. These
//! tests exercise that end to end through the binary: a hand-made icon folder
//! with no manifest at all, then the same folder with a `library.kdl`.

use std::path::Path;
use std::process::Command;

/// A two-path icon, so conversion yields more than one `path` node.
const ROCKET_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M4 20l4-4"/><path d="M12 2l6 6-8 8-6-6z"/></svg>"#;

const ORBIT_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 12h18"/></svg>"#;

fn zenith(args: &[&str]) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .args(args)
        .output()
        .expect("run zenith");
    (
        output.status.success(),
        String::from_utf8(output.stdout).expect("stdout utf8"),
        String::from_utf8(output.stderr).expect("stderr utf8"),
    )
}

/// Build a project directory holding one SVG icon library, optionally with a
/// `library.kdl` manifest.
fn project_with_icons(dir: &Path, manifest: Option<&str>) {
    let icons = dir.join("libraries").join("my-icons");
    std::fs::create_dir_all(&icons).expect("create icon dir");
    std::fs::write(icons.join("rocket-launch.svg"), ROCKET_SVG).expect("write rocket");
    std::fs::write(icons.join("orbit-path.svg"), ORBIT_SVG).expect("write orbit");
    if let Some(manifest) = manifest {
        std::fs::write(icons.join("library.kdl"), manifest).expect("write manifest");
    }
}

#[test]
fn a_bare_svg_directory_is_a_pack_with_no_manifest() {
    let dir = tempfile::tempdir().expect("tempdir");
    project_with_icons(dir.path(), None);
    let path = dir.path().to_string_lossy().into_owned();

    let (ok, stdout, _) = zenith(&["library", "list", &path, "--json"]);
    assert!(ok);
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("json");
    let pack = value["packs"]
        .as_array()
        .expect("packs")
        .iter()
        .find(|p| p["id"] == "@local/my-icons")
        .expect("the icon directory is a pack, addressed @local/<dirname>");

    assert_eq!(pack["source"], "project");
    assert_eq!(pack["format"], "svg");
    // No manifest ⇒ no version, no license.
    assert!(pack["version"].is_null());
    assert!(pack["license"].is_null());

    // Items are the file stems, sorted.
    let ids: Vec<&str> = pack["items"]
        .as_array()
        .expect("items")
        .iter()
        .filter_map(|i| i["id"].as_str())
        .collect();
    assert_eq!(ids, ["orbit-path", "rocket-launch"]);
}

#[test]
fn a_bare_svg_directory_is_searchable_by_filename_tokens() {
    let dir = tempfile::tempdir().expect("tempdir");
    project_with_icons(dir.path(), None);
    let path = dir.path().to_string_lossy().into_owned();

    // With no manifest, the stem's `-`-separated tokens are the only index.
    for query in ["rocket", "launch", "rocket-launch"] {
        let (ok, stdout, _) = zenith(&["library", "search", query, &path]);
        assert!(ok);
        assert!(
            stdout.contains("@local/my-icons#rocket-launch"),
            "query {query:?} did not find the icon: {stdout}"
        );
    }
}

#[test]
fn a_manifest_adds_identity_and_search_metadata() {
    let dir = tempfile::tempdir().expect("tempdir");
    project_with_icons(
        dir.path(),
        Some(
            r#"library id="@acme/space" version="0.3.0" {
  license "CC0-1.0"
  icon "rocket-launch" aliases="liftoff" tags="spaceship booster" categories="space"
}
"#,
        ),
    );
    let path = dir.path().to_string_lossy().into_owned();

    let (ok, stdout, _) = zenith(&["library", "list", &path, "--json"]);
    assert!(ok);
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("json");
    let pack = value["packs"]
        .as_array()
        .expect("packs")
        .iter()
        .find(|p| p["id"] == "@acme/space")
        .expect("manifest id wins over the directory name");
    assert_eq!(pack["version"], "0.3.0");
    assert_eq!(pack["license"], "CC0-1.0");

    // An icon with no manifest entry still exists.
    let ids: Vec<&str> = pack["items"]
        .as_array()
        .expect("items")
        .iter()
        .filter_map(|i| i["id"].as_str())
        .collect();
    assert!(ids.contains(&"orbit-path"), "unlisted icons still exist");

    // The alias is findable, and outranks nothing else here.
    let (ok, stdout, _) = zenith(&["library", "search", "liftoff", &path]);
    assert!(ok);
    assert!(
        stdout.contains("@acme/space#rocket-launch"),
        "alias search: {stdout}"
    );

    // Categories filter.
    let (ok, stdout, _) = zenith(&["library", "search", "rocket", &path, "--category", "space"]);
    assert!(ok);
    assert!(
        stdout.contains("@acme/space#rocket-launch"),
        "got: {stdout}"
    );

    let (ok, stdout, _) = zenith(&[
        "library",
        "search",
        "rocket",
        &path,
        "--category",
        "weather",
    ]);
    assert!(ok);
    assert!(stdout.contains("no library items matched"), "got: {stdout}");
}

#[test]
fn a_malformed_manifest_degrades_metadata_without_hiding_icons() {
    let dir = tempfile::tempdir().expect("tempdir");
    project_with_icons(dir.path(), Some("library id=\"unterminated"));
    let path = dir.path().to_string_lossy().into_owned();

    let (ok, stdout, stderr) = zenith(&["library", "list", &path, "--json"]);
    assert!(ok, "a bad manifest must not fail the listing");
    assert!(
        stderr.contains("library.kdl"),
        "a note is written: {stderr}"
    );

    let value: serde_json::Value = serde_json::from_str(&stdout).expect("json");
    let pack = value["packs"]
        .as_array()
        .expect("packs")
        .iter()
        .find(|p| p["id"] == "@local/my-icons")
        .expect("icons remain listed under the fallback id");
    assert_eq!(pack["items"].as_array().expect("items").len(), 2);
}

#[test]
fn an_svg_dir_icon_materializes_into_a_document_as_native_paths() {
    let dir = tempfile::tempdir().expect("tempdir");
    project_with_icons(dir.path(), None);
    let doc = dir.path().join("doc.zen");
    let doc_path = doc.to_string_lossy().into_owned();

    let (ok, _, err) = zenith(&["new", &doc_path, "--name", "Icons"]);
    assert!(ok, "new failed: {err}");

    let (ok, stdout, err) = zenith(&[
        "library",
        "add",
        "@local/my-icons#rocket-launch",
        "--into",
        &doc_path,
        "--page",
        "page.1",
        "--at",
        "20,20",
    ]);
    assert!(ok, "add failed: {err}");
    assert!(
        stdout.contains("added @local/my-icons#rocket-launch"),
        "got: {stdout}"
    );

    let source = std::fs::read_to_string(&doc).expect("read doc");
    assert!(
        source.contains("component id=\"lib.local.my-icons.rocket-launch\""),
        "component landed: {source}"
    );
    // Geometry is converted to native paths, not embedded as an SVG asset.
    assert_eq!(
        source.matches("path id=").count(),
        2,
        "both subpaths landed"
    );
    assert!(!source.contains(".svg"), "no SVG asset reference");

    // And the result validates.
    let (ok, stdout, _) = zenith(&["validate", &doc_path]);
    assert!(ok, "validate failed: {stdout}");
}

/// A project pack shadows an embedded preset of the same id — the rule is
/// format-blind, so an SVG directory can shadow a bundled `.zen` pack.
#[test]
fn a_project_svg_library_shadows_an_embedded_pack_of_the_same_id() {
    let dir = tempfile::tempdir().expect("tempdir");
    project_with_icons(
        dir.path(),
        Some("library id=\"@zenith/icons-lucide\" version=\"99.0.0\"\n"),
    );
    let path = dir.path().to_string_lossy().into_owned();

    let (ok, stdout, _) = zenith(&["library", "list", &path, "--json"]);
    assert!(ok);
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("json");
    let first = value["packs"]
        .as_array()
        .expect("packs")
        .iter()
        .find(|p| p["id"] == "@zenith/icons-lucide")
        .expect("pack present");
    // Project sorts ahead of preset on an id tie, so it is the materializing winner.
    assert_eq!(first["source"], "project");
    assert_eq!(first["version"], "99.0.0");
}
