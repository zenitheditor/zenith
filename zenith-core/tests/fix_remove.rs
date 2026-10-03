//! `FixHint::RemoveProperty`: `zenith_core::fix::fix_source` removes the
//! attributes that `layout.position_ignored`, `layout.inert_attribute`, and
//! `defaults.text_style_unsupported` flag.

use zenith_core::fix::{FixOutcome, fix_source};

fn doc(styles: &str, defaults: &str, children: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.r" name="Remove"
  tokens format="zenith-token-v1" {{
    token id="color.k" type="color" value="#000000"
  }}
  styles {{
{styles}
  }}
{defaults}
  document id="doc.r" title="Remove" {{
    page id="page.r" w=(px)800 h=(px)600 {{
{children}
    }}
  }}
}}
"##
    )
}

/// The source line holding `id="<id>"`.
fn line_of<'s>(src: &'s str, id: &str) -> &'s str {
    let needle = format!("id=\"{id}\"");
    src.lines()
        .find(|l| l.contains(&needle))
        .unwrap_or_else(|| panic!("no line for {id} in:\n{src}"))
}

fn has_attr(line: &str, attr: &str) -> bool {
    line.split_whitespace()
        .any(|tok| tok.starts_with(&format!("{attr}=")))
}

fn remaining(out: &FixOutcome, code: &str) -> usize {
    out.remaining.iter().filter(|d| d.code == code).count()
}

fn applied(out: &FixOutcome) -> Vec<(String, String, String, String)> {
    out.applied
        .iter()
        .map(|f| {
            (
                f.code.clone(),
                f.subject_id.clone(),
                f.property.clone(),
                f.to.clone(),
            )
        })
        .collect()
}

#[test]
fn ignored_xy_and_anchor_on_in_flow_children_are_removed() {
    let src = doc(
        "",
        "",
        r#"      frame id="row" x=(px)10 y=(px)10 w=(px)400 h=(px)100 layout="row" gap=(px)8 {
        rect id="chip" x=(px)40 y=(px)600 w=(px)50 h=(px)20 fill=(token)"color.k"
        rect id="pin" anchor="center" w=(px)50 h=(px)20 fill=(token)"color.k"
      }"#,
    );
    let out = fix_source(&src).expect("parses");
    let chip = line_of(&out.source_after, "chip");
    assert!(!has_attr(chip, "x") && !has_attr(chip, "y"), "{chip}");
    assert!(has_attr(chip, "w") && has_attr(chip, "h"), "{chip}");
    let pin = line_of(&out.source_after, "pin");
    assert!(!has_attr(pin, "anchor"), "{pin}");
    assert_eq!(remaining(&out, "layout.position_ignored"), 0);
    let removed = |subject: &str, property: &str| {
        (
            "layout.position_ignored".to_owned(),
            subject.to_owned(),
            property.to_owned(),
            "(removed)".to_owned(),
        )
    };
    assert_eq!(
        applied(&out),
        [
            removed("chip", "x"),
            removed("chip", "y"),
            removed("pin", "anchor")
        ]
    );
}

#[test]
fn inert_layout_attributes_are_removed() {
    let src = doc(
        "",
        "",
        r#"      frame id="plain" x=(px)0 y=(px)0 w=(px)200 h=(px)200 gap=(px)4 justify="start" {
        rect id="r" x=(px)0 y=(px)0 w=(px)10 h=(px)10 min-w=(px)2 position="absolute" fill=(token)"color.k"
      }
      frame id="g" x=(px)300 y=(px)0 w=(px)200 h=(px)200 layout="grid" columns=2 gap=(px)4 align="start" {
        rect id="t" fill=(token)"color.k"
      }"#,
    );
    let out = fix_source(&src).expect("parses");
    let plain = line_of(&out.source_after, "plain");
    assert!(
        !has_attr(plain, "gap") && !has_attr(plain, "justify"),
        "{plain}"
    );
    let r = line_of(&out.source_after, "r");
    assert!(!has_attr(r, "min-w") && !has_attr(r, "position"), "{r}");
    let g = line_of(&out.source_after, "g");
    assert!(!has_attr(g, "align"), "{g}");
    assert!(has_attr(g, "gap"), "a grid gap is live: {g}");
    assert_eq!(remaining(&out, "layout.inert_attribute"), 0);
}

#[test]
fn unsupported_defaults_text_style_is_removed() {
    let src = doc(
        r#"    style id="s.body" {
      fill (token)"color.k"
    }"#,
        r#"  defaults {
    text style="s.body" text-style="s.body"
  }"#,
        r#"      rect id="r" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.k""#,
    );
    let out = fix_source(&src).expect("parses");
    assert!(
        out.source_after.contains("text style=\"s.body\"")
            && !out.source_after.contains("text-style"),
        "{}",
        out.source_after
    );
    assert_eq!(remaining(&out, "defaults.text_style_unsupported"), 0);
    assert!(
        out.applied
            .iter()
            .any(|f| f.code == "defaults.text_style_unsupported" && f.property == "text-style"),
        "{:?}",
        out.applied
    );
}

#[test]
fn removal_is_idempotent() {
    let src = doc(
        "",
        "",
        r#"      frame id="row" x=(px)10 y=(px)10 w=(px)400 h=(px)100 layout="row" {
        rect id="chip" x=(px)40 y=(px)6 w=(px)50 h=(px)20 fill=(token)"color.k"
      }"#,
    );
    let once = fix_source(&src).expect("parses");
    let twice = fix_source(&once.source_after).expect("parses");
    assert!(twice.applied.is_empty(), "{:?}", twice.applied);
    assert_eq!(twice.source_after, once.source_after);
}
