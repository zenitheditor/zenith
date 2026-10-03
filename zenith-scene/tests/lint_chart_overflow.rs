//! Page lint: `chart.overflow`, chart text ink outside the chart box.

mod common;
use common::*;
use zenith_core::{Diagnostic, FixHint};
use zenith_scene::{DocumentPrep, PageCompiler};

/// A one-page 800 x 600 document with `body` on the page.
fn doc(body: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.co" name="ChartOverflow"
  tokens format="zenith-token-v1" {{
    token id="color.paper" type="color" value="#ffffff"
  }}
  styles {{}}
  document id="doc.co" title="ChartOverflow" {{
    page id="p" w=(px)800 h=(px)600 background=(token)"color.paper" {{
{body}
    }}
  }}
}}
"##
    )
}

fn hits(src: &str) -> Vec<Diagnostic> {
    let document = parse(src);
    let fonts = default_provider();
    let prep = DocumentPrep::new(&document, None, None);
    PageCompiler::new(&prep, &fonts)
        .compile_page_local(0)
        .diagnostics
        .into_iter()
        .filter(|d| d.code == "chart.overflow")
        .collect()
}

/// A titled chart of `kind` at (40, 40), `w` x `h`; `attrs` adds attributes.
fn chart(kind: &str, (w, h): (u32, u32), attrs: &str) -> String {
    format!(
        r#"      chart id="c" kind="{kind}" x=(px)40 y=(px)40 w=(px){w} h=(px){h} title="Sales" {attrs} {{
        categories "North" "South" "West"
        series label="2025" 10.0 20.0 30.0
      }}"#
    )
}

#[test]
fn short_titled_chart_fires_with_an_h_fix() {
    let found = hits(&doc(&chart("bar", (400, 50), "")));
    assert_eq!(found.len(), 1, "{found:#?}");
    let d = &found[0];
    assert_eq!(d.subject_id.as_deref(), Some("c"));
    assert!(
        d.message.contains("chart 'c' text ink leaves"),
        "{}",
        d.message
    );
    assert!(d.message.contains("bottom"), "{}", d.message);
    match d.fix() {
        Some(FixHint::SetProperty { property, to }) => {
            assert_eq!(property, "h");
            assert!(d.message.contains(&format!("h={to}")), "{}", d.message);
            let px: f64 = to.trim_start_matches("(px)").parse().expect("px value");
            assert!(px > 50.0, "{to}");
        }
        other => panic!("expected SetProperty fix, got {other:?}"),
    }
}

#[test]
fn tall_chart_is_silent() {
    assert!(hits(&doc(&chart("bar", (400, 200), ""))).is_empty());
}

#[test]
fn normal_charts_of_every_cartesian_kind_are_silent() {
    for (kind, attrs) in [
        ("bar", "legend=#true"),
        ("line", "legend=#true"),
        ("bar", r#"orientation="horizontal" legend=#true"#),
    ] {
        let found = hits(&doc(&chart(kind, (480, 300), attrs)));
        assert!(found.is_empty(), "{kind} {attrs}: {found:#?}");
    }
}
