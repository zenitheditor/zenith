mod common;
use common::*;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::SceneCommand;

/// (a) A column frame stacks two children vertically separated by `gap`:
/// child2.y == child1.y + child1.h + gap.
#[test]
fn column_frame_stacks_children_with_gap() {
    // pad=0, gap=10. Two rects each with declared h=30. Both omit x/y/w so
    // the column path injects content_left/cursor_y and content_w.
    let src = r##"zenith version=1 {
  project id="proj.col1" name="Col1"
  tokens format="zenith-token-v1" {
token id="color.k" type="color" value="#000000"
token id="space.gap" type="dimension" value=(px)10
  }
  styles {
style id="style.col" {
  gap (token)"space.gap"
}
  }
  document id="doc.col1" title="Col1" {
page id="page.col1" w=(px)200 h=(px)200 {
  frame id="frame.col" x=(px)20 y=(px)30 w=(px)160 h=(px)160 layout="column" clip=#true style="style.col" {
    rect id="rect.a" h=(px)30 fill=(token)"color.k"
    rect id="rect.b" h=(px)30 fill=(token)"color.k"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    assert!(
        result.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );

    let rects = fill_rects(&result);
    assert_eq!(
        rects.len(),
        2,
        "expected two child FillRects; got {rects:?}"
    );
    let (_, y1, _, h1) = rects[0];
    let (_, y2, _, _) = rects[1];
    // content_top = frame_y(30) + pad(0) = 30.
    assert_eq!(y1, 30.0, "child1 y must be content_top (30); got {y1}");
    // child2.y == child1.y + child1.h + gap = 30 + 30 + 10 = 70.
    assert_eq!(
        y2,
        y1 + h1 + 10.0,
        "child2 y must be child1.y + child1.h + gap; got {y2}"
    );
}

/// (b) Padding insets children: child_x == frame_x + pad and
/// child_w == frame_w - 2*pad (when the child declares no own w).
#[test]
fn column_frame_padding_insets_children() {
    // pad=16, frame x=20 w=160 → content_left=36, content_w=128.
    let src = r##"zenith version=1 {
  project id="proj.col2" name="Col2"
  tokens format="zenith-token-v1" {
token id="color.k" type="color" value="#000000"
token id="space.pad" type="dimension" value=(px)16
  }
  styles {
style id="style.col" {
  padding (token)"space.pad"
}
  }
  document id="doc.col2" title="Col2" {
page id="page.col2" w=(px)200 h=(px)200 {
  frame id="frame.col" x=(px)20 y=(px)30 w=(px)160 h=(px)160 layout="column" clip=#true style="style.col" {
    rect id="rect.a" h=(px)30 fill=(token)"color.k"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    assert!(
        result.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );

    let rects = fill_rects(&result);
    assert_eq!(rects.len(), 1, "expected one child FillRect; got {rects:?}");
    let (x, y, w, _) = rects[0];
    assert_eq!(x, 36.0, "child_x must be frame_x + pad (20+16=36); got {x}");
    assert_eq!(y, 46.0, "child_y must be content_top (30+16=46); got {y}");
    assert_eq!(
        w, 128.0,
        "child_w must be frame_w - 2*pad (160-32=128); got {w}"
    );
}

/// (c) Layout absent / "absolute": a child with explicit x/y produces a
/// byte-identical command stream to the clip-only model (no column injection).
#[test]
fn layout_absent_is_byte_identical() {
    // Same document twice: once with layout="absolute", once with no layout.
    // Both must equal the clip-only output where the child keeps its own
    // x=50 y=60 coords.
    let make = |layout_attr: &str| {
        format!(
            r##"zenith version=1 {{
  project id="proj.col3" name="Col3"
  tokens format="zenith-token-v1" {{
token id="color.k" type="color" value="#000000"
  }}
  styles {{}}
  document id="doc.col3" title="Col3" {{
page id="page.col3" w=(px)200 h=(px)200 {{
  frame id="frame.abs" x=(px)20 y=(px)30 w=(px)160 h=(px)160 {layout_attr} {{
    rect id="rect.a" x=(px)50 y=(px)60 w=(px)40 h=(px)30 fill=(token)"color.k"
  }}
}}
  }}
}}
"##
        )
    };

    let base = compile(&parse(&make("")), &default_provider());
    let absolute = compile(&parse(&make("layout=\"absolute\"")), &default_provider());

    assert_eq!(
        base.scene.commands, absolute.scene.commands,
        "layout=\"absolute\" must be byte-identical to no-layout clip-only output"
    );
    // And the child kept its own absolute coords (no column injection).
    let rects = fill_rects(&base);
    assert_eq!(rects, vec![(50.0, 60.0, 40.0, 30.0)]);
}

/// (e) A text child WITHOUT a declared `h` gets a measured height so the
/// cursor advances past it — a following rect sits below the text block.
#[test]
fn column_text_without_h_advances_cursor() {
    // pad=0, gap=0. A text child (no h) followed by a rect (h=20). The rect
    // must sit at content_top + measured_text_height (> content_top), proving
    // the text's intrinsic height advanced the cursor.
    let src = r##"zenith version=1 {
  project id="proj.col5" name="Col5"
  tokens format="zenith-token-v1" {
token id="color.k" type="color" value="#000000"
  }
  styles {}
  document id="doc.col5" title="Col5" {
page id="page.col5" w=(px)400 h=(px)400 {
  frame id="frame.col" x=(px)0 y=(px)0 w=(px)300 h=(px)300 layout="column" clip=#true {
    text id="text.a" font-size=(px)20 {
      span "Hello column layout"
    }
    rect id="rect.below" h=(px)20 fill=(token)"color.k"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    // The rect is the only FillRect; its y must be strictly below content_top
    // (0.0) by the text's laid-out line height.
    let rects = fill_rects(&result);
    assert_eq!(
        rects.len(),
        1,
        "expected one child FillRect (the rect); got {rects:?}"
    );
    let (_, rect_y, _, _) = rects[0];
    assert!(
        rect_y > 0.0,
        "rect must sit below the text (cursor advanced by measured text height); got y={rect_y}"
    );

    // Sanity: a glyph run for the text was emitted above the rect.
    let has_glyphs = result
        .scene
        .commands
        .iter()
        .any(|c| matches!(c, SceneCommand::DrawGlyphRun { .. }));
    assert!(has_glyphs, "expected the text child to emit glyph runs");
}

/// Wrap `frame_line` (a `frame … {` opener) and its children in a minimal doc.
fn column_doc(frame_line: &str, children: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.colx" name="ColX"
  tokens format="zenith-token-v1" {{
token id="color.k" type="color" value="#000000"
token id="space.pad" type="dimension" value=(px)16
  }}
  styles {{
style id="style.pad" {{
  padding (token)"space.pad"
  gap (token)"space.pad"
}}
  }}
  document id="doc.colx" title="ColX" {{
page id="page.colx" w=(px)400 h=(px)400 {{
  {frame_line}
{children}
  }}
}}
  }}
}}
"##
    )
}

/// Number of `PushClip` commands (the page itself may push one).
fn clip_count(result: &zenith_scene::CompileResult) -> usize {
    result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::PushClip { .. }))
        .count()
}

const TWO_RECTS: &str = r#"    rect id="rect.a" h=(px)30 fill=(token)"color.k"
    rect id="rect.b" h=(px)30 fill=(token)"color.k""#;

/// `clip` defaults off for a column frame and on for an absolute frame;
/// `clip=#true` restores the clip on a column frame.
#[test]
fn column_clip_defaults_off_and_opts_in() {
    let column = compile(
        &parse(&column_doc(
            r#"frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="column" {"#,
            TWO_RECTS,
        )),
        &default_provider(),
    );

    let clipped = compile(
        &parse(&column_doc(
            r#"frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="column" clip=#true {"#,
            TWO_RECTS,
        )),
        &default_provider(),
    );
    assert_eq!(
        clip_count(&clipped),
        clip_count(&column) + 1,
        "column frame clips only with clip=#true"
    );
    assert_eq!(fill_rects(&column), fill_rects(&clipped));

    let absolute = compile(
        &parse(&column_doc(
            r#"frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 {"#,
            r#"    rect id="rect.a" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.k""#,
        )),
        &default_provider(),
    );

    let unclipped = compile(
        &parse(&column_doc(
            r#"frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 clip=#false {"#,
            r#"    rect id="rect.a" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.k""#,
        )),
        &default_provider(),
    );
    assert_eq!(
        clip_count(&absolute),
        clip_count(&unclipped) + 1,
        "absolute frame clips by default; clip=#false removes it"
    );
    assert_eq!(
        clip_count(&column),
        clip_count(&unclipped),
        "column default matches clip=#false"
    );
}

/// `gap` / `padding` attributes (px literals) override the frame style.
#[test]
fn column_gap_and_padding_attrs_override_style() {
    let result = compile(
        &parse(&column_doc(
            r#"frame id="f" x=(px)20 y=(px)30 w=(px)160 h=(px)300 layout="column" style="style.pad" padding=(px)10 gap=(px)5 {"#,
            TWO_RECTS,
        )),
        &default_provider(),
    );
    let rects = fill_rects(&result);
    assert_eq!(
        rects,
        vec![(30.0, 40.0, 140.0, 30.0), (30.0, 75.0, 140.0, 30.0)]
    );
}

/// Per-side padding wins over per-axis, which wins over `padding`.
#[test]
fn column_padding_precedence_side_over_axis_over_all() {
    let result = compile(
        &parse(&column_doc(
            r#"frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)300 layout="column" padding=(px)10 padding-x=(px)4 padding-left=(px)2 padding-y=(px)6 {"#,
            r#"    rect id="rect.a" h=(px)30 fill=(token)"color.k""#,
        )),
        &default_provider(),
    );
    // left=2 (side), right=4 (axis), top=6 (axis).
    assert_eq!(fill_rects(&result), vec![(2.0, 6.0, 194.0, 30.0)]);
}

/// Style padding still applies when no padding attribute is set.
#[test]
fn column_style_padding_is_the_fallback() {
    let result = compile(
        &parse(&column_doc(
            r#"frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)300 layout="column" style="style.pad" {"#,
            r#"    rect id="rect.a" h=(px)30 fill=(token)"color.k""#,
        )),
        &default_provider(),
    );
    assert_eq!(fill_rects(&result), vec![(16.0, 16.0, 168.0, 30.0)]);
}

/// Until the row algorithm exists, `layout="row"` (rejected by validation)
/// renders like `absolute`: children keep their authored coordinates.
#[test]
fn row_renders_like_absolute_until_supported() {
    let child = r#"    rect id="rect.a" x=(px)50 y=(px)60 w=(px)40 h=(px)30 fill=(token)"color.k""#;
    let row = compile(
        &parse(&column_doc(
            r#"frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="row" clip=#true {"#,
            child,
        )),
        &default_provider(),
    );
    let absolute = compile(
        &parse(&column_doc(
            r#"frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 {"#,
            child,
        )),
        &default_provider(),
    );
    assert_eq!(row.scene.commands, absolute.scene.commands);
}
