mod common;
use common::*;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::SceneCommand;

// ── Mask ──────────────────────────────────────────────────────────────

/// A rect carrying `mask=(token)` with NO effect must emit a
/// `BeginMask { mask }` … `EndMask` bracket around its fill, with the
/// `MaskSpec` carrying the rect's x/y/w/h and the resolved shape/radius/feather.
#[test]
fn mask_emits_begin_end_bracket() {
    let src = r##"zenith version=1 {
  project id="proj.mk" name="Mk"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#445566"
    token id="mask.m" type="mask" {
      rounded radius=12 feather=8
    }
  }
  styles {}
  document id="doc.mk" title="Mk" {
    page id="page.mk" w=(px)200 h=(px)200 {
      rect id="rect.mk" x=(px)10 y=(px)20 w=(px)80 h=(px)40 fill=(token)"color.fill" mask=(token)"mask.m"
    }
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let cmds = &result.scene.commands;

    // Locate the BeginMask and verify the resolved spec.
    let spec = cmds
        .iter()
        .find_map(|c| match c {
            SceneCommand::BeginMask { mask } => Some(*mask),
            _ => None,
        })
        .expect("a BeginMask must be emitted");
    assert_eq!(spec.shape, zenith_scene::MaskShape::RoundedRect);
    assert_eq!(spec.radius, 12.0);
    assert_eq!(spec.feather, 8.0);
    assert!(!spec.invert);
    assert_eq!((spec.x, spec.y, spec.w, spec.h), (10.0, 20.0, 80.0, 40.0));

    // Bracket is balanced, BeginMask precedes a fill which precedes EndMask.
    let begin_idx = cmds
        .iter()
        .position(|c| matches!(c, SceneCommand::BeginMask { .. }))
        .expect("begin index");
    let end_idx = cmds
        .iter()
        .position(|c| matches!(c, SceneCommand::EndMask))
        .expect("end index");
    assert!(begin_idx < end_idx, "BeginMask must precede EndMask");
    let has_fill_between = cmds
        .get(begin_idx + 1..end_idx)
        .map(|window| {
            window.iter().any(|c| {
                matches!(
                    c,
                    SceneCommand::FillRect { .. } | SceneCommand::FillRoundedRect { .. }
                )
            })
        })
        .unwrap_or(false);
    assert!(
        has_fill_between,
        "the fill must sit inside the mask bracket: {cmds:?}"
    );

    // No effect was set, so no blur/shadow/filter bracket appears.
    assert!(
        !cmds.iter().any(|c| matches!(
            c,
            SceneCommand::BeginBlur { .. }
                | SceneCommand::BeginShadow { .. }
                | SceneCommand::BeginFilter { .. }
        )),
        "no effect bracket should appear for a mask-only rect: {cmds:?}"
    );
}

/// A rect with BOTH `blur` and `mask` must emit the fill TWICE: once bare
/// (sharp base), then inside `BeginMask BeginBlur .. EndBlur EndMask`. The
/// command order is: bare fill, BeginMask, BeginBlur, fill, EndBlur, EndMask.
#[test]
fn mask_and_blur_emits_sharp_base_then_masked_effect() {
    let src = r##"zenith version=1 {
  project id="proj.mb" name="Mb"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#445566"
    token id="mask.m" type="mask" {
      ellipse feather=6
    }
  }
  styles {}
  document id="doc.mb" title="Mb" {
    page id="page.mb" w=(px)200 h=(px)200 {
      rect id="rect.mb" x=(px)10 y=(px)10 w=(px)80 h=(px)60 fill=(token)"color.fill" mask=(token)"mask.m" blur=(px)6
    }
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let cmds = &result.scene.commands;

    // Collect the indices of the relevant commands in order.
    let is_fill = |c: &SceneCommand| matches!(c, SceneCommand::FillRect { .. });
    let fills: Vec<usize> = cmds
        .iter()
        .enumerate()
        .filter(|(_, c)| is_fill(c))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        fills.len(),
        2,
        "fill must be emitted twice (sharp base + masked): {cmds:?}"
    );

    let begin_mask = cmds
        .iter()
        .position(|c| matches!(c, SceneCommand::BeginMask { .. }))
        .expect("BeginMask");
    let begin_blur = cmds
        .iter()
        .position(|c| matches!(c, SceneCommand::BeginBlur { .. }))
        .expect("BeginBlur");
    let end_blur = cmds
        .iter()
        .position(|c| matches!(c, SceneCommand::EndBlur))
        .expect("EndBlur");
    let end_mask = cmds
        .iter()
        .position(|c| matches!(c, SceneCommand::EndMask))
        .expect("EndMask");

    // Order: bare fill < BeginMask < BeginBlur < masked fill < EndBlur < EndMask.
    assert!(fills[0] < begin_mask, "sharp base fill precedes BeginMask");
    assert!(begin_mask < begin_blur, "BeginMask precedes BeginBlur");
    assert!(begin_blur < fills[1], "BeginBlur precedes the masked fill");
    assert!(fills[1] < end_blur, "masked fill precedes EndBlur");
    assert!(end_blur < end_mask, "EndBlur precedes EndMask");
}

/// A rect with `blur` and NO mask must be byte-identical to the pre-mask
/// stream: BeginBlur, fill, EndBlur — and NO BeginMask anywhere.
#[test]
fn blur_without_mask_emits_no_mask_bracket() {
    let src = r##"zenith version=1 {
  project id="proj.bn" name="Bn"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#445566"
  }
  styles {}
  document id="doc.bn" title="Bn" {
    page id="page.bn" w=(px)200 h=(px)200 {
      rect id="rect.bn" x=(px)10 y=(px)10 w=(px)80 h=(px)60 fill=(token)"color.fill" blur=(px)6
    }
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let cmds = &result.scene.commands;

    assert!(
        !cmds
            .iter()
            .any(|c| matches!(c, SceneCommand::BeginMask { .. } | SceneCommand::EndMask)),
        "no mask bracket must appear when mask is absent: {cmds:?}"
    );

    // Exactly one BeginBlur, EndBlur, and a single fill between them.
    let begin = cmds
        .iter()
        .position(|c| matches!(c, SceneCommand::BeginBlur { .. }))
        .expect("BeginBlur");
    let end = cmds
        .iter()
        .position(|c| matches!(c, SceneCommand::EndBlur))
        .expect("EndBlur");
    assert!(begin < end);
    let fills_between = cmds
        .get(begin + 1..end)
        .map(|w| {
            w.iter()
                .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
                .count()
        })
        .unwrap_or(0);
    assert_eq!(
        fills_between, 1,
        "exactly one fill inside the blur bracket: {cmds:?}"
    );
}
