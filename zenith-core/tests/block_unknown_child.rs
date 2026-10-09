//! Integration tests for the `block.unknown_child` Error: an unknown child node
//! inside a structural block is captured at parse time and reported, with a
//! did-you-mean over the block's accepted children, from validation.

#[path = "common/has_code.rs"]
mod has_code;

use has_code::has_code;
use zenith_core::{Document, KdlAdapter, KdlSource, Severity, validate};

/// Parse a document with `top` spliced in before `document` and `page_body`
/// inside the single page. Parsing is lenient, so malformed blocks still parse.
fn parse_doc(top: &str, page_body: &str) -> Document {
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.bu" name="Block unknown child"
  tokens format="zenith-token-v1" {{
  }}
  styles {{
  }}
  {top}
  document id="doc.bu" title="BU" {{
    page id="page.bu" w=(px)400 h=(px)300 {{
      {page_body}
    }}
  }}
}}
"##
    );
    KdlAdapter
        .parse(src.as_bytes())
        .expect("parse must succeed")
}

/// Validate and return every `block.unknown_child` message.
fn messages(top: &str, page_body: &str) -> Vec<String> {
    validate(&parse_doc(top, page_body))
        .diagnostics
        .iter()
        .filter(|d| d.code == "block.unknown_child")
        .map(|d| {
            assert_eq!(d.severity, Severity::Error, "{}", d.message);
            d.message.clone()
        })
        .collect()
}

fn flags(top: &str, page_body: &str, parent: &str, child: &str) {
    let found = messages(top, page_body);
    let needle = format!("unknown child '{child}'");
    assert!(
        found
            .iter()
            .any(|m| m.contains(parent) && m.contains(&needle)),
        "expected {parent}/{child}, got {found:?}"
    );
}

#[test]
fn well_formed_blocks_emit_nothing() {
    let top = r##"
  brand { colors "#ffffff"; fonts "Noto Sans"; weights 400 }
  sections { section id="s1" name="One" start-page="page.bu" }
  libraries { library id="lib" version="1" }
  provenance { origin id="o1" node="n" library="lib" }
  variants { variant id="v1" source="page.bu" w=(px)10 h=(px)10 { override node="n" visible=#true } }
  recipes { recipe id="r1" kind="k" { param name="a" value=1; palette token="c"; expanded node="n" } }
  diagnostics { allow "layout.off_canvas" }
  assets { }
  imports { }
  components { component id="c1" { ports { port node="n" id="p" anchor="center" } } }
"##;
    let found = messages(top, "");
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn unknown_top_level_child_of_zenith() {
    flags("bogus", "", "zenith", "bogus");
}

#[test]
fn near_miss_gets_did_you_mean() {
    let found = messages(r##"brand { color "#ffffff" }"##, "");
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("did you mean 'colors'?"), "{}", found[0]);
    assert!(
        found[0].contains("Allowed children: colors, fonts, weights"),
        "{}",
        found[0]
    );
}

#[test]
fn brand_unknown_child() {
    flags("brand { palette \"x\" }", "", "brand", "palette");
}

#[test]
fn assets_unknown_child() {
    flags("assets { icon id=\"a\" }", "", "assets", "icon");
}

#[test]
fn asset_unknown_child() {
    flags(
        r##"assets { asset id="a" kind="image" src="a.png" { extra } }"##,
        "",
        "asset",
        "extra",
    );
}

#[test]
fn libraries_unknown_child() {
    flags("libraries { lib id=\"l\" }", "", "libraries", "lib");
}

#[test]
fn imports_and_import_unknown_children() {
    flags("imports { include id=\"i\" }", "", "imports", "include");
    flags(
        r##"imports { import id="i" kind="zen" src="a.zen" { token-mapp from="a" to="b" } }"##,
        "",
        "import",
        "token-mapp",
    );
}

#[test]
fn actions_unknown_child() {
    flags("actions { act id=\"a\" }", "", "actions", "act");
    flags(
        r##"actions { action id="a" { tx "{}"; extra } }"##,
        "",
        "action",
        "extra",
    );
}

#[test]
fn project_unknown_child() {
    let src = r##"zenith version=1 {
  project id="p" name="n" { writer "x" }
  document id="d" { page id="pg" w=(px)10 h=(px)10 { } }
}"##;
    let doc = KdlAdapter.parse(src.as_bytes()).expect("parse");
    let report = validate(&doc);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "block.unknown_child" && d.message.contains("'writer'")),
        "{:?}",
        report.diagnostics
    );
}

#[test]
fn sections_and_section_unknown_children() {
    flags("sections { sect id=\"s\" }", "", "sections", "sect");
    flags(
        r##"sections { section id="s" name="n" start-page="page.bu" { extra } }"##,
        "",
        "section",
        "extra",
    );
}

#[test]
fn provenance_unknown_child() {
    flags("provenance { orgin id=\"o\" }", "", "provenance", "orgin");
}

#[test]
fn variants_unknown_children() {
    flags(
        "variants { variation id=\"v\" }",
        "",
        "variants",
        "variation",
    );
    flags(
        r##"variants { variant id="v" source="page.bu" w=(px)1 h=(px)1 { overide node="n" } }"##,
        "",
        "variant",
        "overide",
    );
}

#[test]
fn recipes_unknown_children() {
    flags(
        "recipes { recipee id=\"r\" kind=\"k\" }",
        "",
        "recipes",
        "recipee",
    );
    flags(
        r##"recipes { recipe id="r" kind="k" { parm name="a" value=1 } }"##,
        "",
        "recipe",
        "parm",
    );
}

#[test]
fn diagnostics_unknown_verb() {
    let found = messages(r##"diagnostics { alow "layout.off_canvas" }"##, "");
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("did you mean 'allow'?"), "{}", found[0]);
}

#[test]
fn styles_unknown_child() {
    flags("styles { stile id=\"s\" }", "", "styles", "stile");
}

#[test]
fn tokens_children_depend_on_type() {
    flags(
        r##"tokens format="zenith-token-v1" { token id="c" type="color" value="#ffffff" { stop } }"##,
        "",
        "token",
        "stop",
    );
    flags(
        r##"tokens format="zenith-token-v1" { token id="f" type="filter" { sepia; blurr } }"##,
        "",
        "token",
        "blurr",
    );
}

#[test]
fn components_unknown_child() {
    flags(
        "components { componnt id=\"c\" }",
        "",
        "components",
        "componnt",
    );
    flags(
        r##"components { component id="c" { ports { prt node="n" } } }"##,
        "",
        "ports",
        "prt",
    );
}

#[test]
fn masters_unknown_child() {
    flags("masters { mastr id=\"m\" }", "", "masters", "mastr");
}

#[test]
fn document_and_page_structural_children() {
    flags(
        "",
        r##"construction { gide id="g1" }"##,
        "construction",
        "gide",
    );
    flags(
        "",
        r##"safe-zone id="z" x=(px)0 y=(px)0 w=(px)1 h=(px)1 { extra }"##,
        "safe-zone",
        "extra",
    );
}

#[test]
fn table_row_unknown_child() {
    flags(
        "",
        r##"table id="t" { column; row { cel; cell } }"##,
        "row",
        "cel",
    );
}

#[test]
fn instance_override_unknown_child() {
    flags(
        "",
        r##"instance id="i" component="c" { override ref="a" { spn "x" } }"##,
        "override",
        "spn",
    );
}

#[test]
fn path_subpath_unknown_child() {
    flags(
        "",
        r##"path id="p" { subpath { anchr x=(px)0 y=(px)0 } }"##,
        "subpath",
        "anchr",
    );
}

#[test]
fn leaf_substructure_children_are_flagged() {
    flags(
        "",
        r##"text id="t" x=(px)0 y=(px)0 w=(px)10 h=(px)10 { span "a" { nested } }"##,
        "span",
        "nested",
    );
    flags(
        "",
        r##"chart id="c" kind="bar" { series 1.0 { nested } }"##,
        "series",
        "nested",
    );
}

#[test]
fn mask_token_extra_shape_is_error() {
    let top = r##"tokens format="zenith-token-v1" { token id="m" type="mask" { rect; ellipse } }"##;
    let report = validate(&parse_doc(top, ""));
    let found: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|d| d.code == "token.mask_extra_shape")
        .collect();
    assert_eq!(found.len(), 1, "{:?}", report.diagnostics);
    assert_eq!(found[0].severity, Severity::Error);
    assert!(
        found[0].message.contains("'ellipse'"),
        "{}",
        found[0].message
    );
    let single =
        r##"tokens format="zenith-token-v1" { token id="m" type="mask" { rounded radius=4 } }"##;
    assert!(!has_code(
        &validate(&parse_doc(single, "")),
        "token.mask_extra_shape"
    ));
    let info = zenith_core::diag_catalog::lookup("token.mask_extra_shape").expect("catalogued");
    assert_eq!(info.severity, Severity::Error);
}

#[test]
fn block_unknown_child_is_catalogued_error() {
    let info = zenith_core::diag_catalog::lookup("block.unknown_child").expect("catalogued");
    assert_eq!(info.severity, Severity::Error);
}
