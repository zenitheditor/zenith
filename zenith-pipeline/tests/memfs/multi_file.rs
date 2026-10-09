//! A multi-file project held only in memory: a composition import with its
//! own image and font assets, a host image asset, and a text source.

use std::path::{Path, PathBuf};

use zenith_core::Diagnostic;
use zenith_pipeline::render::{render_png, render_scene_json};
use zenith_pipeline::{FsConfig, Host, MemFs, PolicyFlags, RenderOptions, SourceFs};

use super::disk::DiskFs;

const SWATCH: &[u8] = include_bytes!("../../../examples/assets/swatch.png");
const SERIF: &[u8] = include_bytes!("../../../zenith-core/assets/fonts/NotoSerif-Regular.ttf");

const HOST: &str = r##"zenith version=1 {
  project id="proj.host" name="Host"
  imports {
    import id="brand" kind="zen" src="brand/brand.zen"
  }
  assets {
    asset id="asset.photo" kind="image" src="assets/photo.png"
  }
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
    token id="color.ink" type="color" value="#111827"
    token id="font.body" type="fontFamily" value="Noto Sans"
    token id="size.body" type="dimension" value=(px)14
  }
  styles {}
  document id="doc.host" title="Host" {
    page id="page.host" w=(px)320 h=(px)200 background=(token)"color.bg" {
      image id="photo" asset="asset.photo" x=(px)200 y=(px)20 w=(px)100 h=(px)80 fit="stretch"
      instance id="card" source="brand#component.card" x=(px)20 y=(px)20
      text id="note" src="copy/note.txt" x=(px)20 y=(px)150 w=(px)280 h=(px)30 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body"
    }
  }
}
"##;

const BRAND: &str = r##"zenith version=1 {
  project id="proj.brand" name="Brand"
  assets {
    asset id="asset.logo" kind="image" src="logo.png"
    asset id="asset.serif" kind="font" src="fonts/serif.ttf"
  }
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#111827"
    token id="font.display" type="fontFamily" value="Noto Serif"
    token id="size.title" type="dimension" value=(px)24
  }
  styles {}
  document id="doc.brand" title="Brand" {
    page id="page.brand" w=(px)10 h=(px)10
  }
  components {
    component id="card" {
      image id="logo" asset="asset.logo" x=(px)0 y=(px)0 w=(px)64 h=(px)40 fit="stretch"
      text id="title" x=(px)0 y=(px)48 w=(px)160 h=(px)32 fill=(token)"color.ink" font-family=(token)"font.display" font-size=(token)"size.title" { span "Brand" }
    }
  }
}
"##;

/// The project files, as `(path relative to the project, bytes)`.
fn project_files() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("host.zen", HOST.as_bytes().to_vec()),
        ("assets/photo.png", SWATCH.to_vec()),
        ("copy/note.txt", b"Held only in memory".to_vec()),
        ("brand/brand.zen", BRAND.as_bytes().to_vec()),
        ("brand/logo.png", SWATCH.to_vec()),
        ("brand/fonts/serif.ttf", SERIF.to_vec()),
    ]
}

/// The project in memory under `root`, without the files in `skip`.
fn memory_project(root: &Path, skip: &[&str]) -> MemFs {
    let mut fs = MemFs::new();
    for (rel, bytes) in project_files() {
        if !skip.contains(&rel) {
            fs.insert(root.join(rel), bytes);
        }
    }
    fs
}

fn png(fs: &dyn SourceFs, root: &Path) -> (Vec<u8>, Vec<Diagnostic>) {
    let config = FsConfig::new(fs, None);
    let flags = PolicyFlags::default();
    let artifact = render_png(
        Host::new(fs, &config),
        HOST,
        Some(root),
        1,
        RenderOptions::new(&flags),
    )
    .expect("render");
    (artifact.png, artifact.diagnostics)
}

#[test]
fn imported_component_assets_and_font_resolve_from_memory() {
    let root = PathBuf::from("proj");
    let fs = memory_project(&root, &[]);
    let (bytes, diagnostics) = png(&fs, &root);
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert!(!Diagnostic::has_errors(&diagnostics), "{diagnostics:?}");

    let config = FsConfig::new(&fs, None);
    let flags = PolicyFlags::default();
    let scene = render_scene_json(
        Host::new(&fs, &config),
        HOST,
        Some(&root),
        1,
        RenderOptions::new(&flags),
    )
    .expect("scene");
    // The imported text uses the font asset registered under the import's
    // namespace, and the text source's contents reach the scene.
    assert!(
        scene.json.contains("brand/noto-serif"),
        "imported font face"
    );
    assert!(
        scene.json.contains("brand/asset.logo"),
        "imported image asset"
    );
}

#[test]
fn memory_project_matches_the_same_files_on_disk() {
    let dir = tempfile::tempdir().expect("tempdir");
    for (rel, bytes) in project_files() {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&path, bytes).expect("write");
    }
    let mem = memory_project(dir.path(), &[]);
    assert_eq!(png(&DiskFs, dir.path()), png(&mem, dir.path()));
}

#[test]
fn files_missing_from_memory_are_reported() {
    let root = PathBuf::from("proj");
    let fs = memory_project(&root, &["brand/fonts/serif.ttf", "copy/note.txt"]);
    let (_, diagnostics) = png(&fs, &root);
    let codes: Vec<&str> = diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert!(codes.contains(&"import.asset_missing"), "{codes:?}");
    assert!(codes.contains(&"text.src_missing"), "{codes:?}");
}
