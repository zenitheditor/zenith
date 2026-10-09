//! Auto-layout sizes a hugging `image` from its asset file's pixel size on
//! every CLI compile path (scene JSON, PNG render, validate).

use tempfile::TempDir;
use zenith_cli::commands::render::{to_png_with_dir, to_scene_json};
use zenith_pipeline::PolicyFlags;

fn write_fixtures(dir: &TempDir) {
    let pixmap = tiny_skia::Pixmap::new(120, 60).expect("pixmap");
    std::fs::write(
        dir.path().join("photo.png"),
        pixmap.encode_png().expect("png"),
    )
    .expect("write png");
    std::fs::write(
        dir.path().join("logo.svg"),
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40"><rect width="80" height="40"/></svg>"#,
    )
    .expect("write svg");
}

const DOC: &str = r##"zenith version=1 {
  project id="proj.lia" name="LIA"
  assets {
    asset id="asset.photo" kind="image" src="photo.png"
    asset id="asset.logo" kind="svg" src="logo.svg"
  }
  tokens format="zenith-token-v1" {
    token id="color.k" type="color" value="#000000"
  }
  styles {}
  document id="doc.lia" title="LIA" {
    page id="p" w=(px)400 h=(px)300 {
      frame id="f" x=(px)10 y=(px)20 layout="row" gap=(px)10 align="start" {
        image id="photo" asset="asset.photo"
        image id="logo" asset="asset.logo" h=(px)20
      }
    }
  }
}
"##;

/// `(x, y, w, h)` of every image draw in a scene JSON, in command order.
fn image_boxes(json: &str) -> Vec<(f64, f64, f64, f64)> {
    let scene: serde_json::Value = serde_json::from_str(json).expect("scene json");
    scene["commands"]
        .as_array()
        .expect("commands")
        .iter()
        .filter(|c| c.get("asset_id").is_some())
        .map(|c| {
            let f = |k: &str| c[k].as_f64().expect("number");
            (f("x"), f("y"), f("w"), f("h"))
        })
        .collect()
}

#[test]
fn scene_json_sizes_hugging_images_from_their_files() {
    let dir = TempDir::new().expect("tempdir");
    write_fixtures(&dir);
    let artifact =
        to_scene_json(DOC, Some(dir.path()), 1, &PolicyFlags::default(), None).expect("scene json");
    assert!(
        !artifact
            .diagnostics
            .iter()
            .any(|d| d.code.starts_with("layout.")),
        "{:?}",
        artifact.diagnostics
    );
    // The png at its pixel size; the svg at h=20 keeps its 2:1 aspect.
    assert_eq!(
        image_boxes(&artifact.json),
        vec![(10.0, 20.0, 120.0, 60.0), (140.0, 20.0, 40.0, 20.0)]
    );
}

#[test]
fn png_render_sizes_hugging_images_from_their_files() {
    let dir = TempDir::new().expect("tempdir");
    write_fixtures(&dir);
    let artifact = to_png_with_dir(
        DOC,
        Some(dir.path()),
        1,
        false,
        &PolicyFlags::default(),
        None,
    )
    .expect("png");
    assert!(
        !artifact
            .diagnostics
            .iter()
            .any(|d| d.code.starts_with("layout.")),
        "{:?}",
        artifact.diagnostics
    );
}

#[test]
fn scene_json_without_project_dir_reports_unsized_images() {
    let artifact = to_scene_json(DOC, None, 1, &PolicyFlags::default(), None);
    let diagnostics = match artifact {
        Ok(a) => a.diagnostics,
        Err(e) => e.diagnostics,
    };
    assert!(
        diagnostics.iter().any(|d| d.code == "layout.unsized_child"),
        "{diagnostics:?}"
    );
}

/// A layout frame that lives only in a component, instanced on a page with no
/// layout frame of its own.
const COMPONENT_DOC: &str = r##"zenith version=1 {
  project id="proj.liac" name="LIAC"
  assets {
    asset id="asset.photo" kind="image" src="photo.png"
  }
  tokens format="zenith-token-v1" {
    token id="color.k" type="color" value="#000000"
  }
  styles {}
  components {
    component id="comp.card" {
      frame id="card" x=(px)0 y=(px)0 layout="row" align="start" {
        image id="photo" asset="asset.photo"
      }
    }
  }
  document id="doc.liac" title="LIAC" {
    page id="p" w=(px)400 h=(px)300 {
      instance id="c" component="comp.card" x=(px)10 y=(px)20
    }
  }
}
"##;

#[test]
fn component_layout_sizes_images_on_a_page_without_layout() {
    let dir = TempDir::new().expect("tempdir");
    write_fixtures(&dir);
    let artifact = to_scene_json(
        COMPONENT_DOC,
        Some(dir.path()),
        1,
        &PolicyFlags::default(),
        None,
    )
    .expect("scene json");
    assert!(
        !artifact
            .diagnostics
            .iter()
            .any(|d| d.code.starts_with("layout.")),
        "{:?}",
        artifact.diagnostics
    );
    let boxes = image_boxes(&artifact.json);
    assert_eq!(boxes.len(), 1, "{boxes:?}");
    let (_, _, w, h) = boxes[0];
    assert_eq!((w, h), (120.0, 60.0));
}
