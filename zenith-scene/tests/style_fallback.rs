//! Style fallback for `align`, `v-align`, and `shadow`: a node without the
//! attribute takes its style's value and compiles to the same scene as the
//! attribute form. Also covers the `shape` `shadow` attribute and the byte
//! identity of documents that carry a `defaults` block (no render effect yet).

mod common;
use common::*;
use zenith_scene::ir::SceneCommand;

/// A document with style `s` holding `style_body` and `page_body` on the page.
fn doc(style_body: &str, page_body: &str) -> String {
    format!(
        r##"zenith version=1 {{
  tokens format="zenith-token-v1" {{
    token id="color.ink" type="color" value="#102030"
    token id="color.fill" type="color" value="#c0d0e0"
    token id="shadow.soft" type="shadow" {{
      layer dx=(px)2 dy=(px)3 blur=(px)4 color=(token)"color.ink"
    }}
  }}
  styles {{
    style id="s" {{
      {style_body}
    }}
  }}
  assets {{
    asset id="asset.swatch" kind="image" src="assets/swatch.png"
  }}
  document id="doc" {{
    page id="p" w=(px)400 h=(px)400 {{
      {page_body}
    }}
  }}
}}
"##
    )
}

fn commands(src: &str) -> Vec<SceneCommand> {
    compile(&parse(src), &default_provider()).scene.commands
}

/// Compile `attr_body` (attribute form, empty style) and `style_body` /
/// `styled_body` (style form) and assert the scenes are equal and non-trivial.
fn assert_same(attr_body: &str, style_body: &str, styled_body: &str) -> Vec<SceneCommand> {
    let by_attr = commands(&doc("", attr_body));
    let by_style = commands(&doc(style_body, styled_body));
    assert!(!by_attr.is_empty());
    assert_eq!(by_attr, by_style, "style fallback must match the attribute");
    by_attr
}

fn has_shadow(cmds: &[SceneCommand]) -> bool {
    cmds.iter()
        .any(|c| matches!(c, SceneCommand::BeginShadow { .. }))
}

#[test]
fn text_align_falls_back_to_style() {
    let cmds = assert_same(
        r#"text id="t" x=(px)10 y=(px)10 w=(px)300 fill=(token)"color.ink" align="center" { span "Hi" }"#,
        r#"align "center""#,
        r#"text id="t" x=(px)10 y=(px)10 w=(px)300 fill=(token)"color.ink" style="s" { span "Hi" }"#,
    );
    let start = commands(&doc(
        "",
        r#"text id="t" x=(px)10 y=(px)10 w=(px)300 fill=(token)"color.ink" { span "Hi" }"#,
    ));
    assert_ne!(cmds, start, "center must differ from the start default");
}

#[test]
fn text_attribute_beats_style_align() {
    let attr = commands(&doc(
        r#"align "center""#,
        r#"text id="t" x=(px)10 y=(px)10 w=(px)300 fill=(token)"color.ink" style="s" align="end" { span "Hi" }"#,
    ));
    let plain = commands(&doc(
        "",
        r#"text id="t" x=(px)10 y=(px)10 w=(px)300 fill=(token)"color.ink" align="end" { span "Hi" }"#,
    ));
    assert_eq!(attr, plain);
}

#[test]
fn markdown_text_align_falls_back_to_style() {
    assert_same(
        r##"text id="t" x=(px)10 y=(px)10 w=(px)300 fill=(token)"color.ink" format="markdown" align="end" { span "# Head\n\nBody" }"##,
        r#"align "end""#,
        r##"text id="t" x=(px)10 y=(px)10 w=(px)300 fill=(token)"color.ink" format="markdown" style="s" { span "# Head\n\nBody" }"##,
    );
}

#[test]
fn text_v_align_falls_back_to_style() {
    assert_same(
        r#"text id="t" x=(px)10 y=(px)10 w=(px)300 h=(px)200 fill=(token)"color.ink" v-align="bottom" { span "Hi" }"#,
        r#"v-align "bottom""#,
        r#"text id="t" x=(px)10 y=(px)10 w=(px)300 h=(px)200 fill=(token)"color.ink" style="s" { span "Hi" }"#,
    );
}

#[test]
fn shape_v_align_falls_back_to_style() {
    assert_same(
        r#"shape id="sh" x=(px)10 y=(px)10 w=(px)200 h=(px)120 fill=(token)"color.fill" v-align="top" { span "Go" }"#,
        r#"v-align "top""#,
        r#"shape id="sh" x=(px)10 y=(px)10 w=(px)200 h=(px)120 fill=(token)"color.fill" style="s" { span "Go" }"#,
    );
}

#[test]
fn table_v_align_falls_back_to_style() {
    let table = |attrs: &str| {
        format!(
            r#"table id="tb" x=(px)10 y=(px)10 w=(px)200 h=(px)120 cell-padding=(px)0 gap=(px)0 {attrs} {{
        column width=(px)200
        row {{ cell {{ text id="c" fill=(token)"color.ink" {{ span "Cell" }} }} }}
      }}"#
        )
    };
    assert_same(
        &table(r#"v-align="bottom""#),
        r#"v-align "bottom""#,
        &table(r#"style="s""#),
    );
}

#[test]
fn table_cell_text_style_align_beats_cell_default() {
    let table = |text_attrs: &str| {
        format!(
            r#"table id="tb" x=(px)10 y=(px)10 w=(px)200 h=(px)120 cell-padding=(px)0 gap=(px)0 h-align="start" {{
        column width=(px)200
        row {{ cell {{ text id="c" fill=(token)"color.ink" {text_attrs} {{ span "Cell" }} }} }}
      }}"#
        )
    };
    assert_same(
        &table(r#"align="end""#),
        r#"align "end""#,
        &table(r#"style="s""#),
    );
}

#[test]
fn shadow_falls_back_to_style_on_every_kind() {
    let cases: &[(&str, &str)] = &[
        (
            r#"rect id="n" x=(px)10 y=(px)10 w=(px)80 h=(px)40 fill=(token)"color.fill" {X}"#,
            "rect",
        ),
        (
            r#"ellipse id="n" x=(px)10 y=(px)10 w=(px)80 h=(px)40 fill=(token)"color.fill" {X}"#,
            "ellipse",
        ),
        (
            r#"text id="n" x=(px)10 y=(px)10 w=(px)200 fill=(token)"color.ink" {X} { span "Hi" }"#,
            "text",
        ),
        (
            r#"image id="n" asset="asset.swatch" x=(px)10 y=(px)10 w=(px)80 h=(px)40 fit="stretch" {X}"#,
            "image",
        ),
        (
            r#"frame id="n" x=(px)10 y=(px)10 w=(px)120 h=(px)100 fill=(token)"color.fill" {X} { rect id="in" x=(px)0 y=(px)0 w=(px)20 h=(px)20 fill=(token)"color.ink" }"#,
            "frame",
        ),
        (
            r#"group id="n" x=(px)10 y=(px)10 {X} { rect id="in" x=(px)0 y=(px)0 w=(px)20 h=(px)20 fill=(token)"color.ink" }"#,
            "group",
        ),
        (
            r#"shape id="n" x=(px)10 y=(px)10 w=(px)120 h=(px)60 fill=(token)"color.fill" {X} { span "Go" }"#,
            "shape",
        ),
    ];
    for (template, kind) in cases {
        let by_attr = template.replace("{X}", r#"shadow=(token)"shadow.soft""#);
        let by_style = template.replace("{X}", r#"style="s""#);
        let cmds = assert_same(&by_attr, r#"shadow (token)"shadow.soft""#, &by_style);
        assert!(has_shadow(&cmds), "{kind}: BeginShadow expected: {cmds:?}");
    }
}

#[test]
fn shape_shadow_brackets_body_and_label_draws_on_top() {
    let body = |attrs: &str| {
        format!(
            r#"shape id="sh" x=(px)10 y=(px)10 w=(px)200 h=(px)80 fill=(token)"color.fill" {attrs} {{ span "Go" }}"#
        )
    };
    let with = commands(&doc("", &body(r#"shadow=(token)"shadow.soft""#)));
    let without = commands(&doc("", &body("")));
    assert!(!has_shadow(&without));

    let begin = with
        .iter()
        .position(|c| matches!(c, SceneCommand::BeginShadow { .. }))
        .expect("BeginShadow");
    let end = with
        .iter()
        .position(|c| matches!(c, SceneCommand::EndShadow))
        .expect("EndShadow");
    assert!(begin < end);
    let inside = with.get(begin + 1..end).expect("bracket");
    assert!(
        inside.iter().any(|c| matches!(
            c,
            SceneCommand::FillRect { .. } | SceneCommand::FillPath { .. }
        )),
        "the body draws inside the bracket: {inside:?}"
    );
    assert!(
        !inside
            .iter()
            .any(|c| matches!(c, SceneCommand::DrawGlyphRun { .. })),
        "the label is not shadowed: {inside:?}"
    );
    assert!(
        with.get(end + 1..).is_some_and(|tail| tail
            .iter()
            .any(|c| matches!(c, SceneCommand::DrawGlyphRun { .. }))),
        "the label draws after the bracket"
    );

    // Additive: removing the bracket gives the shadow-free scene exactly.
    let stripped: Vec<SceneCommand> = with
        .into_iter()
        .filter(|c| {
            !matches!(
                c,
                SceneCommand::BeginShadow { .. } | SceneCommand::EndShadow
            )
        })
        .collect();
    assert_eq!(stripped, without);
}

#[test]
fn defaults_block_has_no_render_effect_yet() {
    let page = r#"text id="t" x=(px)10 y=(px)10 w=(px)300 fill=(token)"color.ink" { span "Hi" }
      shape id="sh" x=(px)10 y=(px)60 w=(px)200 h=(px)80 fill=(token)"color.fill" { span "Go" }"#;
    let plain = commands(&doc(r#"align "center""#, page));
    let src = doc(r#"align "center""#, page).replace(
        "  assets {",
        "  defaults {\n    text style=\"s\"\n    shape style=\"s\" text-style=\"s\"\n  }\n  assets {",
    );
    assert!(src.contains("defaults {"));
    let with_page_defaults = src.replace(
        "page id=\"p\" w=(px)400 h=(px)400 {\n",
        "page id=\"p\" w=(px)400 h=(px)400 {\n      defaults { rect style=\"s\" }\n",
    );
    assert!(with_page_defaults.contains("defaults { rect"));
    assert_eq!(commands(&with_page_defaults), plain);
}

#[test]
fn shape_h_align_falls_back_to_style_align() {
    let shape = |attrs: &str| {
        format!(
            r#"shape id="sh" x=(px)10 y=(px)10 w=(px)300 h=(px)80 fill=(token)"color.fill" {attrs} {{ span "Go" }}"#
        )
    };
    let cmds = assert_same(
        &shape(r#"h-align="end""#),
        r#"align "end""#,
        &shape(r#"style="s""#),
    );
    assert_ne!(
        cmds,
        commands(&doc("", &shape(""))),
        "end differs from center"
    );
    // `justify` is not an h-align: the style value is ignored (center default).
    assert_eq!(
        commands(&doc(r#"align "justify""#, &shape(r#"style="s""#))),
        commands(&doc("", &shape("")))
    );
    // The node attribute beats the style.
    assert_eq!(
        commands(&doc(
            r#"align "end""#,
            &shape(r#"style="s" h-align="start""#)
        )),
        commands(&doc("", &shape(r#"h-align="start""#)))
    );
}

#[test]
fn table_h_align_falls_back_to_style_align() {
    let table = |attrs: &str| {
        format!(
            r#"table id="tb" x=(px)10 y=(px)10 w=(px)200 h=(px)120 cell-padding=(px)0 gap=(px)0 {attrs} {{
        column width=(px)200
        row {{ cell {{ rect id="c" x=(px)0 y=(px)0 w=(px)20 h=(px)20 fill=(token)"color.ink" }} }}
      }}"#
        )
    };
    let cmds = assert_same(
        &table(r#"h-align="end""#),
        r#"align "end""#,
        &table(r#"style="s""#),
    );
    assert_ne!(
        cmds,
        commands(&doc("", &table(""))),
        "end differs from start"
    );
    assert_eq!(
        commands(&doc(r#"align "justify""#, &table(r#"style="s""#))),
        commands(&doc("", &table("")))
    );
}

/// Remove every BeginShadow/EndShadow from `cmds`.
fn without_shadow(cmds: Vec<SceneCommand>) -> Vec<SceneCommand> {
    cmds.into_iter()
        .filter(|c| {
            !matches!(
                c,
                SceneCommand::BeginShadow { .. } | SceneCommand::EndShadow
            )
        })
        .collect()
}

#[test]
fn pattern_shadow_wraps_the_tiling_and_falls_back_to_style() {
    let pattern = |attrs: &str| {
        format!(
            r#"pattern id="pt" kind="grid" x=(px)10 y=(px)10 w=(px)200 h=(px)100 spacing=(px)40 fill=(token)"color.fill" {attrs} {{
        ellipse id="dot" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.ink"
      }}"#
        )
    };
    let plain = commands(&doc("", &pattern("")));
    assert!(!has_shadow(&plain));
    let with = assert_same(
        &pattern(r#"shadow=(token)"shadow.soft""#),
        r#"shadow (token)"shadow.soft""#,
        &pattern(r#"style="s""#),
    );
    assert!(has_shadow(&with));
    let begin = with
        .iter()
        .position(|c| matches!(c, SceneCommand::BeginShadow { .. }))
        .expect("begin");
    assert!(
        matches!(with.get(begin + 1), Some(SceneCommand::PushClip { .. })),
        "the shadow wraps the clipped tiling: {with:?}"
    );
    assert_eq!(
        with.iter()
            .filter(|c| matches!(c, SceneCommand::BeginShadow { .. }))
            .count(),
        1,
        "one shadow for the whole tiling"
    );
    assert_eq!(without_shadow(with), plain);
}

#[test]
fn chart_shadow_wraps_the_chart_and_falls_back_to_style() {
    let chart = |attrs: &str| {
        format!(
            r#"chart id="ch" kind="pie" x=(px)10 y=(px)10 w=(px)300 h=(px)200 {attrs} {{
        series 50.0 50.0
      }}"#
        )
    };
    let plain = commands(&doc("", &chart("")));
    assert!(!plain.is_empty(), "chart renders");
    assert!(!has_shadow(&plain));
    let with = assert_same(
        &chart(r#"shadow=(token)"shadow.soft""#),
        r#"shadow (token)"shadow.soft""#,
        &chart(r#"style="s""#),
    );
    // Inside the page clip, the whole chart ink sits in one shadow bracket.
    let inner = with
        .get(1..with.len().saturating_sub(1))
        .expect("page clip body");
    assert!(
        matches!(inner.first(), Some(SceneCommand::BeginShadow { .. })),
        "{with:?}"
    );
    assert!(
        matches!(inner.last(), Some(SceneCommand::EndShadow)),
        "{with:?}"
    );
    assert_eq!(without_shadow(with), plain);
}

/// Style `s` with `s_body` plus style `lbl` with `lbl_body`.
fn two_styles(s_body: &str, lbl_body: &str) -> String {
    format!("{s_body}\n    }}\n    style id=\"lbl\" {{\n      {lbl_body}")
}

#[test]
fn shape_label_align_precedence() {
    let shape = |attrs: &str| {
        format!(
            r#"shape id="sh" x=(px)10 y=(px)10 w=(px)300 h=(px)80 fill=(token)"color.fill" {attrs} {{ span "Go" }}"#
        )
    };
    let by_attr = |align: &str| commands(&doc("", &shape(&format!(r#"h-align="{align}""#))));
    // text-style align beats the shape style align.
    assert_eq!(
        commands(&doc(
            &two_styles(r#"align "start""#, r#"align "end""#),
            &shape(r#"style="s" text-style="lbl""#)
        )),
        by_attr("end")
    );
    // The shape style align applies when the text-style has none.
    assert_eq!(
        commands(&doc(
            &two_styles(r#"align "start""#, r#"fill (token)"color.ink""#),
            &shape(r#"style="s" text-style="lbl""#)
        )),
        commands(&doc(
            &two_styles("", r#"fill (token)"color.ink""#),
            &shape(r#"text-style="lbl" h-align="start""#)
        ))
    );
    // The shape h-align beats the text-style align.
    assert_eq!(
        commands(&doc(
            &two_styles("", r#"align "end""#),
            &shape(r#"text-style="lbl" h-align="start""#)
        )),
        commands(&doc(
            &two_styles("", r#"align "center""#),
            &shape(r#"text-style="lbl" h-align="start""#)
        ))
    );
    assert_ne!(by_attr("end"), by_attr("start"));
}

#[test]
fn connector_label_takes_text_style_align() {
    let page = |text_style: &str| {
        format!(
            r#"rect id="a" x=(px)10 y=(px)10 w=(px)40 h=(px)40 fill=(token)"color.fill"
      rect id="b" x=(px)300 y=(px)10 w=(px)40 h=(px)40 fill=(token)"color.fill"
      connector id="c" from="a" to="b" stroke=(token)"color.ink" text-style="{text_style}" {{ span "yes" }}"#
        )
    };
    let end = commands(&doc(
        &two_styles(r#"align "end""#, r#"align "center""#),
        &page("s"),
    ));
    let center = commands(&doc(
        &two_styles(r#"align "end""#, r#"align "center""#),
        &page("lbl"),
    ));
    let plain = commands(&doc(
        &two_styles(r#"v-align "top""#, r#"align "center""#),
        &page("s"),
    ));
    assert_ne!(end, center, "text-style align end moves the label");
    assert_eq!(center, plain, "absent align keeps the centered label");
}
