mod common;
use common::*;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::SceneCommand;

// ── Group: children emitted in source order ───────────────────────────

#[test]
fn group_children_emitted_in_order() {
    // A page with a bg rect and a group containing a rect then an ellipse.
    // After PushClip + bg FillRect, the group produces: FillRect, FillEllipse.
    let src = r##"zenith version=1 {
  project id="proj.gc" name="GC"
  tokens format="zenith-token-v1" {
token id="color.bg"   type="color" value="#ffffff"
token id="color.r"    type="color" value="#ff0000"
token id="color.e"    type="color" value="#0000ff"
  }
  styles {}
  document id="doc.gc" title="GC" {
page id="page.gc" w=(px)320 h=(px)200 background=(token)"color.bg" {
  group id="group.gc" {
    rect id="rect.gc" x=(px)10 y=(px)10 w=(px)50 h=(px)50 fill=(token)"color.r"
    ellipse id="ellipse.gc" x=(px)70 y=(px)10 w=(px)50 h=(px)50 fill=(token)"color.e"
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

    let cmds = &result.scene.commands;
    // PushClip, FillRect(bg), FillRect(rect.gc), FillEllipse(ellipse.gc), PopClip
    assert_eq!(cmds.len(), 5, "expected 5 commands; got: {:?}", cmds);
    assert!(matches!(cmds[0], SceneCommand::PushClip { .. }));
    assert!(
        matches!(cmds[1], SceneCommand::FillRect { .. }),
        "cmd[1] must be bg FillRect"
    );
    assert!(
        matches!(cmds[2], SceneCommand::FillRect { .. }),
        "cmd[2] must be group-child FillRect"
    );
    assert!(
        matches!(cmds[3], SceneCommand::FillEllipse { .. }),
        "cmd[3] must be group-child FillEllipse"
    );
    assert!(matches!(cmds[4], SceneCommand::PopClip));
}

// ── Group: visible=false → entire subtree excluded ────────────────────

#[test]
fn invisible_group_subtree_not_emitted() {
    let src = r##"zenith version=1 {
  project id="proj.gv" name="GV"
  tokens format="zenith-token-v1" {
token id="color.r" type="color" value="#ff0000"
token id="color.b" type="color" value="#0000ff"
  }
  styles {}
  document id="doc.gv" title="GV" {
page id="page.gv" w=(px)100 h=(px)100 {
  group id="group.gv" visible=#false {
    rect id="rect.gv1" x=(px)0 y=(px)0 w=(px)50 h=(px)50 fill=(token)"color.r"
    rect id="rect.gv2" x=(px)50 y=(px)50 w=(px)50 h=(px)50 fill=(token)"color.b"
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

    let cmds = &result.scene.commands;
    // Only PushClip + PopClip; both children excluded because group is invisible.
    assert_eq!(
        cmds.len(),
        2,
        "expected PushClip + PopClip only; got: {:?}",
        cmds
    );
    assert!(matches!(cmds[0], SceneCommand::PushClip { .. }));
    assert!(matches!(cmds[1], SceneCommand::PopClip));
}

// ── Group: opacity cascades to child alpha ────────────────────────────

#[test]
fn group_opacity_cascades_to_child() {
    // Group opacity=0.5, child rect fill is fully opaque #ffffff (a=255).
    // Expected child FillRect alpha ≈ 128 (255 * 1.0 * 0.5 = 127.5 → 128).
    let src = r##"zenith version=1 {
  project id="proj.go" name="GO"
  tokens format="zenith-token-v1" {
token id="color.w" type="color" value="#ffffff"
  }
  styles {}
  document id="doc.go" title="GO" {
page id="page.go" w=(px)100 h=(px)100 {
  group id="group.go" opacity=0.5 {
    rect id="rect.go" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.w"
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

    let cmds = &result.scene.commands;
    // PushClip, FillRect, PopClip
    assert_eq!(cmds.len(), 3, "expected 3 commands; got: {:?}", cmds);

    match &cmds[1] {
        SceneCommand::FillRect {
            paint: Paint::Solid { color },
            ..
        } => {
            // 255 * 1.0 (node opacity) * 0.5 (group opacity) = 127.5 → 128.
            assert_eq!(
                color.a, 128,
                "cascaded opacity 0.5 must give a=128; got {}",
                color.a
            );
        }
        other => panic!("expected FillRect, got {other:?}"),
    }
}

// ── Group: x/y translates child geometry ─────────────────────────────

#[test]
fn group_xy_translates_child() {
    // Group x=(px)10 y=(px)20; child rect at x=(px)5 y=(px)5.
    // Expected FillRect at x=15.0 y=25.0.
    let src = r##"zenith version=1 {
  project id="proj.gt" name="GT"
  tokens format="zenith-token-v1" {
token id="color.k" type="color" value="#000000"
  }
  styles {}
  document id="doc.gt" title="GT" {
page id="page.gt" w=(px)200 h=(px)200 {
  group id="group.gt" x=(px)10 y=(px)20 {
    rect id="rect.gt" x=(px)5 y=(px)5 w=(px)50 h=(px)50 fill=(token)"color.k"
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

    let cmds = &result.scene.commands;
    // PushClip, FillRect, PopClip
    assert_eq!(cmds.len(), 3, "expected 3 commands; got: {:?}", cmds);

    match &cmds[1] {
        SceneCommand::FillRect { x, y, .. } => {
            assert_eq!(
                *x, 15.0,
                "child x must be group.x(10) + rect.x(5) = 15; got {x}"
            );
            assert_eq!(
                *y, 25.0,
                "child y must be group.y(20) + rect.y(5) = 25; got {y}"
            );
        }
        other => panic!("expected FillRect, got {other:?}"),
    }
}
