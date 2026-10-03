//! Page lint: `text.edge_crowding`, `text.too_small`, and
//! `type.near_duplicate_size`.

mod common;
use common::*;
use zenith_core::{Diagnostic, FixHint, default_provider};
use zenith_scene::{DocumentPrep, PageCompiler};

/// A one-page 1000×1000 document; `page_attrs` and `body` fill the page.
fn doc(page_attrs: &str, body: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.lint" name="Lint"
  tokens format="zenith-token-v1" {{
    token id="color.ink" type="color" value="#111111"
    token id="color.paper" type="color" value="#ffffff"
    token id="size.type" type="dimension" value=(px)32
    token id="size.out" type="dimension" value=(px)20
    token id="size.label" type="dimension" value=(px)21
    token id="size.big" type="dimension" value=(px)48
    token id="size.tiny" type="dimension" value=(px)6
  }}
  styles {{}}
  components {{}}
  document id="doc.lint" title="Lint" {{
    page id="p" w=(px)1000 h=(px)1000 background=(token)"color.paper" {page_attrs} {{
{body}
    }}
  }}
}}
"##
    )
}

fn lint(src: &str) -> Vec<Diagnostic> {
    let document = parse(src);
    let fonts = default_provider();
    let prep = DocumentPrep::new(&document, None, None);
    PageCompiler::new(&prep, &fonts)
        .compile_page_local(0)
        .diagnostics
}

fn with_code<'a>(diags: &'a [Diagnostic], code: &str) -> Vec<&'a Diagnostic> {
    diags.iter().filter(|d| d.code == code).collect()
}

fn text_at(id: &str, x: u32, y: u32, size: &str, extra: &str) -> String {
    format!(
        r#"      text id="{id}" x=(px){x} y=(px){y} w=(px)600 font-size={size} fill=(token)"color.ink" {extra} {{ span "Headline" }}"#
    )
}

fn token(id: &str) -> String {
    format!(r#"(token)"{id}""#)
}

fn crowd(extra: &str) -> String {
    text_at("title", 400, 2, &token("size.type"), extra)
}

const ZERO: &str = "";

#[test]
fn edge_crowding_fires_with_a_y_fix() {
    let diags = lint(&doc(ZERO, &crowd("")));
    let hits = with_code(&diags, "text.edge_crowding");
    assert_eq!(hits.len(), 1, "{diags:#?}");
    let d = hits[0];
    assert_eq!(d.subject_id.as_deref(), Some("title"));
    assert!(
        d.message.contains("'title' ink is ") && d.message.contains("from top edge (floor 15px)"),
        "{}",
        d.message
    );
    match d.fix() {
        Some(FixHint::SetProperty { property, to }) => {
            assert_eq!(property, "y");
            let px: f64 = to.trim_start_matches("(px)").parse().expect("px value");
            assert!(px > 2.0, "{to}");
            assert!(d.message.contains(&format!("set y={to}")), "{}", d.message);
        }
        other => panic!("expected SetProperty fix, got {other:?}"),
    }
}

#[test]
fn edge_crowding_measures_from_the_trim_under_a_bleed() {
    let plain = lint(&doc(ZERO, &crowd("")));
    let bled = lint(&doc("bleed=(px)35", &crowd("")));
    let plain = with_code(&plain, "text.edge_crowding");
    let bled = with_code(&bled, "text.edge_crowding");
    assert_eq!(bled.len(), 1, "{bled:#?}");
    assert_eq!(bled[0].message, plain[0].message);
}

#[test]
fn edge_crowding_left_edge_fixes_x() {
    let body = text_at("title", 2, 400, &token("size.type"), "");
    let diags = lint(&doc(ZERO, &body));
    let hits = with_code(&diags, "text.edge_crowding");
    assert_eq!(hits.len(), 1, "{diags:#?}");
    assert!(hits[0].message.contains("left edge"), "{}", hits[0].message);
    assert!(matches!(
        hits[0].fix(),
        Some(FixHint::SetProperty { property, .. }) if property == "x"
    ));
}

#[test]
fn edge_crowding_has_no_fix_when_anchored() {
    let body = format!(
        r#"      text id="title" x=(px)0 y=(px)2 w=(px)600 h=(px)60 anchor="top-center" font-size={} fill=(token)"color.ink" {{ span "Headline" }}"#,
        token("size.type")
    );
    let diags = lint(&doc(ZERO, &body));
    let hits = with_code(&diags, "text.edge_crowding");
    assert_eq!(hits.len(), 1, "{diags:#?}");
    assert!(hits[0].fix().is_none());
    assert!(
        hits[0].message.contains("move 'title'"),
        "{}",
        hits[0].message
    );
}

#[test]
fn edge_crowding_quiet_cases() {
    let margins =
        r#"margin-inner=(px)60 margin-outer=(px)60 margin-top=(px)80 margin-bottom=(px)80"#;
    let safe = r#"safe-zone id="z" type="required" x=(px)100 y=(px)100 w=(px)800 h=(px)800"#;
    let cases = [
        (margins.to_owned(), crowd("")),
        (String::new(), crowd(r#"role="guide""#)),
        (String::new(), crowd("visible=#false")),
        (String::new(), crowd(r#"role="decoration""#)),
        (
            String::new(),
            text_at("title", 400, 400, &token("size.type"), ""),
        ),
        // Ink that leaves the trim is bleed.
        (
            String::new(),
            text_at("title", 400, 0, &token("size.big"), r#"rotate=0"#)
                .replace("y=(px)0", "y=(px)-40"),
        ),
    ];
    for (attrs, body) in cases {
        let diags = lint(&doc(&attrs, &body));
        assert!(
            with_code(&diags, "text.edge_crowding").is_empty(),
            "{attrs} {body}: {diags:#?}"
        );
    }
    // A safe zone is a child of the page.
    let body = format!("      {safe}\n{}", crowd(""));
    let diags = lint(&doc(ZERO, &body));
    assert!(
        with_code(&diags, "text.edge_crowding").is_empty(),
        "{diags:#?}"
    );
}

#[test]
fn too_small_with_a_token_has_message_only() {
    let body = text_at("legal", 400, 400, &token("size.tiny"), "");
    let diags = lint(&doc(ZERO, &body));
    let hits = with_code(&diags, "text.too_small");
    assert_eq!(hits.len(), 1, "{diags:#?}");
    let d = hits[0];
    assert!(
        d.message.contains("'legal' 6px < 9px floor for 1000×1000"),
        "{}",
        d.message
    );
    assert!(d.message.contains("size.tiny"), "{}", d.message);
    assert!(d.fix().is_none());
}

#[test]
fn too_small_with_a_literal_sets_the_floor() {
    let body = text_at("legal", 400, 400, "(px)6", "");
    let diags = lint(&doc(ZERO, &body));
    let hits = with_code(&diags, "text.too_small");
    assert_eq!(hits.len(), 1, "{diags:#?}");
    assert!(
        hits[0].message.contains("set font-size=(px)9"),
        "{}",
        hits[0].message
    );
    match hits[0].fix() {
        Some(FixHint::SetProperty { property, to }) => {
            assert_eq!(property, "font-size");
            assert_eq!(to, "(px)9");
        }
        other => panic!("expected SetProperty fix, got {other:?}"),
    }
}

#[test]
fn too_small_quiet_cases() {
    for extra in [r#"role="guide""#, "visible=#false", r#"role="decoration""#] {
        let body = text_at("legal", 400, 400, &token("size.tiny"), extra);
        let diags = lint(&doc(ZERO, &body));
        assert!(
            with_code(&diags, "text.too_small").is_empty(),
            "{extra}: {diags:#?}"
        );
    }
    let body = text_at("ok", 400, 400, &token("size.out"), "");
    let diags = lint(&doc(ZERO, &body));
    assert!(with_code(&diags, "text.too_small").is_empty());
}

fn artboard(id: &str, a: &str, b: &str) -> String {
    format!(
        "      frame id=\"{id}\" x=(px)0 y=(px)0 w=(px)500 h=(px)500 {{\n  {}\n  {}\n      }}",
        text_at(&format!("{id}.a"), 100, 100, &token(a), ""),
        text_at(&format!("{id}.b"), 100, 200, &token(b), ""),
    )
}

#[test]
fn near_duplicate_size_fires_inside_one_group() {
    let body = artboard("board", "size.out", "size.label");
    let diags = lint(&doc(ZERO, &body));
    let hits = with_code(&diags, "type.near_duplicate_size");
    assert_eq!(hits.len(), 1, "{diags:#?}");
    assert_eq!(
        hits[0].message,
        "sizes 20 (size.out) and 21 (size.label) differ by 5% — merge into one token"
    );
    assert!(hits[0].fix().is_none());
}

#[test]
fn near_duplicate_size_quiet_cases() {
    // Separate artboards do not compare against each other.
    let body = format!(
        "{}\n{}",
        artboard("one", "size.out", "size.out"),
        artboard("two", "size.label", "size.label")
    );
    let diags = lint(&doc(ZERO, &body));
    assert!(
        with_code(&diags, "type.near_duplicate_size").is_empty(),
        "{diags:#?}"
    );
    // Clearly distinct steps.
    let diags = lint(&doc(ZERO, &artboard("board", "size.out", "size.big")));
    assert!(
        with_code(&diags, "type.near_duplicate_size").is_empty(),
        "{diags:#?}"
    );
    // Hidden, guide, and table internals never count.
    for extra in [r#"role="guide""#, "visible=#false"] {
        let body = format!(
            "      frame id=\"board\" x=(px)0 y=(px)0 w=(px)500 h=(px)500 {{\n  {}\n  {}\n      }}",
            text_at("a", 100, 100, &token("size.out"), ""),
            text_at("b", 100, 200, &token("size.label"), extra),
        );
        let diags = lint(&doc(ZERO, &body));
        assert!(
            with_code(&diags, "type.near_duplicate_size").is_empty(),
            "{extra}: {diags:#?}"
        );
    }
    let body = format!(
        "      frame id=\"board\" x=(px)0 y=(px)0 w=(px)500 h=(px)500 {{\n  {}\n  table id=\"t\" x=(px)0 y=(px)300 w=(px)200 h=(px)100 {{\n column width=(px)200\n row {{ cell {{ {} }} }}\n }}\n      }}",
        text_at("a", 100, 100, &token("size.out"), ""),
        text_at("c", 0, 0, &token("size.label"), ""),
    );
    let diags = lint(&doc(ZERO, &body));
    assert!(
        with_code(&diags, "type.near_duplicate_size").is_empty(),
        "{diags:#?}"
    );
}

#[test]
fn legibility_is_deterministic() {
    let body = format!(
        "{}\n{}",
        crowd(""),
        artboard("board", "size.out", "size.label")
    );
    let src = doc(ZERO, &body);
    let a = lint(&src);
    let b = lint(&src);
    assert_eq!(a, b);
    assert!(!with_code(&a, "text.edge_crowding").is_empty());
}
