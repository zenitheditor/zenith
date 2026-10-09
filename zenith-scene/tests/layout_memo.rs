//! The per-lowering measure memo: nested hugging frames measure each node
//! a bounded number of times, and the result is deterministic.

mod common;
use common::parse;
use zenith_core::default_provider;
use zenith_scene::{DocumentPrep, PageCompiler};

/// `depth` levels of hugging row / column frames, two per level, with
/// `leaves` texts in each innermost frame. Returns the source and the text
/// count.
fn nested(depth: usize, leaves: usize) -> (String, usize) {
    fn frame(level: usize, depth: usize, leaves: usize, next: &mut usize, out: &mut String) {
        let mode = if level.is_multiple_of(2) {
            "row"
        } else {
            "column"
        };
        *next += 1;
        let origin = if level == 0 {
            r#" x=(px)10 y=(px)10"#
        } else {
            ""
        };
        out.push_str(&format!(
            "frame id=\"f{next}\" layout=\"{mode}\" gap=(px)4 padding=(px)2{origin} {{\n"
        ));
        if level + 1 == depth {
            for _ in 0..leaves {
                *next += 1;
                out.push_str(&format!(
                    "text id=\"t{next}\" font-size=(px)8 fill=(token)\"color.k\" {{ span \"Leaf {next} words\" }}\n"
                ));
            }
        } else {
            frame(level + 1, depth, leaves, next, out);
            frame(level + 1, depth, leaves, next, out);
        }
        out.push_str("}\n");
    }
    let mut body = String::new();
    let mut next = 0;
    frame(0, depth, leaves, &mut next, &mut body);
    let texts = (1 << (depth - 1)) * leaves;
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.lm" name="LM"
  tokens format="zenith-token-v1" {{
    token id="color.k" type="color" value="#000000"
  }}
  styles {{}}
  document id="doc.lm" title="LM" {{
    page id="p" w=(px)4000 h=(px)3000 {{
{body}
    }}
  }}
}}
"##
    );
    (src, texts)
}

#[test]
fn nested_hug_chain_probes_each_text_once() {
    let (src, texts) = nested(6, 4);
    let parsed = parse(&src);
    let fonts = default_provider();
    let prep = DocumentPrep::new(&parsed, None, None);
    let compiler = PageCompiler::new(&prep, &fonts);
    let stats = compiler.layout_stats(0).expect("stats");
    assert_eq!(stats.passes, 1);
    // One scratch compile per text height; each text is measured at one
    // width (its natural width) however deep it nests.
    assert_eq!(stats.probes, texts, "{stats:?}");
}

#[test]
fn memoized_lowering_is_deterministic() {
    let (src, _) = nested(5, 3);
    let parsed = parse(&src);
    let fonts = default_provider();
    let run = || {
        let prep = DocumentPrep::new(&parsed, None, None);
        let compiler = PageCompiler::new(&prep, &fonts);
        let boxes = compiler.layout_boxes(0).expect("page 0").clone();
        let result = compiler.compile_page(0);
        (
            boxes,
            result.scene.to_json().expect("json"),
            result.diagnostics,
        )
    };
    let one = run();
    let two = run();
    assert_eq!(one.0, two.0);
    assert_eq!(one.1, two.1);
    assert_eq!(one.2, two.2);
}
