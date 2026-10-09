mod common;
use common::parse;
use zenith_core::default_provider;
use zenith_scene::ir::{Paint, SceneCommand};
use zenith_scene::{CompileResult, compile, compile_page};

/// The solid-`FillRect` color red-channel values present in a scene.
fn fill_reds(result: &CompileResult) -> Vec<u8> {
    result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::FillRect {
                paint: Paint::Solid { color },
                ..
            } => Some(color.r),
            _ => None,
        })
        .collect()
}

/// A two-page document. Page 1 has a full-bleed rect filled `#252525`
/// (r=0x25); page 2 a full-bleed rect filled `#dcdcdc` (r=0xdc). The page
/// fill color uniquely identifies which page was compiled.
const TWO_PAGE_DOC: &str = r##"zenith version=1 {
  project id="proj.mp" name="MP"
  tokens format="zenith-token-v1" {
token id="color.p1" type="color" value="#252525"
token id="color.p2" type="color" value="#dcdcdc"
  }
  styles {}
  document id="doc.mp" title="MP" {
page id="page.p1" w=(px)100 h=(px)100 {
  rect id="rect.p1" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.p1"
}
page id="page.p2" w=(px)200 h=(px)200 {
  rect id="rect.p2" x=(px)0 y=(px)0 w=(px)200 h=(px)200 fill=(token)"color.p2"
}
  }
}
"##;

#[test]
fn json_schema_field_value() {
    let src = r##"zenith version=1 {
  project id="proj.t5" name="T5"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.t5" title="T5" {
page id="page.t5" w=(px)100 h=(px)100 {}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let json = result.scene.to_json().expect("serialize must succeed");
    assert!(
        json.contains(r#""schema": "zenith-scene-v1""#),
        "JSON must contain schema field; got snippet: {}",
        &json[..json.len().min(200)]
    );
}

#[test]
fn json_serialization_is_deterministic() {
    let src = r##"zenith version=1 {
  project id="proj.t6" name="T6"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#aabbcc"
  }
  styles {}
  document id="doc.t6" title="T6" {
page id="page.t6" w=(px)200 h=(px)100 {
  rect id="rect.t6" x=(px)10 y=(px)20 w=(px)100 h=(px)50 fill=(token)"color.fill"
}
  }
}
"##;
    let doc = parse(src);
    let r1 = compile(&doc, &default_provider());
    let r2 = compile(&doc, &default_provider());
    let j1 = r1.scene.to_json().expect("serialize 1");
    let j2 = r2.scene.to_json().expect("serialize 2");
    assert_eq!(
        j1, j2,
        "two compiles of the same doc must produce identical JSON"
    );
}

#[test]
fn compile_page_selects_second_page() {
    let doc = parse(TWO_PAGE_DOC);
    let result = compile_page(&doc, &default_provider(), 1, None);

    // Page 2 size is 200×200 (page 1 is 100×100).
    assert_eq!(result.scene.width, 200.0, "must be page 2's width");
    assert_eq!(result.scene.height, 200.0, "must be page 2's height");

    let reds = fill_reds(&result);
    assert!(
        reds.contains(&0xdc),
        "page-2 fill (#dc...) must be present; got {reds:?}"
    );
    assert!(
        !reds.contains(&0x25),
        "page-1-only fill (#25...) must be absent; got {reds:?}"
    );
}

#[test]
fn compile_page_out_of_range_is_empty_with_advisory() {
    let doc = parse(TWO_PAGE_DOC);
    let result = compile_page(&doc, &default_provider(), 9, None);

    assert_eq!(result.scene.width, 0.0, "out-of-range scene must be 0 wide");
    assert_eq!(
        result.scene.height, 0.0,
        "out-of-range scene must be 0 tall"
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "scene.page_out_of_range"),
        "out-of-range page must emit scene.page_out_of_range; got {:?}",
        result
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn compile_no_pages_still_yields_no_pages_advisory() {
    let src = r##"zenith version=1 {
  project id="proj.np" name="NP"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.np" title="NP" {}
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "scene.no_pages"),
        "empty document must emit scene.no_pages"
    );
}

#[test]
fn compile_equals_compile_page_zero() {
    let doc = parse(TWO_PAGE_DOC);
    let via_compile = compile(&doc, &default_provider());
    let via_page0 = compile_page(&doc, &default_provider(), 0, None);

    // compile renders page 1 (index 0): same dimensions and fills.
    assert_eq!(via_compile.scene.width, via_page0.scene.width);
    assert_eq!(via_compile.scene.height, via_page0.scene.height);
    assert_eq!(fill_reds(&via_compile), fill_reds(&via_page0));
    // And it is page 1, not page 2.
    assert!(fill_reds(&via_compile).contains(&0x25));
}
