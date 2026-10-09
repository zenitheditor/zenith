//! Page lint: `text.ink_overlap`, `text.occluded`, `label.overflow`, and the
//! contrast of expanded instance content.

mod common;
use common::parse;
use zenith_core::{Diagnostic, FixHint, default_provider};
use zenith_scene::{DocumentPrep, PageCompiler};

/// A one-page document with `body` on the page and `components` declared.
fn doc(components: &str, body: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.lint" name="Lint"
  tokens format="zenith-token-v1" {{
    token id="color.ink" type="color" value="#111111"
    token id="color.paper" type="color" value="#ffffff"
    token id="color.pale" type="color" value="#f4f4f4"
    token id="color.box" type="color" value="#2255aa"
    token id="color.glass" type="color" value="#2255aa80"
    token id="size.type" type="dimension" value=(px)32
  }}
  styles {{}}
  components {{
{components}
  }}
  document id="doc.lint" title="Lint" {{
    page id="p" w=(px)800 h=(px)600 background=(token)"color.paper" {{
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

fn text(id: &str, y: u32, extra: &str) -> String {
    format!(
        r#"      text id="{id}" x=(px)40 y=(px){y} w=(px)600 font-size=(token)"size.type" fill=(token)"color.ink" {extra} {{ span "Overlapping headline" }}"#
    )
}

#[test]
fn ink_overlap_fires_with_a_y_fix_on_the_lower_text() {
    let body = format!("{}\n{}", text("a", 100, ""), text("b", 110, ""));
    let diags = lint(&doc("", &body));
    let hits = with_code(&diags, "text.ink_overlap");
    assert_eq!(hits.len(), 1, "{diags:#?}");
    let d = hits[0];
    assert_eq!(d.subject_id.as_deref(), Some("b"));
    assert!(d.message.contains("'b' ink overlaps 'a'"), "{}", d.message);
    match d.fix() {
        Some(FixHint::SetProperty { property, to }) => {
            assert_eq!(property, "y");
            assert!(
                d.message.contains(&format!("move 'b' to y={to}")),
                "{}",
                d.message
            );
            let px: f64 = to.trim_start_matches("(px)").parse().expect("px value");
            assert!(px > 110.0, "{to}");
        }
        other => panic!("expected SetProperty fix, got {other:?}"),
    }
}

#[test]
fn ink_overlap_stays_quiet_on_exemptions() {
    for extra in [
        r#"role="decoration""#,
        r#"role="background""#,
        r#"role="guide""#,
        "visible=#false",
        "opacity=0.3",
    ] {
        let body = format!("{}\n{}", text("a", 100, ""), text("b", 110, extra));
        let diags = lint(&doc("", &body));
        assert!(
            with_code(&diags, "text.ink_overlap").is_empty(),
            "{extra}: {diags:#?}"
        );
    }
    // One node's own lines never collide with each other.
    let tight = r#"      text id="solo" x=(px)40 y=(px)100 w=(px)200 style="st.tight" fill=(token)"color.ink" { span "many words wrap onto several tight lines here" }"#;
    let diags = lint(&shape_doc(tight));
    assert!(
        with_code(&diags, "text.ink_overlap").is_empty(),
        "{diags:#?}"
    );
}

#[test]
fn ink_overlap_in_expanded_content_has_no_fix() {
    let components = r#"    component id="card" {
      text id="t" x=(px)0 y=(px)0 w=(px)600 font-size=(token)"size.type" fill=(token)"color.ink" { span "Overlapping headline" }
    }"#;
    let body = format!(
        "{}\n      instance id=\"inst\" component=\"card\" x=(px)40 y=(px)110",
        text("a", 100, "")
    );
    let diags = lint(&doc(components, &body));
    let hits = with_code(&diags, "text.ink_overlap");
    assert_eq!(hits.len(), 1, "{diags:#?}");
    assert_eq!(hits[0].subject_id.as_deref(), Some("inst/t"));
    assert!(hits[0].fix().is_none());
    assert!(
        hits[0].message.contains("move 'inst/t' down"),
        "{}",
        hits[0].message
    );
}

fn cover(extra: &str) -> String {
    format!(
        r#"      rect id="cover" x=(px)20 y=(px)80 w=(px)700 h=(px)80 fill=(token)"color.box" {extra}"#
    )
}

#[test]
fn occluded_fires_when_opaque_paint_lands_later() {
    let body = format!("{}\n{}", text("a", 100, ""), cover(""));
    let diags = lint(&doc("", &body));
    let hits = with_code(&diags, "text.occluded");
    assert_eq!(hits.len(), 1, "{diags:#?}");
    assert_eq!(hits[0].subject_id.as_deref(), Some("a"));
    assert!(
        hits[0].message.contains("rect 'cover'"),
        "{}",
        hits[0].message
    );
    assert!(hits[0].fix().is_none());
}

#[test]
fn occluded_stays_quiet_on_exemptions() {
    let cases = [
        format!("{}\n{}", cover(""), text("a", 100, "")),
        format!("{}\n{}", text("a", 100, ""), cover(r#"role="decoration""#)),
        format!("{}\n{}", text("a", 100, ""), cover("opacity=0.5")),
        format!("{}\n{}", text("a", 100, r#"role="background""#), cover("")),
        format!(
            "{}\n{}",
            text("a", 100, ""),
            r#"      rect id="cover" x=(px)20 y=(px)80 w=(px)700 h=(px)80 fill=(token)"color.glass""#
        ),
    ];
    for body in cases {
        let diags = lint(&doc("", &body));
        assert!(
            with_code(&diags, "text.occluded").is_empty(),
            "{body}\n{diags:#?}"
        );
    }
}

const CHART: &str = r#"      chart id="c" kind="bar" x=(px)40 y=(px)100 w=(px)400 h=(px)300 {
        categories "North" "South" "West"
        series label="2025" 10.0 20.0 30.0
      }"#;

const FOOTER: &str =
    r#"      frame id="footer" x=(px)0 y=(px)340 w=(px)800 h=(px)200 fill=(token)"color.box""#;

#[test]
fn occluded_fires_on_chart_text_under_a_later_frame() {
    let diags = lint(&doc("", &format!("{CHART}\n{FOOTER}")));
    let hits = with_code(&diags, "text.occluded");
    assert!(!hits.is_empty(), "{diags:#?}");
    for d in &hits {
        assert_eq!(d.subject_id.as_deref(), Some("c"));
        assert!(d.message.contains("chart 'c'"), "{}", d.message);
        assert!(d.message.contains("frame 'footer'"), "{}", d.message);
    }
    assert!(
        hits.iter().any(|d| d.message.contains("category text")),
        "{hits:#?}"
    );
}

#[test]
fn chart_over_an_earlier_frame_is_not_occluded() {
    let diags = lint(&doc("", &format!("{FOOTER}\n{CHART}")));
    assert!(with_code(&diags, "text.occluded").is_empty(), "{diags:#?}");
    // One chart's strings never pair with each other.
    assert!(
        with_code(&diags, "text.ink_overlap").is_empty(),
        "{diags:#?}"
    );
}

fn shape(kind: &str, label: &str) -> String {
    format!(
        r#"      shape id="s" kind="{kind}" x=(px)100 y=(px)100 w=(px)200 h=(px)100 fill=(token)"color.pale" text-style="st.label" {{ span "{label}" }}"#
    )
}

fn shape_doc(body: &str) -> String {
    doc("", body).replace(
        "styles {}",
        r#"styles {
    style id="st.label" { font-size (token)"size.type"; fill (token)"color.ink"; }
    style id="st.tight" { font-size (token)"size.type"; line-height 0.6; }
  }"#,
    )
}

#[test]
fn label_overflow_fires_for_an_ellipse_only() {
    let diags = lint(&shape_doc(&shape("ellipse", "Wider label text here")));
    let hits = with_code(&diags, "label.overflow");
    assert_eq!(hits.len(), 1, "{diags:#?}");
    assert_eq!(hits[0].subject_id.as_deref(), Some("s"));
    assert!(
        hits[0].message.contains("ellipse outline"),
        "{}",
        hits[0].message
    );
    assert!(
        hits[0].message.contains("set w=(px)"),
        "{}",
        hits[0].message
    );
    assert!(with_code(&diags, "text.overflow").is_empty(), "{diags:#?}");

    // The same label in a process box is `text.overflow`'s domain.
    let diags = lint(&shape_doc(&shape("process", "Wider label text here")));
    assert!(with_code(&diags, "label.overflow").is_empty(), "{diags:#?}");
    // A short label fits the ellipse.
    let diags = lint(&shape_doc(&shape("ellipse", "Hi")));
    assert!(with_code(&diags, "label.overflow").is_empty(), "{diags:#?}");
}

#[test]
fn expanded_instance_text_contrast_reports_with_the_component_span() {
    let components = r#"    component id="card" {
      text id="t" x=(px)0 y=(px)0 w=(px)300 font-size=(token)"size.type" fill=(token)"color.pale" { span "Faint" }
    }"#;
    let body = r#"      instance id="inst" component="card" x=(px)40 y=(px)300"#;
    let src = doc(components, body);
    let diags = lint(&src);
    let hit = diags
        .iter()
        .find(|d| d.code.starts_with("contrast.") && d.subject_id.as_deref() == Some("inst/t"))
        .unwrap_or_else(|| panic!("{diags:#?}"));
    let span = hit.span.expect("span");
    let authored = &src[span.start..span.end];
    assert!(authored.starts_with(r#"text id="t""#), "{authored}");
}

#[test]
fn lint_is_deterministic() {
    let body = format!(
        "{}\n{}\n{}\n{}",
        text("a", 100, ""),
        text("b", 110, ""),
        cover(""),
        shape("decision", "Wide label!")
    );
    let src = shape_doc(&body);
    assert_eq!(lint(&src), lint(&src));
}
