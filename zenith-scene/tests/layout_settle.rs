//! Hug heights that depend on their own position: text that wraps around a
//! `text-exclusion` box and text that snaps to the page baseline grid. The
//! laid-out box must equal the rendered extent.

mod common;
use common::parse;
use zenith_core::default_provider;
use zenith_scene::ir::SceneCommand;
use zenith_scene::{DocumentPrep, LayoutBox, PageCompiler};

const LONG: &str = "The quick brown fox jumps over the lazy dog and then keeps running far \
                    beyond the box edge to force wrapping across many lines of body text here";

fn doc(page_attrs: &str, children: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.ls" name="LS"
  tokens format="zenith-token-v1" {{
    token id="color.k" type="color" value="#000000"
  }}
  styles {{}}
  document id="doc.ls" title="LS" {{
    page id="p" w=(px)600 h=(px)900 {page_attrs} {{
{children}
    }}
  }}
}}
"##
    )
}

struct Laid {
    text: LayoutBox,
    after: LayoutBox,
    passes: usize,
    /// Distinct baselines of the text's glyph runs, ascending.
    baselines: Vec<f64>,
    codes: Vec<String>,
}

fn lay_out(src: &str) -> Laid {
    let parsed = parse(src);
    let fonts = default_provider();
    let prep = DocumentPrep::new(&parsed, None, None);
    let compiler = PageCompiler::new(&prep, &fonts);
    let boxes = compiler.layout_boxes(0).expect("page 0").clone();
    let result = compiler.compile_page(0);
    let mut baselines: Vec<f64> = result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::DrawGlyphRun {
                y, source_node_id, ..
            } if source_node_id.as_deref() == Some("t") => Some(*y),
            _ => None,
        })
        .collect();
    baselines.sort_by(f64::total_cmp);
    baselines.dedup();
    Laid {
        text: *boxes.get("t").expect("t"),
        after: *boxes.get("after").expect("after"),
        passes: compiler.layout_stats(0).expect("stats").passes,
        baselines,
        codes: result.diagnostics.iter().map(|d| d.code.clone()).collect(),
    }
}

fn column(text_attrs: &str) -> String {
    format!(
        r#"      rect id="ex" x=(px)0 y=(px)130 w=(px)250 h=(px)120 fill=(token)"color.k"
      frame id="f" x=(px)0 y=(px)7 w=(px)400 layout="column" {{
        rect id="spacer" h=(px)90 fill=(token)"color.k"
        text id="t" font-size=(px)20 fill=(token)"color.k" {text_attrs} {{
          span "{LONG}"
        }}
        rect id="after" h=(px)10 fill=(token)"color.k"
      }}"#
    )
}

#[test]
fn runaround_hug_height_equals_the_rendered_lines() {
    let laid = lay_out(&doc("", &column(r#"text-exclusion="ex""#)));
    assert!(laid.passes >= 2, "settles over passes: {}", laid.passes);
    let n = laid.baselines.len();
    assert!(n >= 4, "{:?}", laid.baselines);
    let line = laid.baselines[1] - laid.baselines[0];
    assert!(
        (laid.text.h - n as f64 * line).abs() < 1e-6,
        "box h {} vs {n} lines of {line}",
        laid.text.h
    );
    assert_eq!(laid.after.y, laid.text.y + laid.text.h);
    assert!(
        !laid.codes.iter().any(|c| c == "layout.conflicting_size"),
        "{:?}",
        laid.codes
    );

    // Without the exclusion the text wraps to fewer lines.
    let plain = lay_out(&doc("", &column("")));
    assert!(
        plain.text.h < laid.text.h,
        "{} vs {}",
        plain.text.h,
        laid.text.h
    );
}

#[test]
fn baseline_grid_hug_height_equals_the_snapped_extent() {
    let grid = 10.0;
    let laid = lay_out(&doc("baseline-grid=(px)10", &column("")));
    let free = lay_out(&doc("", &column("")));
    // Same position either way: the spacer above is fixed.
    assert_eq!(laid.text.y, free.text.y);
    let ascent = free.baselines[0] - free.text.y;
    let n = laid.baselines.len();
    let advance = laid.baselines[1] - laid.baselines[0];
    assert_eq!(advance % grid, 0.0, "snapped advance {advance}");
    let snap = laid.baselines[0] - ascent - laid.text.y;
    let expected = snap + n as f64 * advance;
    assert!(
        (laid.text.h - expected).abs() < 1e-6,
        "box h {} vs snapped extent {expected}",
        laid.text.h
    );
    assert!(
        laid.text.h > free.text.h,
        "{} vs {}",
        laid.text.h,
        free.text.h
    );
    assert_eq!(laid.after.y, laid.text.y + laid.text.h);
}

#[test]
fn position_independent_pages_lower_once() {
    let laid = lay_out(&doc("", &column("")));
    assert_eq!(laid.passes, 1);
}

#[test]
fn long_grid_column_settles_in_two_passes() {
    // A text's snap offset depends on where the texts above it end; the
    // column measures each one at its running position, so one more pass
    // only confirms the positions.
    let texts: String = (0..12)
        .map(|i| {
            format!(
                r#"        text id="t{i}" font-size=(px)13 fill=(token)"color.k" {{
          span "Line {i} wraps once the column narrows enough to break it"
        }}
"#
            )
        })
        .collect();
    let src = doc(
        "baseline-grid=(px)7",
        &format!(
            r#"      frame id="f" x=(px)0 y=(px)3 w=(px)160 layout="column" gap=(px)5 {{
{texts}      }}"#
        ),
    );
    let parsed = parse(&src);
    let fonts = default_provider();
    let prep = DocumentPrep::new(&parsed, None, None);
    let compiler = PageCompiler::new(&prep, &fonts);
    assert_eq!(compiler.layout_stats(0).expect("stats").passes, 2);
    let result = compiler.compile_page(0);
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|d| d.code == "layout.conflicting_size"),
        "{:?}",
        result.diagnostics
    );
    let boxes = compiler.layout_boxes(0).expect("page 0");
    for i in 1..12 {
        let prev = boxes.get(&format!("t{}", i - 1)).expect("prev");
        let next = boxes.get(&format!("t{i}")).expect("next");
        assert_eq!(next.y, prev.y + prev.h + 5.0, "t{i}");
    }
}
