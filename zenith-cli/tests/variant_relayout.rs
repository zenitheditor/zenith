//! `zenith variant` with overrides that re-layout.
//!
//! - `examples/variants.zen` retargets one auto-layout square to a story and
//!   a banner. Every generated variant renders and validates with no
//!   diagnostics.
//! - A document whose overrides use only the older keys generates the same
//!   bytes as the release before layout overrides. The hashes below come from
//!   that release.

use std::path::Path;

use sha2::{Digest, Sha256};
use zenith_cli::commands::variant::run_variant;
use zenith_core::{
    Document, KdlAdapter, KdlSource, LayoutKind, Node, PropertyValue, SizeKeyword, validate,
};

/// The examples directory, relative to the crate directory (the test cwd).
const EXAMPLES: &str = "../examples";

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn find<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Node> {
    nodes.iter().find_map(|n| {
        if n.id() == Some(id) {
            return Some(n);
        }
        n.children().and_then(|c| find(c, id))
    })
}

fn frame<'a>(doc: &'a Document, id: &str) -> &'a zenith_core::FrameNode {
    match find(&doc.body.pages[0].children, id) {
        Some(Node::Frame(f)) => f,
        other => panic!("{id} must be a frame, got {other:?}"),
    }
}

fn read_variant(dir: &Path, id: &str) -> (String, Document) {
    let src = std::fs::read_to_string(dir.join(format!("variants-{id}.zen")))
        .unwrap_or_else(|e| panic!("read variants-{id}.zen: {e}"));
    let doc = KdlAdapter
        .parse(src.as_bytes())
        .unwrap_or_else(|e| panic!("variants-{id}.zen must parse: {}", e.message));
    (src, doc)
}

#[test]
fn example_retargets_to_three_clean_variants() {
    let src = std::fs::read_to_string(Path::new(EXAMPLES).join("variants.zen"))
        .expect("read examples/variants.zen");
    let source = KdlAdapter.parse(src.as_bytes()).expect("example parses");
    assert!(
        validate(&source).diagnostics.is_empty(),
        "the source must validate clean: {:?}",
        validate(&source).diagnostics
    );

    let out = tempfile::TempDir::new().expect("tempdir");
    let report = run_variant(&src, Some(Path::new(EXAMPLES)), out.path(), "variants")
        .expect("run_variant succeeds");
    let ids: Vec<&str> = report.variants.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["banner", "square", "story"]);
    for record in &report.variants {
        assert!(
            record.failure.is_none(),
            "{}: {:?}",
            record.id,
            record.failure
        );
        assert!(
            record.diagnostics.is_empty(),
            "{}: render diagnostics {:?}",
            record.id,
            record.diagnostics
        );
    }

    for (id, w, h) in [
        ("banner", 1920.0, 1080.0),
        ("square", 1080.0, 1080.0),
        ("story", 1080.0, 1920.0),
    ] {
        let (_, doc) = read_variant(out.path(), id);
        let page = &doc.body.pages[0];
        assert_eq!((page.width.value, page.height.value), (w, h), "{id}");
        assert!(
            validate(&doc).diagnostics.is_empty(),
            "{id}: the generated .zen must validate clean: {:?}",
            validate(&doc).diagnostics
        );
        assert!(doc.variants.is_empty(), "{id}: no variants block in output");
    }

    let (story_src, story) = read_variant(out.path(), "story");
    assert_eq!(frame(&story, "stats").layout, Some(LayoutKind::Column));
    assert_eq!(frame(&story, "root").h.as_ref().map(px), Some(Some(1920.0)));
    assert!(story_src.contains(r#"font-size=(token)"size.title.story""#));
    // The square size and the banner size are now unused, so they are gone.
    assert!(!story_src.contains(r#"token id="size.title.banner""#));
    assert!(!story_src.contains(r#"token id="size.title" "#));

    let (banner_src, banner) = read_variant(out.path(), "banner");
    assert_eq!(frame(&banner, "main").layout, Some(LayoutKind::Row));
    assert_eq!(
        frame(&banner, "hero").layout_item.w_keyword,
        Some(SizeKeyword::Fill)
    );
    assert!(banner_src.contains(r#"font-size=(token)"size.title.banner""#));

    let (square_src, _) = read_variant(out.path(), "square");
    assert!(square_src.contains(r#"font-size=(token)"size.title""#));
    assert!(!square_src.contains(r#"token id="size.title.story""#));
}

fn px(pv: &PropertyValue) -> Option<f64> {
    match pv {
        PropertyValue::Dimension(d) => Some(d.value),
        PropertyValue::TokenRef(_) | PropertyValue::Literal(_) | PropertyValue::DataRef(_) => None,
    }
}

/// Overrides that use only `visible`, `x`, `y`, `w`, `h`, `fill`, and `text`.
const LEGACY: &str = r##"zenith version=1 {
  project id="proj.legacy" name="Legacy Overrides"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
    token id="color.ink" type="color" value="#111111"
    token id="color.alt" type="color" value="#3b82f6"
    token id="font.sans" type="fontFamily" value="Noto Sans"
    token id="size.body" type="dimension" value=(px)24
  }
  styles {}
  variants {
    variant id="wide" source="page.a" w=(px)1200 h=(px)600 {
      override node="rect.bg" w=(px)1200 h=(px)600 fill=(token)"color.alt"
      override node="text.label" x=(px)40 y=(px)40 text="Wide variant"
    }
    variant id="tall" source="page.a" w=(px)600 h=(px)1200 {
      override node="rect.chip" visible=#false
      override node="rect.bg" h=(px)1200
    }
  }
  document id="doc.legacy" title="Legacy Overrides" {
    page id="page.a" w=(px)800 h=(px)600 background=(token)"color.bg" {
      rect id="rect.bg" x=(px)0 y=(px)0 w=(px)800 h=(px)600 fill=(token)"color.bg"
      rect id="rect.chip" x=(px)600 y=(px)40 w=(px)160 h=(px)60 fill=(token)"color.alt"
      text id="text.label" x=(px)20 y=(px)20 w=(px)500 h=(px)60 fill=(token)"color.ink" font-family=(token)"font.sans" font-size=(token)"size.body" {
        span "Original label"
      }
    }
  }
}
"##;

#[test]
fn legacy_overrides_generate_the_same_bytes_as_before() {
    let out = tempfile::TempDir::new().expect("tempdir");
    let report = run_variant(LEGACY, None, out.path(), "legacy").expect("run_variant succeeds");
    assert_eq!(report.generated(), 2, "{:?}", report.variants);
    for (name, expected) in [
        (
            "legacy-tall.png",
            "7952e0608aedc1a75aaf7264e953152c2c5008a8bd368f8c16546f407ade03f7",
        ),
        (
            "legacy-tall.zen",
            "58c42d6034689e7d176b41d7e65ed64edd2282e6ca4ed5943e043e080a93fce8",
        ),
        (
            "legacy-wide.png",
            "a2c6b91ca7fdcd94b219f2dd241a08e30468e3feef3884b3944bb5cf5af3db8f",
        ),
        (
            "legacy-wide.zen",
            "d69e066b411c896a477675696dd38794adb47102245fe7f662346d38f89c03dc",
        ),
    ] {
        let bytes = std::fs::read(out.path().join(name)).expect("read output");
        assert_eq!(sha256(&bytes), expected, "{name} changed");
    }
}

#[test]
fn literal_font_size_fails_only_that_variant() {
    let src = LEGACY.replace(
        r#"override node="rect.chip" visible=#false"#,
        r#"override node="text.label" font-size=(px)40"#,
    );
    let out = tempfile::TempDir::new().expect("tempdir");
    let report = run_variant(&src, None, out.path(), "legacy").expect("run_variant succeeds");
    let tall = report
        .variants
        .iter()
        .find(|r| r.id == "tall")
        .expect("tall present");
    let reason = tall.failure.as_deref().expect("tall fails");
    assert!(
        reason.contains("font-size must be a dimension token ref"),
        "{reason}"
    );
    let wide = report
        .variants
        .iter()
        .find(|r| r.id == "wide")
        .expect("wide present");
    assert!(wide.failure.is_none(), "{:?}", wide.failure);
}
