mod common;
use common::parse;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::Paint;
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

#[test]
fn group_live_symmetry_emits_rotated_copies() {
    let src = r##"zenith version=1 {
  project id="proj.sym" name="SYM"
  tokens format="zenith-token-v1" {
token id="color.r" type="color" value="#ff0000"
  }
  styles {}
  document id="doc.sym" title="SYM" {
page id="page.sym" w=(px)100 h=(px)100 {
  group id="group.sym" symmetry-count=4 symmetry-cx=(px)50 symmetry-cy=(px)50 {
    rect id="rect.sym" x=(px)40 y=(px)10 w=(px)20 h=(px)20 fill=(token)"color.r"
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
    let fill_count = cmds
        .iter()
        .filter(|cmd| matches!(cmd, SceneCommand::FillRect { .. }))
        .count();
    let transforms: Vec<_> = cmds
        .iter()
        .filter_map(|cmd| match cmd {
            SceneCommand::PushTransform { angle_deg, cx, cy } => Some((*angle_deg, *cx, *cy)),
            _ => None,
        })
        .collect();

    assert_eq!(fill_count, 4, "seed plus three live copies must render");
    assert_eq!(
        transforms,
        vec![(90.0, 50.0, 50.0), (180.0, 50.0, 50.0), (270.0, 50.0, 50.0)],
        "copies must be transform-bracketed around the requested symmetry center"
    );
}

#[test]
fn group_mirror_symmetry_emits_reflection_matrices() {
    let src = r##"zenith version=1 {
  project id="proj.mir" name="MIR"
  tokens format="zenith-token-v1" {
token id="color.r" type="color" value="#ff0000"
  }
  styles {}
  document id="doc.mir" title="MIR" {
page id="page.mir" w=(px)100 h=(px)100 {
  group id="group.mir" symmetry-count=1 symmetry-mode="mirror" symmetry-cx=(px)50 symmetry-cy=(px)50 symmetry-start-angle=(deg)90 {
    rect id="rect.mir" x=(px)40 y=(px)10 w=(px)20 h=(px)20 fill=(token)"color.r"
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
    let fill_count = cmds
        .iter()
        .filter(|cmd| matches!(cmd, SceneCommand::FillRect { .. }))
        .count();
    let matrices: Vec<_> = cmds
        .iter()
        .filter_map(|cmd| match cmd {
            SceneCommand::PushTransformMatrix { a, b, c, d, e, f } => {
                Some((*a, *b, *c, *d, *e, *f))
            }
            _ => None,
        })
        .collect();

    // count=1 mirror → seed plus one reflected copy.
    assert_eq!(fill_count, 2, "seed plus one mirrored copy must render");
    assert_eq!(matrices.len(), 1, "one reflection matrix must be pushed");
    // Reflection across the vertical axis x=50 negates x about the center:
    // a=-1, d=1, e=2·50=100, others 0.
    let (a, b, c, d, e, f) = matrices[0];
    assert!((a - -1.0).abs() < 1.0e-9, "a={a}");
    assert!(b.abs() < 1.0e-9, "b={b}");
    assert!(c.abs() < 1.0e-9, "c={c}");
    assert!((d - 1.0).abs() < 1.0e-9, "d={d}");
    assert!((e - 100.0).abs() < 1.0e-9, "e={e}");
    assert!(f.abs() < 1.0e-9, "f={f}");
    // The radial-only PushTransform must NOT appear in mirror mode.
    assert!(
        !cmds
            .iter()
            .any(|cmd| matches!(cmd, SceneCommand::PushTransform { .. })),
        "mirror mode must not emit rotational PushTransform copies"
    );
}

#[test]
fn plain_group_does_not_emit_live_symmetry_transforms() {
    let src = r##"zenith version=1 {
  project id="proj.symplain" name="SYMPLAIN"
  tokens format="zenith-token-v1" {
token id="color.r" type="color" value="#ff0000"
  }
  styles {}
  document id="doc.symplain" title="SYMPLAIN" {
page id="page.symplain" w=(px)100 h=(px)100 {
  group id="group.symplain" {
    rect id="rect.symplain" x=(px)40 y=(px)10 w=(px)20 h=(px)20 fill=(token)"color.r"
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
    assert!(
        result
            .scene
            .commands
            .iter()
            .all(|cmd| !matches!(cmd, SceneCommand::PushTransform { .. })),
        "plain group must not emit live-symmetry transforms"
    );
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
