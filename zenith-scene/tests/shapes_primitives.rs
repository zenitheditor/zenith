mod common;
use common::*;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::{Paint, SceneCommand};

// ── Minimal single-rect document ──────────────────────────────────────

/// A page with a single full-page rect filled via a token color.
/// Expected scene: PushClip → FillRect (bg from token) → FillRect (rect) → PopClip.
/// In this test the page has no background, so background FillRect is absent.
#[test]
fn single_rect_token_fill_compiles_correctly() {
    let src = r##"zenith version=1 {
  project id="proj.t1" name="T1"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#f8fafc"
  }
  styles {}
  document id="doc.t1" title="T1" {
page id="page.t1" w=(px)640 h=(px)360 {
  rect id="rect.t1" x=(px)0 y=(px)0 w=(px)640 h=(px)360 fill=(token)"color.fill"
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
    assert_eq!(cmds.len(), 3, "expected 3 commands, got: {:?}", cmds);

    assert!(
        matches!(cmds[0], SceneCommand::PushClip { x, y, w, h } if x == 0.0 && y == 0.0 && w == 640.0 && h == 360.0),
        "first command must be PushClip covering the page"
    );

    match &cmds[1] {
        SceneCommand::FillRect {
            x,
            y,
            w,
            h,
            paint: Paint::Solid { color },
        } => {
            assert_eq!(*x, 0.0);
            assert_eq!(*y, 0.0);
            assert_eq!(*w, 640.0);
            assert_eq!(*h, 360.0);
            // #f8fafc → r=0xf8=248, g=0xfa=250, b=0xfc=252, a=255
            assert_eq!(color.r, 0xf8);
            assert_eq!(color.g, 0xfa);
            assert_eq!(color.b, 0xfc);
            assert_eq!(color.a, 255);
        }
        other => panic!("expected FillRect, got {other:?}"),
    }

    assert!(
        matches!(cmds[2], SceneCommand::PopClip),
        "last command must be PopClip"
    );
}

// ── Two rects → two FillRects in source order ─────────────────────────

#[test]
fn two_rects_emitted_in_source_order() {
    let src = r##"zenith version=1 {
  project id="proj.t2" name="T2"
  tokens format="zenith-token-v1" {
token id="color.a" type="color" value="#111111"
token id="color.b" type="color" value="#222222"
  }
  styles {}
  document id="doc.t2" title="T2" {
page id="page.t2" w=(px)100 h=(px)100 {
  rect id="rect.a" x=(px)0 y=(px)0 w=(px)50 h=(px)50 fill=(token)"color.a"
  rect id="rect.b" x=(px)50 y=(px)50 w=(px)50 h=(px)50 fill=(token)"color.b"
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
    // PushClip, FillRect(a), FillRect(b), PopClip
    assert_eq!(cmds.len(), 4, "expected 4 commands, got: {:?}", cmds);

    match &cmds[1] {
        SceneCommand::FillRect {
            paint: Paint::Solid { color },
            ..
        } => assert_eq!(color.r, 0x11),
        other => panic!("expected FillRect for rect.a, got {other:?}"),
    }
    match &cmds[2] {
        SceneCommand::FillRect {
            paint: Paint::Solid { color },
            ..
        } => assert_eq!(color.r, 0x22),
        other => panic!("expected FillRect for rect.b, got {other:?}"),
    }
}

// ── visible=false rect is not emitted ─────────────────────────────────

#[test]
fn invisible_rect_not_emitted() {
    let src = r##"zenith version=1 {
  project id="proj.t3" name="T3"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#abcdef"
  }
  styles {}
  document id="doc.t3" title="T3" {
page id="page.t3" w=(px)100 h=(px)100 {
  rect id="rect.hidden" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.fill" visible=#false
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    // No diagnostics expected (visible=false is a normal skip, not an error).
    assert!(
        result.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );

    let cmds = &result.scene.commands;
    // Only PushClip + PopClip; no FillRect.
    assert_eq!(
        cmds.len(),
        2,
        "expected PushClip + PopClip only; got: {:?}",
        cmds
    );
    assert!(matches!(cmds[0], SceneCommand::PushClip { .. }));
    assert!(matches!(cmds[1], SceneCommand::PopClip));
}

// ── Page background emitted as first FillRect ─────────────────────────

#[test]
fn page_background_emitted_before_children() {
    let src = r##"zenith version=1 {
  project id="proj.t7" name="T7"
  tokens format="zenith-token-v1" {
token id="color.bg" type="color" value="#ffffff"
token id="color.fill" type="color" value="#000000"
  }
  styles {}
  document id="doc.t7" title="T7" {
page id="page.t7" w=(px)100 h=(px)100 background=(token)"color.bg" {
  rect id="rect.t7" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.fill"
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
    // PushClip, FillRect(bg=white), FillRect(rect=black), PopClip
    assert_eq!(cmds.len(), 4, "expected 4 commands; got: {:?}", cmds);

    // Background fill must be white.
    match &cmds[1] {
        SceneCommand::FillRect {
            paint: Paint::Solid { color },
            ..
        } => {
            assert_eq!(color.r, 255, "bg must be white");
            assert_eq!(color.g, 255);
            assert_eq!(color.b, 255);
        }
        other => panic!("expected background FillRect, got {other:?}"),
    }

    // Child rect must be black.
    match &cmds[2] {
        SceneCommand::FillRect {
            paint: Paint::Solid { color },
            ..
        } => {
            assert_eq!(color.r, 0, "child rect must be black");
            assert_eq!(color.g, 0);
            assert_eq!(color.b, 0);
        }
        other => panic!("expected child FillRect, got {other:?}"),
    }
}

// ── Opacity multiplied into alpha ─────────────────────────────────────

#[test]
fn opacity_applied_to_fill_alpha() {
    // A full-alpha color (#ffffff, a=255) with opacity=0.5 → a≈128.
    let src = r##"zenith version=1 {
  project id="proj.t8" name="T8"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#ffffff"
  }
  styles {}
  document id="doc.t8" title="T8" {
page id="page.t8" w=(px)100 h=(px)100 {
  rect id="rect.t8" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.fill" opacity=0.5
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

    match &result.scene.commands[1] {
        SceneCommand::FillRect {
            paint: Paint::Solid { color },
            ..
        } => {
            // 255 * 0.5 = 127.5 → rounds to 128.
            assert_eq!(color.a, 128, "opacity 0.5 must give a=128; got {}", color.a);
        }
        other => panic!("expected FillRect, got {other:?}"),
    }
}

// ── Rect then text → FillRect before DrawGlyphRun (z-order) ──────────

#[test]
fn rect_then_text_z_order_preserved() {
    let src = r##"zenith version=1 {
  project id="proj.tx2" name="TX2"
  tokens format="zenith-token-v1" {
token id="color.bg"  type="color"      value="#ffffff"
token id="color.ink" type="color"      value="#000000"
token id="font.body" type="fontFamily" value="Noto Sans"
token id="size.body" type="dimension"  value=(px)16
  }
  styles {}
  document id="doc.tx2" title="TX2" {
page id="page.tx2" w=(px)400 h=(px)200 {
  rect id="bg.rect" x=(px)0 y=(px)0 w=(px)400 h=(px)200 fill=(token)"color.bg"
  text id="label.tx2" x=(px)10 y=(px)20 w=(px)380 h=(px)40 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
    span "Hello"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let cmds = &result.scene.commands;
    // PushClip, FillRect, DrawGlyphRun, PopClip
    assert_eq!(cmds.len(), 4, "expected 4 commands; got: {:?}", cmds);
    assert!(
        matches!(cmds[1], SceneCommand::FillRect { .. }),
        "second command must be FillRect (rect comes first)"
    );
    assert!(
        matches!(cmds[2], SceneCommand::DrawGlyphRun { .. }),
        "third command must be DrawGlyphRun (text comes after rect)"
    );
}

// ── role="guide" nodes are excluded from render output ──────────────────

#[test]
fn guide_role_nodes_are_not_rendered() {
    let src = r##"zenith version=1 {
  project id="proj.g" name="G"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#000000"
  }
  styles {}
  document id="doc.g" title="G" {
page id="page.g" w=(px)100 h=(px)100 {
  rect id="rect.real" x=(px)0 y=(px)0 w=(px)40 h=(px)40 fill=(token)"color.fill"
  rect id="rect.guide" role="guide" x=(px)50 y=(px)50 w=(px)40 h=(px)40 fill=(token)"color.fill"
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    // Exactly one FillRect for the real rect; the guide rect emits nothing.
    // (No page background, so no background FillRect.)
    let fills = result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
        .count();
    assert_eq!(
        fills, 1,
        "guide-role rect must not render; expected 1 FillRect, got {fills}: {:?}",
        result.scene.commands
    );
}

// ── Ellipse: token fill compiles to FillEllipse ───────────────────────

#[test]
fn single_ellipse_token_fill_compiles_correctly() {
    let src = r##"zenith version=1 {
  project id="proj.e1" name="E1"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#f8fafc"
  }
  styles {}
  document id="doc.e1" title="E1" {
page id="page.e1" w=(px)640 h=(px)360 {
  ellipse id="ellipse.e1" x=(px)0 y=(px)0 w=(px)640 h=(px)360 fill=(token)"color.fill"
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
    // PushClip, FillEllipse, PopClip
    assert_eq!(cmds.len(), 3, "expected 3 commands, got: {:?}", cmds);

    assert!(
        matches!(cmds[0], SceneCommand::PushClip { x, y, w, h } if x == 0.0 && y == 0.0 && w == 640.0 && h == 360.0),
        "first command must be PushClip covering the page"
    );

    match &cmds[1] {
        SceneCommand::FillEllipse {
            x,
            y,
            w,
            h,
            rx,
            ry,
            paint: Paint::Solid { color },
        } => {
            assert_eq!(*x, 0.0);
            assert_eq!(*y, 0.0);
            assert_eq!(*w, 640.0);
            assert_eq!(*h, 360.0);
            assert!(rx.is_none(), "expected rx=None for inscribed ellipse");
            assert!(ry.is_none(), "expected ry=None for inscribed ellipse");
            // #f8fafc → r=0xf8=248, g=0xfa=250, b=0xfc=252, a=255
            assert_eq!(color.r, 0xf8);
            assert_eq!(color.g, 0xfa);
            assert_eq!(color.b, 0xfc);
            assert_eq!(color.a, 255);
        }
        other => panic!("expected FillEllipse, got {other:?}"),
    }

    assert!(
        matches!(cmds[2], SceneCommand::PopClip),
        "last command must be PopClip"
    );
}

// ── Ellipse: visible=false not emitted ────────────────────────────────

#[test]
fn invisible_ellipse_not_emitted() {
    let src = r##"zenith version=1 {
  project id="proj.e2" name="E2"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#abcdef"
  }
  styles {}
  document id="doc.e2" title="E2" {
page id="page.e2" w=(px)100 h=(px)100 {
  ellipse id="ellipse.hidden" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.fill" visible=#false
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
    // Only PushClip + PopClip; no FillEllipse.
    assert_eq!(
        cmds.len(),
        2,
        "expected PushClip + PopClip only; got: {:?}",
        cmds
    );
    assert!(matches!(cmds[0], SceneCommand::PushClip { .. }));
    assert!(matches!(cmds[1], SceneCommand::PopClip));
}

// ── Ellipse: fill + stroke tokens compile to FillEllipse then StrokeEllipse

#[test]
fn ellipse_fill_and_stroke_tokens_emit_fill_then_stroke() {
    let src = r##"zenith version=1 {
  project id="proj.e3" name="E3"
  tokens format="zenith-token-v1" {
token id="color.fill"   type="color"     value="#1e293b"
token id="color.stroke" type="color"     value="#94a3b8"
token id="size.sw"      type="dimension" value=(px)4
  }
  styles {}
  document id="doc.e3" title="E3" {
page id="page.e3" w=(px)200 h=(px)200 {
  ellipse id="ellipse.e3" x=(px)10 y=(px)10 w=(px)180 h=(px)180 fill=(token)"color.fill" stroke=(token)"color.stroke" stroke-width=(token)"size.sw"
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
    // PushClip, FillEllipse, StrokeEllipse, PopClip
    assert_eq!(cmds.len(), 4, "expected 4 commands, got: {:?}", cmds);

    assert!(
        matches!(cmds[0], SceneCommand::PushClip { .. }),
        "first command must be PushClip"
    );

    match &cmds[1] {
        SceneCommand::FillEllipse {
            x,
            y,
            w,
            h,
            rx,
            ry,
            paint: Paint::Solid { color },
        } => {
            assert_eq!(*x, 10.0);
            assert_eq!(*y, 10.0);
            assert_eq!(*w, 180.0);
            assert_eq!(*h, 180.0);
            assert!(rx.is_none(), "expected rx=None for inscribed ellipse");
            assert!(ry.is_none(), "expected ry=None for inscribed ellipse");
            // #1e293b → r=0x1e=30, g=0x29=41, b=0x3b=59, a=255
            assert_eq!(color.r, 0x1e);
            assert_eq!(color.g, 0x29);
            assert_eq!(color.b, 0x3b);
            assert_eq!(color.a, 255);
        }
        other => panic!("expected FillEllipse at index 1, got {other:?}"),
    }

    match &cmds[2] {
        SceneCommand::StrokeEllipse {
            x,
            y,
            w,
            h,
            color,
            stroke_width,
            ..
        } => {
            assert_eq!(*x, 10.0);
            assert_eq!(*y, 10.0);
            assert_eq!(*w, 180.0);
            assert_eq!(*h, 180.0);
            // #94a3b8 → r=0x94=148, g=0xa3=163, b=0xb8=184, a=255
            assert_eq!(color.r, 0x94);
            assert_eq!(color.g, 0xa3);
            assert_eq!(color.b, 0xb8);
            assert_eq!(color.a, 255);
            assert_eq!(*stroke_width, 4.0);
        }
        other => panic!("expected StrokeEllipse at index 2, got {other:?}"),
    }

    assert!(
        matches!(cmds[3], SceneCommand::PopClip),
        "last command must be PopClip"
    );
}

// ── Ellipse: stroke only (no fill) compiles to StrokeEllipse only ─────

#[test]
fn ellipse_stroke_only_emits_stroke_ellipse_without_fill() {
    let src = r##"zenith version=1 {
  project id="proj.e4" name="E4"
  tokens format="zenith-token-v1" {
token id="color.stroke" type="color"     value="#f43f5e"
token id="size.sw"      type="dimension" value=(px)3
  }
  styles {}
  document id="doc.e4" title="E4" {
page id="page.e4" w=(px)100 h=(px)100 {
  ellipse id="ellipse.e4" x=(px)5 y=(px)5 w=(px)90 h=(px)90 stroke=(token)"color.stroke" stroke-width=(token)"size.sw"
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
    // PushClip, StrokeEllipse, PopClip — no FillEllipse
    assert_eq!(
        cmds.len(),
        3,
        "expected 3 commands (no fill), got: {:?}",
        cmds
    );

    assert!(
        matches!(cmds[0], SceneCommand::PushClip { .. }),
        "first command must be PushClip"
    );

    match &cmds[1] {
        SceneCommand::StrokeEllipse {
            x,
            y,
            w,
            h,
            color,
            stroke_width,
            ..
        } => {
            assert_eq!(*x, 5.0);
            assert_eq!(*y, 5.0);
            assert_eq!(*w, 90.0);
            assert_eq!(*h, 90.0);
            // #f43f5e → r=0xf4=244, g=0x3f=63, b=0x5e=94, a=255
            assert_eq!(color.r, 0xf4);
            assert_eq!(color.g, 0x3f);
            assert_eq!(color.b, 0x5e);
            assert_eq!(color.a, 255);
            assert_eq!(*stroke_width, 3.0);
        }
        other => panic!("expected StrokeEllipse at index 1, got {other:?}"),
    }

    assert!(
        matches!(cmds[2], SceneCommand::PopClip),
        "last command must be PopClip"
    );
}

// ── Line: token stroke compiles to StrokeLine ─────────────────────────

#[test]
fn single_line_token_stroke_compiles_correctly() {
    let src = r##"zenith version=1 {
  project id="proj.l1" name="L1"
  tokens format="zenith-token-v1" {
token id="color.rule" type="color" value="#94a3b8"
token id="size.stroke" type="dimension" value=(px)2
  }
  styles {}
  document id="doc.l1" title="L1" {
page id="page.l1" w=(px)320 h=(px)200 {
  line id="line.divider" x1=(px)40 y1=(px)100 x2=(px)280 y2=(px)100 stroke=(token)"color.rule" stroke-width=(token)"size.stroke"
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
    // PushClip, StrokeLine, PopClip
    assert_eq!(cmds.len(), 3, "expected 3 commands, got: {:?}", cmds);

    assert!(
        matches!(cmds[0], SceneCommand::PushClip { .. }),
        "first command must be PushClip"
    );

    match &cmds[1] {
        SceneCommand::StrokeLine {
            x1,
            y1,
            x2,
            y2,
            color,
            stroke_width,
            ..
        } => {
            assert_eq!(*x1, 40.0);
            assert_eq!(*y1, 100.0);
            assert_eq!(*x2, 280.0);
            assert_eq!(*y2, 100.0);
            // #94a3b8 → r=0x94=148, g=0xa3=163, b=0xb8=184
            assert_eq!(color.r, 0x94);
            assert_eq!(color.g, 0xa3);
            assert_eq!(color.b, 0xb8);
            assert_eq!(color.a, 255);
            // size.stroke = (px)2
            assert_eq!(*stroke_width, 2.0);
        }
        other => panic!("expected StrokeLine, got {other:?}"),
    }

    assert!(
        matches!(cmds[2], SceneCommand::PopClip),
        "last command must be PopClip"
    );
}

// ── Line: visible=false not emitted ──────────────────────────────────

#[test]
fn invisible_line_not_emitted() {
    let src = r##"zenith version=1 {
  project id="proj.l2" name="L2"
  tokens format="zenith-token-v1" {
token id="color.rule" type="color" value="#94a3b8"
  }
  styles {}
  document id="doc.l2" title="L2" {
page id="page.l2" w=(px)100 h=(px)100 {
  line id="line.hidden" x1=(px)0 y1=(px)50 x2=(px)100 y2=(px)50 stroke=(token)"color.rule" visible=#false
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
    // Only PushClip + PopClip; no StrokeLine.
    assert_eq!(
        cmds.len(),
        2,
        "expected PushClip + PopClip only; got: {:?}",
        cmds
    );
    assert!(matches!(cmds[0], SceneCommand::PushClip { .. }));
    assert!(matches!(cmds[1], SceneCommand::PopClip));
}
