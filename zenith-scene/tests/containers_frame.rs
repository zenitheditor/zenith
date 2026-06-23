mod common;
use common::*;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::SceneCommand;

// ── Frame: PushClip → children → PopClip ─────────────────────────────

#[test]
fn frame_emits_pushclip_children_popclip() {
    let src = r##"zenith version=1 {
  project id="proj.f1" name="F1"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#3b82f6"
  }
  styles {}
  document id="doc.f1" title="F1" {
page id="page.f1" w=(px)320 h=(px)200 {
  frame id="frame.clip" x=(px)40 y=(px)40 w=(px)120 h=(px)100 {
    rect id="rect.inner" x=(px)50 y=(px)50 w=(px)60 h=(px)60 fill=(token)"color.fill"
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
    // Page PushClip, Frame PushClip, FillRect(child), Frame PopClip, Page PopClip
    assert_eq!(cmds.len(), 5, "expected 5 commands; got: {:?}", cmds);

    // Page clip
    assert!(
        matches!(cmds[0], SceneCommand::PushClip { x, y, w, h } if x == 0.0 && y == 0.0 && w == 320.0 && h == 200.0),
        "cmd[0] must be page PushClip"
    );
    // Frame clip — the frame's own bbox
    assert!(
        matches!(cmds[1], SceneCommand::PushClip { x, y, w, h } if x == 40.0 && y == 40.0 && w == 120.0 && h == 100.0),
        "cmd[1] must be frame PushClip at (40,40,120,100); got: {:?}",
        cmds[1]
    );
    // Child FillRect
    assert!(
        matches!(cmds[2], SceneCommand::FillRect { .. }),
        "cmd[2] must be child FillRect"
    );
    // Frame PopClip
    assert!(
        matches!(cmds[3], SceneCommand::PopClip),
        "cmd[3] must be frame PopClip"
    );
    // Page PopClip
    assert!(
        matches!(cmds[4], SceneCommand::PopClip),
        "cmd[4] must be page PopClip"
    );
}

// ── Frame: child overflow still emitted (renderer clips, not compiler) ─

#[test]
fn frame_child_overflow_still_emitted() {
    // Child rect extends well beyond the frame bounds — compiler must emit
    // its full FillRect unchanged; clipping is the renderer's job.
    let src = r##"zenith version=1 {
  project id="proj.f2" name="F2"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#f97316"
  }
  styles {}
  document id="doc.f2" title="F2" {
page id="page.f2" w=(px)320 h=(px)200 {
  frame id="frame.clip" x=(px)40 y=(px)40 w=(px)120 h=(px)100 {
    rect id="rect.overflow" x=(px)100 y=(px)30 w=(px)100 h=(px)120 fill=(token)"color.fill"
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
    // Ensure child FillRect is present with its full (unclipped) geometry.
    let fill_rects_vec: Vec<_> = cmds
        .iter()
        .filter_map(|c| {
            if let SceneCommand::FillRect { x, y, w, h, .. } = c {
                Some((*x, *y, *w, *h))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(fill_rects_vec.len(), 1, "expected exactly one FillRect");
    let (rx, ry, rw, rh) = fill_rects_vec[0];
    assert_eq!(
        rx, 100.0,
        "child FillRect x must be 100 (absolute, unclipped)"
    );
    assert_eq!(ry, 30.0, "child FillRect y must be 30");
    assert_eq!(rw, 100.0, "child FillRect w must be 100");
    assert_eq!(rh, 120.0, "child FillRect h must be 120");
}

// ── Frame: missing geometry → advisory, no PushClip ───────────────────

#[test]
fn frame_missing_geometry_skipped() {
    // Frame with x=None; compile must push a scene.missing_geometry advisory
    // and emit NO PushClip (so push/pop balance is preserved).
    let src = r##"zenith version=1 {
  project id="proj.f3" name="F3"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.f3" title="F3" {
page id="page.f3" w=(px)100 h=(px)100 {
  frame id="frame.nogeo" y=(px)0 w=(px)100 h=(px)100 {
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let missing: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "scene.missing_geometry")
        .collect();
    assert_eq!(
        missing.len(),
        1,
        "expected 1 scene.missing_geometry advisory; got: {:?}",
        result.diagnostics
    );

    // Push/pop must still be balanced: only page PushClip + PopClip.
    let push_count = result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::PushClip { .. }))
        .count();
    let pop_count = result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::PopClip))
        .count();
    assert_eq!(push_count, pop_count, "PushClip/PopClip must be balanced");
    assert_eq!(push_count, 1, "only the page PushClip must be present");
}

// ── Frame: visible=false → entire subtree excluded ────────────────────

#[test]
fn invisible_frame_subtree_not_emitted() {
    let src = r##"zenith version=1 {
  project id="proj.f4" name="F4"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#3b82f6"
  }
  styles {}
  document id="doc.f4" title="F4" {
page id="page.f4" w=(px)100 h=(px)100 {
  frame id="frame.hidden" x=(px)0 y=(px)0 w=(px)100 h=(px)100 visible=#false {
    rect id="rect.inner" x=(px)0 y=(px)0 w=(px)50 h=(px)50 fill=(token)"color.fill"
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
    // Only page PushClip + PopClip; no frame PushClip, no FillRect.
    assert_eq!(
        cmds.len(),
        2,
        "expected PushClip + PopClip only; got: {:?}",
        cmds
    );
    assert!(matches!(cmds[0], SceneCommand::PushClip { .. }));
    assert!(matches!(cmds[1], SceneCommand::PopClip));
}

// ── Frame: opacity cascades to child alpha ─────────────────────────────

#[test]
fn frame_opacity_cascades_to_child() {
    // Frame opacity=0.5, child rect fill fully opaque #ffffff (a=255).
    // Expected child FillRect alpha ≈ 128 (255 * 1.0 * 0.5 = 127.5 → 128).
    let src = r##"zenith version=1 {
  project id="proj.f5" name="F5"
  tokens format="zenith-token-v1" {
token id="color.w" type="color" value="#ffffff"
  }
  styles {}
  document id="doc.f5" title="F5" {
page id="page.f5" w=(px)100 h=(px)100 {
  frame id="frame.opaque" x=(px)0 y=(px)0 w=(px)100 h=(px)100 opacity=0.5 {
    rect id="rect.inner" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.w"
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

    let fill_rect = result
        .scene
        .commands
        .iter()
        .find(|c| matches!(c, SceneCommand::FillRect { .. }));
    match fill_rect {
        Some(SceneCommand::FillRect {
            paint: Paint::Solid { color },
            ..
        }) => {
            // 255 * 1.0 (node opacity) * 0.5 (frame opacity) = 127.5 → 128.
            assert_eq!(
                color.a, 128,
                "cascaded opacity 0.5 must give a=128; got {}",
                color.a
            );
        }
        _ => panic!("expected a FillRect command"),
    }
}

// ── Frame: does NOT translate children (clip-only) ─────────────────────

#[test]
fn frame_does_not_translate_child() {
    // Frame at x=(px)40 y=(px)40; child rect at x=(px)50 y=(px)50.
    // Because frame is clip-only (no translation), the child FillRect must
    // be at x=50.0 y=50.0, NOT 90.0/90.0.
    let src = r##"zenith version=1 {
  project id="proj.f6" name="F6"
  tokens format="zenith-token-v1" {
token id="color.k" type="color" value="#000000"
  }
  styles {}
  document id="doc.f6" title="F6" {
page id="page.f6" w=(px)200 h=(px)200 {
  frame id="frame.noxlate" x=(px)40 y=(px)40 w=(px)120 h=(px)120 {
    rect id="rect.abs" x=(px)50 y=(px)50 w=(px)50 h=(px)50 fill=(token)"color.k"
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

    let fill_rect = result
        .scene
        .commands
        .iter()
        .find(|c| matches!(c, SceneCommand::FillRect { .. }));
    match fill_rect {
        Some(SceneCommand::FillRect { x, y, .. }) => {
            assert_eq!(
                *x, 50.0,
                "child x must be 50 (absolute, frame does not translate); got {x}"
            );
            assert_eq!(
                *y, 50.0,
                "child y must be 50 (absolute, frame does not translate); got {y}"
            );
        }
        _ => panic!("expected a FillRect command"),
    }
}
