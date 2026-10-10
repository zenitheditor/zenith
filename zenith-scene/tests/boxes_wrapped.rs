//! Box records of the children of a group or frame with an effect or a
//! mask. The children compile into a scratch stream that is spliced into
//! the page stream. Their records keep every ancestor transform and clip,
//! and `command_index` points into the final page stream.

use std::collections::BTreeMap;

use zenith_core::{KdlAdapter, KdlSource, default_provider};
use zenith_scene::{CompileResult, CompiledBox, DocumentPrep, PageCompiler};

/// A page whose outer frame turns and clips a group, and the group holds a
/// turned, clipping frame. `fx` lands on the group and on the inner frame.
fn source(fx: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.w" name="W"
  tokens format="zenith-token-v1" {{
    token id="c.k" type="color" value="#203040"
    token id="c.sh" type="color" value="#00000066"
    token id="sh" type="shadow" {{
      layer dx=(px)2 dy=(px)4 blur=(px)6 color=(token)"c.sh"
    }}
    token id="mk" type="mask" {{
      rounded radius=12 feather=8
    }}
  }}
  styles {{}}
  document id="doc.w" title="W" {{
    page id="p" w=(px)400 h=(px)300 {{
      rect id="before" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"c.k"
      frame id="outer" x=(px)-40 y=(px)20 w=(px)360 h=(px)300 rotate=(deg)10 {{
        group id="g" x=(px)10 y=(px)10 w=(px)220 h=(px)200 rotate=(deg)14 {fx} {{
          rect id="r" x=(px)-60 y=(px)10 w=(px)90 h=(px)40 rotate=(deg)30 fill=(token)"c.k"
          frame id="f" x=(px)80 y=(px)20 w=(px)100 h=(px)100 rotate=(deg)-20 fill=(token)"c.k" {fx} {{
            rect id="fr" x=(px)-10 y=(px)50 w=(px)80 h=(px)30 rotate=(deg)15 fill=(token)"c.k"
            ellipse id="fe" x=(px)40 y=(px)-5 w=(px)50 h=(px)50 fill=(token)"c.k"
            line id="fl" x1=(px)0 y1=(px)0 x2=(px)90 y2=(px)90 stroke=(token)"c.k"
          }}
        }}
      }}
      rect id="after" x=(px)300 y=(px)200 w=(px)10 h=(px)10 fill=(token)"c.k"
    }}
  }}
}}
"##
    )
}

fn compiled(fx: &str) -> (CompileResult, BTreeMap<String, CompiledBox>) {
    let doc = KdlAdapter
        .parse(source(fx).as_bytes())
        .expect("fixture parses");
    let fonts = default_provider();
    let prep = DocumentPrep::new(&doc, None, None);
    PageCompiler::new(&prep, &fonts).compile_page_with_boxes(0, false)
}

/// Every box under the wrapped group equals the box the same node gets
/// without the effect, and its `command_index` finds the same commands in
/// the final stream.
fn assert_children_match(fx: &str) {
    let (plain, plain_boxes) = compiled("");
    let (wrapped, wrapped_boxes) = compiled(fx);
    // Each id with the length of its own run of commands, which ends
    // before any command the wrapper or the parent adds.
    for (id, len) in [("r", 3), ("f", 1), ("fr", 3), ("fe", 1), ("fl", 1)] {
        let want = plain_boxes.get(id).expect("reference box");
        let got = wrapped_boxes.get(id).expect("wrapped box");
        assert_eq!(got.world, want.world, "{fx}: {id} world");
        assert_eq!(got.clip, want.clip, "{fx}: {id} clip");
        assert_eq!(got.local, want.local, "{fx}: {id} local");
        assert_eq!(got.spin, want.spin, "{fx}: {id} spin");
        assert_eq!(got.rect, want.rect, "{fx}: {id} rect");
        assert_eq!(got.visual, want.visual, "{fx}: {id} visual");
        assert_eq!(got.shape, want.shape, "{fx}: {id} shape");
        assert_eq!(got.paint_order, want.paint_order, "{fx}: {id} rank");
        // The node's own commands sit at `command_index` in both streams.
        let run = |r: &CompileResult, b: &CompiledBox| {
            r.scene
                .commands
                .get(b.command_index..b.command_index + len)
                .map(<[_]>::to_vec)
        };
        let want_run = run(&plain, want).expect("reference commands");
        assert_eq!(run(&wrapped, got), Some(want_run), "{fx}: {id} commands");
    }
    // The children carry the outer frame's turn, the group's turn, and the
    // page clip, the outer frame clip, and (for `f`'s children) `f`'s clip.
    let fr = wrapped_boxes.get("fr").expect("fr");
    assert_ne!(fr.world, zenith_scene::Affine2::IDENTITY, "{fx}");
    assert!(fr.clip.len() >= 3, "{fx}: {:?}", fr.clip);
    // Siblings after the wrapped subtree keep their final indices too.
    let after = wrapped_boxes.get("after").expect("after");
    assert!(matches!(
        wrapped.scene.commands.get(after.command_index),
        Some(zenith_scene::SceneCommand::FillRect { x, .. }) if *x == 300.0
    ));
}

#[test]
fn shadowed_children_match_the_plain_nesting() {
    assert_children_match(r#"shadow=(token)"sh""#);
}

#[test]
fn masked_children_match_the_plain_nesting() {
    assert_children_match(r#"mask=(token)"mk""#);
}

#[test]
fn shadowed_and_masked_children_point_at_the_sharp_copy() {
    assert_children_match(r#"shadow=(token)"sh" mask=(token)"mk""#);
}

#[test]
fn blurred_children_match_the_plain_nesting() {
    assert_children_match("blur=(px)3");
}
