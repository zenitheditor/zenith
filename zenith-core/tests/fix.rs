//! Integration tests for `zenith_core::fix::fix_source`.

use zenith_core::KdlSource;
use zenith_core::fix::fix_source;

fn doc(tokens: &str, children: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.f" name="Fix"
  tokens format="zenith-token-v1" {{
{tokens}
  }}
  styles {{
  }}
  document id="doc.f" title="Fix" {{
    page id="page.f" w=(px)800 h=(px)600 {{
{children}
    }}
  }}
}}
"##
    )
}

const GEOM: &str = "x=(px)0 y=(px)0 w=(px)100 h=(px)40";

fn error_codes(out: &zenith_core::fix::FixOutcome) -> Vec<String> {
    out.remaining
        .iter()
        .filter(|d| d.is_error())
        .map(|d| d.code.clone())
        .collect()
}

#[test]
fn raw_color_references_token_with_same_value() {
    let src = doc(
        r##"    token id="color.white" type="color" value="#ffffff""##,
        &format!(r##"      rect id="r" {GEOM} fill="#FFF""##),
    );
    let out = fix_source(&src).expect("parses");
    assert!(out.minted.is_empty(), "{:?}", out.minted);
    assert!(
        out.source_after.contains(r#"fill=(token)"color.white""#),
        "{}",
        out.source_after
    );
    assert!(error_codes(&out).is_empty(), "{:?}", out.remaining);
}

#[test]
fn span_level_literal_is_fixed_on_the_span() {
    let src = doc(
        r##"    token id="color.ink" type="color" value="#112233""##,
        &format!(r##"      text id="t" {GEOM} {{ span "hi" fill="#112233" }}"##),
    );
    let out = fix_source(&src).expect("parses");
    assert!(
        out.source_after.contains(r#"fill=(token)"color.ink""#),
        "{}",
        out.source_after
    );
    assert_eq!(out.applied.len(), 1);
    assert_eq!(out.applied[0].subject_id, "t");
}

#[test]
fn invalid_enum_value_takes_unique_did_you_mean() {
    let src = doc(
        "",
        &format!(r#"      rect id="r" {GEOM} blend-mode="multipy""#),
    );
    let out = fix_source(&src).expect("parses");
    assert!(
        out.source_after.contains(r#"blend-mode="multiply""#),
        "{}",
        out.source_after
    );
    assert_eq!(out.applied[0].from, "\"multipy\"");
    assert_eq!(out.applied[0].to, "\"multiply\"");
}

#[test]
fn ambiguous_token_reference_stays_remaining() {
    let src = doc(
        "    token id=\"color.base.100\" type=\"color\" value=\"#ffffff\"\n    \
         token id=\"color.base.200\" type=\"color\" value=\"#eeeeee\"",
        &format!(r#"      rect id="r" {GEOM} fill=(token)"color.base.900""#),
    );
    let out = fix_source(&src).expect("parses");
    assert!(out.applied.is_empty());
    assert_eq!(out.source_after, src, "no fix: source untouched");
    assert!(error_codes(&out).contains(&"token.unknown_reference".to_owned()));
}

#[test]
fn unknown_reference_takes_declared_prefix_of_same_type() {
    let src = doc(
        "    token id=\"color.primary\" type=\"color\" value=\"#605dff\"\n    \
         token id=\"color.primary.content\" type=\"color\" value=\"#ffffff\"",
        &format!(r#"      rect id="r" {GEOM} fill=(token)"color.primary.500""#),
    );
    let out = fix_source(&src).expect("parses");
    assert!(
        out.source_after.contains(r#"fill=(token)"color.primary""#),
        "{}",
        out.source_after
    );
}

#[test]
fn font_family_matches_without_case_else_mints_slug() {
    let src = doc(
        r#"    token id="font.body" type="fontFamily" value="Noto Sans""#,
        &format!(
            "      text id=\"a\" {GEOM} font-family=\"noto sans\" {{ span \"a\" }}\n      \
             text id=\"b\" {GEOM} font-family=\"Playfair Display\" {{ span \"b\" }}"
        ),
    );
    let out = fix_source(&src).expect("parses");
    assert!(
        out.source_after
            .contains(r#"font-family=(token)"font.body""#),
        "{}",
        out.source_after
    );
    assert!(
        out.source_after.contains(
            r#"token id="font.playfair-display" type="fontFamily" value="Playfair Display""#
        ),
        "{}",
        out.source_after
    );
}

#[test]
fn one_minted_token_serves_every_node_with_the_value() {
    let src = doc(
        "",
        &format!(
            "      rect id=\"a\" {GEOM} fill=\"#abcdef\"\n      rect id=\"b\" {GEOM} fill=\"#ABCDEF\""
        ),
    );
    let out = fix_source(&src).expect("parses");
    assert_eq!(out.minted.len(), 1, "{:?}", out.minted);
    assert_eq!(out.minted[0].id, "color.custom.abcdef");
    assert_eq!(
        out.source_after
            .matches(r#"fill=(token)"color.custom.abcdef""#)
            .count(),
        2,
        "{}",
        out.source_after
    );
    assert!(error_codes(&out).is_empty(), "{:?}", out.remaining);
}

#[test]
fn rename_skipped_when_target_already_set() {
    let src = doc(
        r##"    token id="color.ink" type="color" value="#112233""##,
        &format!(r#"      rect id="r" {GEOM} fill=(token)"color.ink" fil=(token)"color.ink""#),
    );
    let out = fix_source(&src).expect("parses");
    assert!(out.applied.is_empty(), "{:?}", out.applied);
    assert!(error_codes(&out).contains(&"node.unknown_property".to_owned()));
}

#[test]
fn point_dimension_mints_unit_suffixed_id() {
    let src = doc(
        "",
        &format!(r#"      text id="t" {GEOM} font-size=(pt)12 {{ span "a" }}"#),
    );
    let out = fix_source(&src).expect("parses");
    assert!(
        out.source_after
            .contains(r#"token id="size.12pt" type="dimension" value=(pt)12"#),
        "{}",
        out.source_after
    );
}

#[test]
fn rename_then_literal_resolves_over_two_passes() {
    let src = doc(
        r#"    token id="weight.bold" type="fontWeight" value=700"#,
        &format!(r#"      text id="t" {GEOM} font-wieght=700 {{ span "a" }}"#),
    );
    let out = fix_source(&src).expect("parses");
    let codes: Vec<&str> = out.applied.iter().map(|f| f.code.as_str()).collect();
    assert_eq!(
        codes,
        vec!["node.unknown_property", "token.raw_visual_literal"]
    );
    assert!(
        out.source_after
            .contains(r#"font-weight=(token)"weight.bold""#),
        "{}",
        out.source_after
    );
    assert!(error_codes(&out).is_empty(), "{:?}", out.remaining);
}

#[test]
fn output_is_canonical_idempotent_and_deterministic() {
    let src = doc(
        "",
        &format!(r##"      rect id="r" {GEOM} fill="#123456" radius=(px)6"##),
    );
    let a = fix_source(&src).expect("parses");
    let b = fix_source(&src).expect("parses");
    assert_eq!(a, b);
    let again = fix_source(&a.source_after).expect("parses");
    assert!(again.applied.is_empty());
    assert_eq!(again.source_after, a.source_after);
}

#[test]
fn parse_error_is_returned() {
    assert!(fix_source("zenith {{{").is_err());
}

#[test]
fn minted_font_weight_is_numeric_and_reference_validates() {
    let src = doc(
        "",
        &format!(r#"      text id="t" {GEOM} font-weight=700 {{ span "a" }}"#),
    );
    let out = fix_source(&src).expect("parses");
    assert!(
        out.source_after
            .contains(r#"token id="weight.700" type="fontWeight" value=700"#),
        "{}",
        out.source_after
    );
    let doc = zenith_core::KdlAdapter
        .parse(out.source_after.as_bytes())
        .expect("fixed source parses");
    let resolved = zenith_core::resolve_tokens(&doc.tokens);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    assert_eq!(
        resolved.resolved.get("weight.700").map(|t| &t.value),
        Some(&zenith_core::ResolvedValue::FontWeight(700))
    );
    assert!(
        out.remaining
            .iter()
            .all(|d| d.subject_id.as_deref() != Some("t")),
        "{:?}",
        out.remaining
    );
}

#[test]
fn validation_carries_structured_fix_hints() {
    use zenith_core::FixHint;
    let src = doc(
        "    token id=\"color.primary\" type=\"color\" value=\"#605dff\"",
        &format!(
            "      rect id=\"a\" {GEOM} fill=(token)\"color.primary.500\" blend-mode=\"multipy\"\n      \
             rect id=\"b\" {GEOM} fil=(token)\"color.primary\""
        ),
    );
    let doc = zenith_core::KdlAdapter
        .parse(src.as_bytes())
        .expect("parses");
    let report = zenith_core::validate(&doc);
    let hint = |code: &str| {
        report
            .diagnostics
            .iter()
            .find(|d| d.code == code)
            .and_then(|d| d.fix().cloned())
    };
    assert_eq!(
        hint("token.unknown_reference"),
        Some(FixHint::ReplaceTokenRef {
            property: "fill".into(),
            from: "color.primary.500".into(),
            to: "color.primary".into(),
        })
    );
    assert_eq!(
        hint("node.invalid_value"),
        Some(FixHint::ReplaceValue {
            property: "blend-mode".into(),
            from: "multipy".into(),
            to: "multiply".into(),
        })
    );
    assert_eq!(
        hint("node.unknown_property"),
        Some(FixHint::RenameProperty {
            from: "fil".into(),
            to: "fill".into(),
        })
    );
}

// ── defaults rows and style property children ────────────────────────────────

/// A document with `styles_body` in `styles`, `top` after `styles`, and
/// `page_top` at the page body start.
fn row_doc(styles_body: &str, top: &str, page_top: &str) -> String {
    format!(
        r##"zenith version=1 {{
  tokens format="zenith-token-v1" {{
    token id="color.ink" type="color" value="#111111"
  }}
  styles {{
    style id="body" {{
      fill (token)"color.ink"
    }}
    style id="box" {{
      fill (token)"color.ink"
    }}
{styles_body}
  }}
{top}
  document id="doc.f" {{
    page id="page.f" w=(px)800 h=(px)600 {{
{page_top}
      rect id="r" {GEOM} fill=(token)"color.ink" style="body"
    }}
  }}
}}
"##
    )
}

fn assert_idempotent(out: &zenith_core::fix::FixOutcome) {
    let again = fix_source(&out.source_after).expect("reparse");
    assert!(
        !again.changed(),
        "second fix changed:\n{}",
        again.source_after
    );
    assert!(again.applied.is_empty());
}

#[test]
fn defaults_unknown_kind_is_renamed() {
    let src = row_doc(
        "",
        "  defaults {\n    txet style=\"body\"\n    rect style=\"box\"\n  }",
        "",
    );
    let out = fix_source(&src).expect("fix");
    assert!(out.changed());
    assert!(
        out.source_after
            .contains("  defaults {\n    rect style=\"box\"\n    text style=\"body\"\n  }\n"),
        "{}",
        out.source_after
    );
    let fix = out
        .applied
        .iter()
        .find(|f| f.code == "defaults.unknown_kind")
        .expect("applied");
    assert_eq!((fix.from.as_str(), fix.to.as_str()), ("txet", "text"));
    assert!(
        error_codes(&out)
            .iter()
            .all(|c| !c.starts_with("defaults."))
    );
    assert_idempotent(&out);
}

#[test]
fn defaults_unknown_kind_rename_skipped_when_target_taken() {
    let src = row_doc(
        "",
        "  defaults {\n    text style=\"body\"\n    txet style=\"box\"\n  }",
        "",
    );
    let out = fix_source(&src).expect("fix");
    assert!(
        out.applied
            .iter()
            .all(|f| f.code != "defaults.unknown_kind"),
        "{:?}",
        out.applied
    );
    assert!(error_codes(&out).contains(&"defaults.unknown_kind".to_owned()));
}

#[test]
fn defaults_unknown_style_is_replaced_at_page_scope() {
    let src = row_doc(
        "",
        "",
        "      defaults {\n        shape style=\"bxo\" text-style=\"bodi\"\n      }",
    );
    let out = fix_source(&src).expect("fix");
    assert!(
        out.source_after
            .contains("defaults {\n        shape style=\"box\" text-style=\"body\"\n      }"),
        "{}",
        out.source_after
    );
    let codes: Vec<&str> = out.applied.iter().map(|f| f.code.as_str()).collect();
    assert_eq!(codes, ["defaults.unknown_style", "defaults.unknown_style"]);
    assert!(
        error_codes(&out)
            .iter()
            .all(|c| !c.starts_with("defaults."))
    );
    assert_idempotent(&out);
}

#[test]
fn defaults_unknown_property_is_renamed() {
    let src = row_doc(
        "",
        "  defaults {\n    shape style=\"box\" text-stlye=\"body\"\n  }",
        "",
    );
    let out = fix_source(&src).expect("fix");
    assert!(
        out.source_after
            .contains("    shape style=\"box\" text-style=\"body\"\n"),
        "{}",
        out.source_after
    );
    assert!(
        out.applied
            .iter()
            .any(|f| f.code == "defaults.unknown_property")
    );
    assert_idempotent(&out);
}

#[test]
fn style_invalid_value_child_is_replaced() {
    let src = row_doc(
        "    style id=\"card\" {\n      align \"centre\"\n      v_align \"midle\"\n    }",
        "",
        "",
    );
    let out = fix_source(&src).expect("fix");
    assert!(
        out.source_after.contains(
            "style id=\"card\" {\n      align \"center\"\n      v-align \"middle\"\n    }"
        ),
        "{}",
        out.source_after
    );
    let codes: Vec<&str> = out.applied.iter().map(|f| f.code.as_str()).collect();
    assert_eq!(codes, ["style.invalid_value", "style.invalid_value"]);
    assert!(!error_codes(&out).contains(&"style.invalid_value".to_owned()));
    assert_idempotent(&out);
}
