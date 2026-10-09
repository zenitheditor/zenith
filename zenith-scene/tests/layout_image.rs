//! Auto-layout of `image` children: a hugging image takes its asset's pixel
//! size, and one fixed axis keeps the asset's aspect ratio.

mod common;
use std::collections::BTreeMap;

use common::parse;
use zenith_core::default_provider;
use zenith_scene::{DocumentPrep, PageCompiler};

fn doc(children: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.lim" name="LIM"
  assets {{
    asset id="asset.photo" kind="image" src="photo.png"
    asset id="asset.unknown" kind="image" src="unknown.png"
  }}
  tokens format="zenith-token-v1" {{
    token id="color.k" type="color" value="#000000"
  }}
  styles {{}}
  document id="doc.lim" title="LIM" {{
    page id="p" w=(px)800 h=(px)600 {{
{children}
    }}
  }}
}}
"##
    )
}

/// Layout boxes `(x, y, w, h)` by node id.
type Boxes = BTreeMap<String, (f64, f64, f64, f64)>;

/// Layout boxes and compile diagnostic codes with `asset.photo` at 400x200 px.
fn lay_out(children: &str) -> (Boxes, Vec<String>) {
    let parsed = parse(&doc(children));
    let fonts = default_provider();
    let sizes = BTreeMap::from([("asset.photo".to_owned(), (400.0, 200.0))]);
    let prep = DocumentPrep::new(&parsed, None, None).with_image_sizes(sizes);
    let compiler = PageCompiler::new(&prep, &fonts);
    let boxes = compiler
        .layout_boxes(0)
        .expect("page 0")
        .iter()
        .map(|(k, b)| (k.clone(), (b.x, b.y, b.w, b.h)))
        .collect();
    let codes = compiler
        .compile_page(0)
        .diagnostics
        .iter()
        .map(|d| d.code.clone())
        .collect();
    (boxes, codes)
}

#[test]
fn hugging_image_takes_its_pixel_size() {
    let (b, codes) = lay_out(
        r#"      frame id="f" x=(px)10 y=(px)20 layout="row" align="start" {
        image id="i" asset="asset.photo"
      }"#,
    );
    assert_eq!(b.get("i"), Some(&(10.0, 20.0, 400.0, 200.0)));
    assert!(!codes.iter().any(|c| c.starts_with("layout.")), "{codes:?}");
}

#[test]
fn one_fixed_axis_keeps_the_aspect() {
    let (b, _) = lay_out(
        r#"      frame id="f" x=(px)0 y=(px)0 layout="row" align="start" gap=(px)10 {
        image id="by-h" asset="asset.photo" h=(px)50
        image id="by-w" asset="asset.photo" w=(px)60
      }"#,
    );
    assert_eq!(b.get("by-h"), Some(&(0.0, 0.0, 100.0, 50.0)));
    assert_eq!(b.get("by-w"), Some(&(110.0, 0.0, 60.0, 30.0)));
}

#[test]
fn fill_width_derives_the_height() {
    let (b, _) = lay_out(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)300 layout="column" {
        image id="i" asset="asset.photo"
      }"#,
    );
    assert_eq!(b.get("i"), Some(&(0.0, 0.0, 300.0, 150.0)));
    assert_eq!(b.get("f"), Some(&(0.0, 0.0, 300.0, 150.0)));
}

#[test]
fn image_without_a_known_size_is_unsized() {
    let (_, codes) = lay_out(
        r#"      frame id="f" x=(px)0 y=(px)0 layout="row" {
        image id="i" asset="asset.unknown"
      }"#,
    );
    assert!(
        codes.iter().any(|c| c == "layout.unsized_child"),
        "{codes:?}"
    );
}
