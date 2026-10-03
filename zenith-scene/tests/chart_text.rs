//! Chart text: style, `defaults`, and content pairing drive size and colour;
//! the engine default scales with the chart box and contrasts with the
//! backdrop; every string carries a `<chart>/<role>/<index>` source; the page
//! lint judges chart text size and contrast per role.

mod common;
use common::*;
use zenith_core::{Diagnostic, default_provider};
use zenith_scene::{DocumentPrep, PageCompiler};

/// A one-page document. `tokens`, `styles`, and `top` (a `defaults` block)
/// extend the base; `body` fills a `w` × `h` page on `color.paper`.
fn doc(tokens: &str, styles: &str, top: &str, (w, h): (u32, u32), body: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.ct" name="ChartText"
  tokens format="zenith-token-v1" {{
    token id="color.paper" type="color" value="#ffffff"
    token id="color.night" type="color" value="#101820"
    token id="color.red" type="color" value="#cc0000"
    token id="color.ghost" type="color" value="#fafafa"
    token id="size.chart" type="dimension" value=(px)20
    token id="size.tiny" type="dimension" value=(px)6
    token id="width.axis" type="dimension" value=(px)3
{tokens}
  }}
  styles {{
{styles}
  }}
  {top}
  document id="doc.ct" title="ChartText" {{
    page id="p" w=(px){w} h=(px){h} background=(token)"color.paper" {{
{body}
    }}
  }}
}}
"##
    )
}

/// A bar chart `id` at `(x, y)` sized `w` × `h` with a title, a legend, and
/// three categories. `attrs` adds chart attributes.
fn bar(id: &str, (x, y, w, h): (u32, u32, u32, u32), attrs: &str) -> String {
    format!(
        r#"      chart id="{id}" kind="bar" x=(px){x} y=(px){y} w=(px){w} h=(px){h} title="Sales" legend=#true {attrs} {{
        categories "North" "South" "West"
        series label="2025" 10.0 20.0 30.0
      }}"#
    )
}

fn compile_doc(src: &str) -> CompileResult {
    let document = parse(src);
    let fonts = default_provider();
    let prep = DocumentPrep::new(&document, None, None);
    PageCompiler::new(&prep, &fonts).compile_page_local(0)
}

/// `(source id, font size, rgb)` of every glyph run whose source starts with
/// `prefix`.
fn runs(result: &CompileResult, prefix: &str) -> Vec<(String, f32, (u8, u8, u8))> {
    result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::DrawGlyphRun {
                source_node_id: Some(source),
                font_size,
                color,
                ..
            } if source.starts_with(prefix) => {
                Some((source.clone(), *font_size, (color.r, color.g, color.b)))
            }
            _ => None,
        })
        .collect()
}

/// The single font size and colour of the runs of `chart` in `role`.
fn role_look(result: &CompileResult, chart: &str, role: &str) -> (f32, (u8, u8, u8)) {
    let found = runs(result, &format!("{chart}/{role}/"));
    assert!(!found.is_empty(), "no {role} runs for {chart}");
    let (_, size, rgb) = found[0].clone();
    for (source, s, c) in &found {
        assert_eq!((*s, *c), (size, rgb), "{source} differs within the role");
    }
    (size, rgb)
}

fn with_code<'a>(diags: &'a [Diagnostic], code: &str) -> Vec<&'a Diagnostic> {
    diags.iter().filter(|d| d.code == code).collect()
}

const BLACK: (u8, u8, u8) = (0, 0, 0);
const WHITE: (u8, u8, u8) = (255, 255, 255);
const RED: (u8, u8, u8) = (0xcc, 0, 0);

#[test]
fn style_sets_the_base_size_and_ink_of_every_role() {
    let result = compile_doc(&doc(
        "",
        r#"    style id="cs" { fill (token)"color.red"; font-size (token)"size.chart" }"#,
        "",
        (1000, 800),
        &bar("c", (50, 50, 600, 400), r#"style="cs""#),
    ));
    assert_eq!(role_look(&result, "c", "axis"), (20.0, RED));
    assert_eq!(role_look(&result, "c", "category"), (20.0, RED));
    assert_eq!(role_look(&result, "c", "legend"), (20.0, RED));
    assert_eq!(role_look(&result, "c", "title"), (25.0, RED));
    // Grouped bars label on top, on the plot background.
    assert_eq!(role_look(&result, "c", "value"), (17.5, RED));
}

#[test]
fn engine_default_base_scales_with_the_chart_box() {
    let body = [
        bar("small", (0, 0, 400, 300), ""),
        bar("large", (0, 400, 1200, 600), ""),
    ]
    .join("\n");
    let result = compile_doc(&doc("", "", "", (1400, 1100), &body));
    assert_eq!(role_look(&result, "small", "axis").0, 12.0);
    assert_eq!(role_look(&result, "large", "axis").0, 24.0);
    assert_eq!(role_look(&result, "large", "title").0, 30.0);
}

#[test]
fn engine_default_ink_contrasts_with_the_backdrop() {
    let body = format!(
        "{}\n      frame id=\"dark\" x=(px)0 y=(px)400 w=(px)500 h=(px)400 fill=(token)\"color.night\" {{\n{}\n      }}",
        bar("light", (0, 0, 400, 300), ""),
        bar("ondark", (20, 20, 400, 300), ""),
    );
    let result = compile_doc(&doc("", "", "", (600, 900), &body));
    assert_eq!(role_look(&result, "light", "axis").1, BLACK);
    assert_eq!(role_look(&result, "ondark", "axis").1, WHITE);
    assert_eq!(role_look(&result, "ondark", "title").1, WHITE);
}

#[test]
fn engine_default_ink_prefers_a_declared_content_token() {
    let tokens = r##"    token id="color.base.content" type="color" value="#0d1529""##;
    let result = compile_doc(&doc(
        tokens,
        "",
        "",
        (600, 400),
        &bar("c", (0, 0, 400, 300), ""),
    ));
    assert_eq!(role_look(&result, "c", "axis").1, (0x0d, 0x15, 0x29));
}

#[test]
fn defaults_row_and_content_pairing_drive_chart_text() {
    let tokens = r##"    token id="color.base.content" type="color" value="#0d1529"
    token id="color.night.content" type="color" value="#f9fafb"
    token id="size.caption" type="dimension" value=(px)18"##;
    let styles = r#"    style id="ui.chart" { fill (token)"color.base.content"; font-size (token)"size.caption" }"#;
    let top = "defaults {\n    chart style=\"ui.chart\"\n  }";
    let body = format!(
        "{}\n      frame id=\"dark\" x=(px)0 y=(px)400 w=(px)500 h=(px)400 fill=(token)\"color.night\" {{\n{}\n      }}",
        bar("page", (0, 0, 400, 300), ""),
        bar("framed", (20, 20, 400, 300), ""),
    );
    let result = compile_doc(&doc(tokens, styles, top, (600, 900), &body));
    assert_eq!(
        role_look(&result, "page", "axis"),
        (18.0, (0x0d, 0x15, 0x29))
    );
    assert_eq!(
        role_look(&result, "framed", "axis"),
        (18.0, (0xf9, 0xfa, 0xfb))
    );
    assert_eq!(role_look(&result, "framed", "title").0, 22.5);
}

#[test]
fn stroke_and_stroke_width_paint_the_axes() {
    let result = compile_doc(&doc(
        "",
        "",
        "",
        (600, 400),
        &bar(
            "c",
            (0, 0, 400, 300),
            r#"stroke=(token)"color.red" stroke-width=(token)"width.axis""#,
        ),
    ));
    let lines: Vec<((u8, u8, u8), f64)> = result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::StrokeLine {
                color,
                stroke_width,
                ..
            } => Some(((color.r, color.g, color.b), *stroke_width)),
            _ => None,
        })
        .collect();
    assert!(lines.iter().all(|(_, w)| *w == 3.0), "{lines:?}");
    assert_eq!(lines.iter().filter(|(c, _)| *c == RED).count(), 2, "axes");
    assert!(
        lines.iter().any(|(c, _)| *c != RED),
        "gridlines are lighter"
    );
}

#[test]
fn value_labels_inside_a_slice_contrast_with_it() {
    // Four equal slices: palette slot 3 is yellow, slot 0 is blue.
    let body = r#"      chart id="pie" kind="pie" x=(px)0 y=(px)0 w=(px)400 h=(px)400 {
        series 1.0 1.0 1.0 1.0
      }"#;
    let result = compile_doc(&doc("", "", "", (600, 600), body));
    let labels = runs(&result, "pie/value-inside/");
    let color_of = |i: usize| {
        labels
            .iter()
            .find(|(s, _, _)| *s == format!("pie/value-inside/{i}"))
            .map(|(_, _, c)| *c)
    };
    assert_eq!(color_of(3), Some(BLACK), "{labels:?}");
    assert_eq!(color_of(0), Some(WHITE), "{labels:?}");
}

#[test]
fn every_chart_run_carries_a_role_source() {
    let result = compile_doc(&doc(
        "",
        "",
        "",
        (600, 400),
        &bar("c", (0, 0, 400, 300), r#"caption="Source: FY25""#),
    ));
    for command in &result.scene.commands {
        if let SceneCommand::DrawGlyphRun { source_node_id, .. } = command {
            let source = source_node_id.as_deref().unwrap_or("");
            let mut parts = source.splitn(3, '/');
            assert_eq!(parts.next(), Some("c"), "{source}");
            let role = parts.next().unwrap_or("");
            assert!(
                [
                    "title",
                    "caption",
                    "axis",
                    "category",
                    "legend",
                    "value",
                    "value-inside"
                ]
                .contains(&role),
                "{source}"
            );
            let index = parts.next().unwrap_or("");
            assert!(index.parse::<usize>().is_ok(), "{source}");
        }
    }
    assert_eq!(runs(&result, "c/caption/").len(), 1, "caption draws");
}

#[test]
fn tiny_chart_text_fires_too_small_once_per_role() {
    let result = compile_doc(&doc(
        "",
        r#"    style id="tiny" { font-size (token)"size.tiny" }"#,
        "",
        (1920, 1080),
        &bar("c", (100, 100, 800, 600), r#"style="tiny""#),
    ));
    let small = with_code(&result.diagnostics, "text.too_small");
    let axis: Vec<_> = small
        .iter()
        .filter(|d| d.message.contains("chart 'c' axis text"))
        .collect();
    assert_eq!(axis.len(), 1, "{small:#?}");
    assert!(
        axis[0]
            .message
            .contains("set the chart style font-size to at least 10px"),
        "{}",
        axis[0].message
    );
    assert_eq!(axis[0].subject_id.as_deref(), Some("c"));
    let value: Vec<_> = small
        .iter()
        .filter(|d| d.message.contains("chart 'c' value text"))
        .collect();
    assert_eq!(value.len(), 1, "{small:#?}");
    assert!(
        value[0].message.contains("at least 12px"),
        "{}",
        value[0].message
    );
}

#[test]
fn low_contrast_chart_text_fires_with_the_chart_role() {
    let result = compile_doc(&doc(
        "",
        "",
        "",
        (1000, 800),
        &bar("c", (50, 50, 600, 400), r#"fill=(token)"color.ghost""#),
    ));
    let contrast: Vec<&Diagnostic> = result
        .diagnostics
        .iter()
        .filter(|d| d.code.starts_with("contrast."))
        .collect();
    let axis: Vec<_> = contrast
        .iter()
        .filter(|d| d.message.contains("chart 'c' axis text"))
        .collect();
    assert_eq!(axis.len(), 1, "{contrast:#?}");
    assert_eq!(axis[0].subject_id.as_deref(), Some("c"));
    assert!(
        contrast
            .iter()
            .any(|d| d.message.contains("chart 'c' title text")),
        "{contrast:#?}"
    );
}

#[test]
fn engine_default_chart_passes_the_lint() {
    let body = format!(
        "{}\n      frame id=\"dark\" x=(px)900 y=(px)100 w=(px)900 h=(px)800 fill=(token)\"color.night\" {{\n{}\n      }}",
        bar("light", (100, 100, 700, 500), ""),
        bar("ondark", (50, 50, 700, 500), ""),
    );
    let result = compile_doc(&doc("", "", "", (1920, 1080), &body));
    let flagged: Vec<&Diagnostic> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "text.too_small" || d.code.starts_with("contrast."))
        .collect();
    assert!(flagged.is_empty(), "{flagged:#?}");
}

#[test]
fn chart_text_is_deterministic() {
    let src = doc(
        "",
        "",
        "",
        (1000, 800),
        &bar("c", (50, 50, 600, 400), r#"caption="Note""#),
    );
    let a = compile_doc(&src);
    let b = compile_doc(&src);
    assert_eq!(a.scene.commands, b.scene.commands);
    assert_eq!(
        format!("{:?}", a.diagnostics),
        format!("{:?}", b.diagnostics)
    );
}
