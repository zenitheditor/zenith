use std::fs;

use zenith_cli::commands::render::{
    RenderEntryOptions, load_data_context, to_svg_all_pages_with_dir_options,
    to_svg_with_dir_options,
};
use zenith_cli::config::CliPolicyFlags;

fn options(flags: &CliPolicyFlags) -> RenderEntryOptions<'_> {
    RenderEntryOptions {
        locked: false,
        subset: true,
        flags,
        data: None,
        construction_overlay: false,
        scale: 1.0,
        raster_scale: 1.0,
    }
}

fn document(assets: &str, page: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.inputs" name="Inputs"
  assets {{ {assets} }}
  tokens format="zenith-token-v1" {{
    token id="color.ink" type="color" value="#ff0000"
  }}
  styles {{}}
  document id="doc.inputs" title="Inputs" {{
    page id="page.inputs" w=(px)120 h=(px)80 {{
{page}
    }}
  }}
}}
"##
    )
}

#[test]
fn svg_data_values_match_literal_scene_bytes() {
    let dir = tempfile::tempdir().expect("directory");
    let path = dir.path().join("data.json");
    fs::write(&path, r##"{"fill":"#ff0000"}"##).expect("write data");
    let data = load_data_context(&path).expect("load data");
    let src = document(
        "",
        r#"      rect id="rect.a" x=(px)0 y=(px)0 w=(px)80 h=(px)60 fill=(data)"fill""#,
    );
    let flags = CliPolicyFlags::default();
    let mut opts = options(&flags);
    opts.data = Some(&data);
    let from_data = to_svg_with_dir_options(&src, Some(dir.path()), 1, opts).expect("data SVG");
    let literal = src.replace(r#"(data)"fill""#, r#"(token)"color.ink""#);
    let from_literal = to_svg_with_dir_options(&literal, Some(dir.path()), 1, options(&flags))
        .expect("literal SVG");
    assert_eq!(from_data.svg, from_literal.svg);
    assert!(
        !from_data
            .diagnostics
            .iter()
            .any(|d| d.code.starts_with("data."))
    );
}

#[test]
fn svg_external_text_matches_inline_glyph_outlines() {
    let dir = tempfile::tempdir().expect("directory");
    fs::write(dir.path().join("copy.txt"), "SVG outlines").expect("write text");
    let external = document(
        "",
        r#"      text id="text.a" x=(px)0 y=(px)0 w=(px)110 h=(px)60 src="copy.txt""#,
    );
    let inline = external.replace(r#"src="copy.txt""#, r#"{ span "SVG outlines"; }"#);
    let flags = CliPolicyFlags::default();
    let from_file = to_svg_with_dir_options(&external, Some(dir.path()), 1, options(&flags))
        .expect("external text");
    let from_inline = to_svg_with_dir_options(&inline, Some(dir.path()), 1, options(&flags))
        .expect("inline text");
    assert_eq!(from_file.svg, from_inline.svg);
    let svg = String::from_utf8(from_file.svg).expect("UTF-8 SVG");
    assert!(svg.contains("<path"), "outlined glyphs: {svg}");
    assert!(!svg.contains("<text"));
    assert!(!svg.contains("copy.txt"));
    assert!(!svg.contains("data:image/png"));
    fs::remove_file(dir.path().join("copy.txt")).expect("remove text");
    let missing = to_svg_with_dir_options(&external, Some(dir.path()), 1, options(&flags));
    match missing {
        Ok(artifact) => assert!(
            artifact
                .diagnostics
                .iter()
                .any(|d| d.code == "text.src_missing" && d.is_error())
        ),
        Err(error) => assert!(
            error
                .diagnostics
                .iter()
                .any(|d| d.code == "text.src_missing")
        ),
    }
}

#[test]
fn svg_imported_page_keeps_geometry_and_document_dimensions() {
    let dir = tempfile::tempdir().expect("directory");
    let slide = document(
        "",
        r##"      rect id="mark" x=(px)10 y=(px)20 w=(px)30 h=(px)40 fill=(token)"color.ink""##,
    );
    fs::write(dir.path().join("slide.zen"), slide).expect("write slide");
    let src = r#"zenith version=1 {
  project id="proj.host" name="Host"
  imports {
    import id="slide" kind="zen" src="slide.zen"
  }
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.host" title="Host" {
    page id="page.host" source="slide#page.page.inputs" fit="fill" w=(px)240 h=(px)160
  }
}"#;
    let flags = CliPolicyFlags::default();
    let single =
        to_svg_with_dir_options(src, Some(dir.path()), 1, options(&flags)).expect("imported SVG");
    assert_eq!(
        (single.width, single.height, single.page),
        (240.0, 160.0, 1)
    );
    assert!(single.diagnostics.is_empty(), "{:?}", single.diagnostics);
    let batch = to_svg_all_pages_with_dir_options(src, Some(dir.path()), options(&flags))
        .expect("imported batch");
    assert_eq!(batch.pages.len(), 1);
    assert_eq!(single.svg, batch.pages[0].svg);
    let svg = String::from_utf8(single.svg).expect("UTF-8 SVG");
    assert!(svg.contains("<path"));
    assert!(svg.contains("matrix(2 0 0 2"), "import transform: {svg}");
    assert!(!svg.contains("slide.zen"));
}

#[test]
fn svg_image_is_embedded_and_locked_assets_keep_hash_checks() {
    let dir = tempfile::tempdir().expect("directory");
    let bytes = tiny_skia::Pixmap::new(8, 8)
        .expect("pixmap")
        .encode_png()
        .expect("PNG");
    fs::write(dir.path().join("image.png"), bytes).expect("write image");
    let src = document(
        r#"asset id="asset.a" kind="image" src="image.png""#,
        r#"      image id="image.a" asset="asset.a" x=(px)0 y=(px)0 w=(px)8 h=(px)8 fit="stretch""#,
    );
    let flags = CliPolicyFlags::default();
    let artifact = to_svg_with_dir_options(&src, Some(dir.path()), 1, options(&flags))
        .expect("embedded image");
    let svg = String::from_utf8(artifact.svg).expect("UTF-8 SVG");
    assert!(svg.contains("data:image/png;base64,"));
    assert!(!svg.contains("image.png"));
    let mut opts = options(&flags);
    opts.locked = true;
    assert!(
        to_svg_with_dir_options(&src, Some(dir.path()), 1, opts).is_err(),
        "locked assets require hashes"
    );
    let wrong_hash = src.replace(
        r#"src="image.png""#,
        &format!(r#"src="image.png" sha256="{}""#, "0".repeat(64)),
    );
    assert!(
        to_svg_with_dir_options(&wrong_hash, Some(dir.path()), 1, opts).is_err(),
        "locked assets check hashes"
    );
    fs::remove_file(dir.path().join("image.png")).expect("remove image");
    assert!(
        to_svg_with_dir_options(&src, Some(dir.path()), 1, options(&flags)).is_err(),
        "missing images cannot export"
    );
}

#[test]
fn svg_construction_overlay_is_deterministic_and_changes_scene() {
    let src = document(
        "",
        r##"      construction {
        guide id="axis" type="segment" x1=(px)0 y1=(px)40 x2=(px)120 y2=(px)40
      }
      rect id="rect.a" x=(px)10 y=(px)10 w=(px)30 h=(px)20 fill=(token)"color.ink""##,
    );
    let flags = CliPolicyFlags::default();
    let plain = to_svg_with_dir_options(&src, None, 1, options(&flags)).expect("plain SVG");
    let mut opts = options(&flags);
    opts.construction_overlay = true;
    let first = to_svg_with_dir_options(&src, None, 1, opts).expect("overlay SVG");
    let second = to_svg_with_dir_options(&src, None, 1, opts).expect("overlay SVG again");
    assert_ne!(plain.svg, first.svg);
    assert_eq!(first.svg, second.svg);
}
