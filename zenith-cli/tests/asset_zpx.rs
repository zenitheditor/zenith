use std::fs;
use std::process::Command;

fn minimal_doc() -> &'static str {
    r#"zenith version=1 {
  project id="proj.asset" name="Asset Test"
  tokens format="zenith-token-v1" { }
  styles { }
  assets { }
  document id="doc.asset" title="Asset Test" {
    page id="page.main" w=(px)100 h=(px)100 { }
  }
}"#
}

fn minimal_zpx_manifest() -> &'static str {
    r##"zpx version=1 {
    canvas width=4 height=4 color-space="srgb" alpha="premultiplied"
    layers {
        layer id="paint" blend="normal" opacity=1.0 visible=#true clipping=#false {
            source kind="program" {
                stroke color="#ff0000ff" opacity=1.0 blend="normal" seed=1 {
                    brush kind="round" radius=3.0 hardness=1.0 spacing=1.0
                    sample x=2.0 y=2.0 pressure=1.0
                }
            }
        }
    }
}"##
}

fn asset_zpx_command<'a>(manifest: &'a std::path::Path, doc: &'a std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_zenith"));
    command
        .arg("asset")
        .arg("zpx-bake")
        .arg(manifest)
        .arg("--into")
        .arg(doc)
        .arg("--id")
        .arg("asset.paint")
        .arg("--src")
        .arg("assets/paint.png");
    command
}

#[test]
fn asset_zpx_dry_run_does_not_write_asset_or_doc() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let doc = tmp.path().join("poster.zen");
    let manifest = tmp.path().join("paint.zpx");
    fs::write(&doc, minimal_doc()).expect("write doc");
    fs::write(&manifest, minimal_zpx_manifest()).expect("write manifest");

    let output = asset_zpx_command(&manifest, &doc)
        .output()
        .expect("run zenith");

    assert!(output.status.success());
    assert_eq!(fs::read_to_string(&doc).expect("read doc"), minimal_doc());
    assert!(!tmp.path().join("assets/paint.png").exists());
}

#[test]
fn asset_zpx_apply_writes_png_asset_and_doc_sha256() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let doc = tmp.path().join("poster.zen");
    let manifest = tmp.path().join("paint.zpx");
    fs::write(&doc, minimal_doc()).expect("write doc");
    fs::write(&manifest, minimal_zpx_manifest()).expect("write manifest");

    let output = asset_zpx_command(&manifest, &doc)
        .arg("--apply")
        .output()
        .expect("run zenith");

    assert!(output.status.success());
    let png = fs::read(tmp.path().join("assets/paint.png")).expect("read asset");
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
    let updated = fs::read_to_string(&doc).expect("read doc");
    assert!(updated.contains(r#"id="asset.paint""#));
    assert!(updated.contains(r#"kind="image""#));
    assert!(updated.contains(r#"src="assets/paint.png""#));
    assert!(updated.contains("sha256="));
}
