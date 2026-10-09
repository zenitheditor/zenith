mod common;
use common::parse;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::SceneCommand;

// ── Container rotation: GROUP / FRAME ─────────────────────────────────

/// (1) A group with `rotate=(deg)30` and NO w/h, containing two rects,
/// must emit PushTransform (center = union-bbox center of the two rects
/// in device space) before the children, and PopTransform last.
///
/// Group at (0,0), rects at (10,20,100,60) and (50,100,40,30).
/// Union bbox: x=[10,110], y=[20,130] → center = (60, 75).
#[test]
fn group_rotate_no_wh_uses_children_union_bbox_center() {
    let src = r##"zenith version=1 {
  project id="proj.gr1" name="GR1"
  tokens format="zenith-token-v1" {
token id="color.a" type="color" value="#ff0000"
token id="color.b" type="color" value="#0000ff"
  }
  styles {}
  document id="doc.gr1" title="GR1" {
page id="page.gr1" w=(px)300 h=(px)300 {
  group id="grp.rot" rotate=(deg)30 {
    rect id="r1" x=(px)10 y=(px)20 w=(px)100 h=(px)60 fill=(token)"color.a"
    rect id="r2" x=(px)50 y=(px)100 w=(px)40 h=(px)30 fill=(token)"color.b"
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
    // Expected: PushClip(page) PushTransform FillRect FillRect PopTransform PopClip
    assert_eq!(cmds.len(), 6, "expected 6 commands; got: {:?}", cmds);

    assert!(matches!(cmds[0], SceneCommand::PushClip { .. }));

    match &cmds[1] {
        SceneCommand::PushTransform { angle_deg, cx, cy } => {
            assert_eq!(*angle_deg, 30.0, "angle must be 30");
            // Union bbox: x=[10,110] → cx=60, y=[20,130] → cy=75
            assert_eq!(*cx, 60.0, "cx must be (10+110)/2=60");
            assert_eq!(*cy, 75.0, "cy must be (20+130)/2=75");
        }
        other => panic!("expected PushTransform at [1], got {other:?}"),
    }

    assert!(
        matches!(cmds[2], SceneCommand::FillRect { .. }),
        "expected FillRect at [2]"
    );
    assert!(
        matches!(cmds[3], SceneCommand::FillRect { .. }),
        "expected FillRect at [3]"
    );

    assert!(
        matches!(cmds[4], SceneCommand::PopTransform),
        "expected PopTransform at [4], got {:?}",
        cmds[4]
    );
    assert!(matches!(cmds[5], SceneCommand::PopClip));
}

/// (2) A group WITHOUT rotate must emit NO PushTransform — byte-identical
/// to the pre-container-rotation baseline.
#[test]
fn group_without_rotate_emits_no_transform() {
    let src = r##"zenith version=1 {
  project id="proj.gr2" name="GR2"
  tokens format="zenith-token-v1" {
token id="color.a" type="color" value="#00ff00"
  }
  styles {}
  document id="doc.gr2" title="GR2" {
page id="page.gr2" w=(px)200 h=(px)200 {
  group id="grp.norot" {
    rect id="r1" x=(px)10 y=(px)10 w=(px)80 h=(px)80 fill=(token)"color.a"
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
    let has_transform = cmds.iter().any(|c| {
        matches!(
            c,
            SceneCommand::PushTransform { .. } | SceneCommand::PopTransform
        )
    });
    assert!(
        !has_transform,
        "unrotated group must emit no transform commands; got: {:?}",
        cmds
    );
}

/// (3) A group WITH declared w/h + rotate uses the declared box center,
/// not the children bbox.
///
/// Group x=0 y=0 w=200 h=100 → center = (100, 50).
/// The single child rect at (10,10,80,80) would give a different center.
#[test]
fn group_rotate_with_wh_uses_declared_box_center() {
    let src = r##"zenith version=1 {
  project id="proj.gr3" name="GR3"
  tokens format="zenith-token-v1" {
token id="color.a" type="color" value="#aabbcc"
  }
  styles {}
  document id="doc.gr3" title="GR3" {
page id="page.gr3" w=(px)300 h=(px)200 {
  group id="grp.wh" w=(px)200 h=(px)100 rotate=(deg)45 {
    rect id="r1" x=(px)10 y=(px)10 w=(px)80 h=(px)80 fill=(token)"color.a"
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
    // PushClip(page) PushTransform FillRect PopTransform PopClip
    assert_eq!(cmds.len(), 5, "expected 5 commands; got: {:?}", cmds);

    match &cmds[1] {
        SceneCommand::PushTransform { angle_deg, cx, cy } => {
            assert_eq!(*angle_deg, 45.0, "angle must be 45");
            // group x defaults 0, w=200 → cx=0+200/2=100
            // group y defaults 0, h=100 → cy=0+100/2=50
            assert_eq!(*cx, 100.0, "cx must be declared box center 0+200/2=100");
            assert_eq!(*cy, 50.0, "cy must be declared box center 0+100/2=50");
        }
        other => panic!("expected PushTransform at [1], got {other:?}"),
    }

    assert!(matches!(cmds[4], SceneCommand::PopClip));
}

/// (4) A frame with rotate=(deg)20 must emit PushTransform (center from
/// the frame box) BEFORE PushClip, and PopTransform AFTER PopClip.
///
/// Frame x=10 y=20 w=100 h=60 → device-space center = (60, 50).
#[test]
fn frame_rotate_wraps_clip_outermost() {
    let src = r##"zenith version=1 {
  project id="proj.fr1" name="FR1"
  tokens format="zenith-token-v1" {
token id="color.a" type="color" value="#112233"
  }
  styles {}
  document id="doc.fr1" title="FR1" {
page id="page.fr1" w=(px)200 h=(px)200 {
  frame id="frm.rot" x=(px)10 y=(px)20 w=(px)100 h=(px)60 rotate=(deg)20 {
    rect id="r1" x=(px)5 y=(px)5 w=(px)40 h=(px)30 fill=(token)"color.a"
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
    // PushClip(page) PushTransform PushClip(frame) FillRect PopClip PopTransform PopClip(page)
    assert_eq!(cmds.len(), 7, "expected 7 commands; got: {:?}", cmds);

    assert!(
        matches!(cmds[0], SceneCommand::PushClip { .. }),
        "[0] must be page PushClip"
    );

    match &cmds[1] {
        SceneCommand::PushTransform { angle_deg, cx, cy } => {
            assert_eq!(*angle_deg, 20.0, "angle must be 20");
            // ctx.dx=0 + frame_x=10 + frame_w/2=50 → cx=60
            // ctx.dy=0 + frame_y=20 + frame_h/2=30 → cy=50
            assert_eq!(*cx, 60.0, "cx must be 0+10+100/2=60");
            assert_eq!(*cy, 50.0, "cy must be 0+20+60/2=50");
        }
        other => panic!("expected PushTransform at [1], got {other:?}"),
    }

    assert!(
        matches!(cmds[2], SceneCommand::PushClip { .. }),
        "[2] must be frame PushClip (inside transform); got {:?}",
        cmds[2]
    );

    assert!(
        matches!(cmds[3], SceneCommand::FillRect { .. }),
        "[3] must be FillRect"
    );

    assert!(
        matches!(cmds[4], SceneCommand::PopClip),
        "[4] must be frame PopClip; got {:?}",
        cmds[4]
    );

    assert!(
        matches!(cmds[5], SceneCommand::PopTransform),
        "[5] must be PopTransform (after PopClip); got {:?}",
        cmds[5]
    );

    assert!(
        matches!(cmds[6], SceneCommand::PopClip),
        "[6] must be page PopClip"
    );
}

/// (5) A rotated group containing a rotated rect must emit BOTH
/// PushTransform commands nested correctly:
///   PushClip(page) PushTransform(group) PushTransform(rect) FillRect PopTransform(rect) PopTransform(group) PopClip(page)
///
/// Group (no w/h) rotate=15°, contains rect x=10 y=10 w=80 h=40 rotate=45°.
/// Children bbox center = (50, 30) → group PushTransform cx=50, cy=30.
/// Rect center = (10+40, 10+20) = (50, 30) (device space same as group).
#[test]
fn rotated_group_containing_rotated_rect_nests_both_transforms() {
    let src = r##"zenith version=1 {
  project id="proj.gr5" name="GR5"
  tokens format="zenith-token-v1" {
token id="color.a" type="color" value="#ff8800"
  }
  styles {}
  document id="doc.gr5" title="GR5" {
page id="page.gr5" w=(px)200 h=(px)200 {
  group id="grp.outer" rotate=(deg)15 {
    rect id="r1" x=(px)10 y=(px)10 w=(px)80 h=(px)40 fill=(token)"color.a" rotate=(deg)45
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
    // PushClip PushTransform(group) PushTransform(rect) FillRect PopTransform(rect) PopTransform(group) PopClip
    assert_eq!(cmds.len(), 7, "expected 7 commands; got: {:?}", cmds);

    assert!(matches!(cmds[0], SceneCommand::PushClip { .. }));

    // Group PushTransform
    match &cmds[1] {
        SceneCommand::PushTransform { angle_deg, cx, cy } => {
            assert_eq!(*angle_deg, 15.0, "group angle must be 15");
            // Rect bbox: x=10,y=10,w=80,h=40 → center=(50,30)
            assert_eq!(*cx, 50.0, "group pivot cx=10+80/2=50");
            assert_eq!(*cy, 30.0, "group pivot cy=10+40/2=30");
        }
        other => panic!("expected group PushTransform at [1], got {other:?}"),
    }

    // Rect PushTransform
    match &cmds[2] {
        SceneCommand::PushTransform { angle_deg, .. } => {
            assert_eq!(*angle_deg, 45.0, "rect angle must be 45");
        }
        other => panic!("expected rect PushTransform at [2], got {other:?}"),
    }

    assert!(
        matches!(cmds[3], SceneCommand::FillRect { .. }),
        "[3] must be FillRect"
    );
    assert!(
        matches!(cmds[4], SceneCommand::PopTransform),
        "[4] must be rect PopTransform"
    );
    assert!(
        matches!(cmds[5], SceneCommand::PopTransform),
        "[5] must be group PopTransform"
    );
    assert!(matches!(cmds[6], SceneCommand::PopClip));
}
