mod common;
use common::*;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::{Paint, SceneCommand};

// ── rect: fill only → FillRect (regression) ──────────────────────────

#[test]
fn rect_fill_only_emits_fill_rect() {
    let src = r##"zenith version=1 {
  project id="proj.rf" name="RF"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#112233"
  }
  styles {}
  document id="doc.rf" title="RF" {
page id="page.rf" w=(px)100 h=(px)100 {
  rect id="rect.rf" x=(px)10 y=(px)10 w=(px)40 h=(px)40 fill=(token)"color.fill"
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
    assert!(
        matches!(cmds[1], SceneCommand::FillRect { .. }),
        "expected a single FillRect; got {:?}",
        cmds[1]
    );
}

// ── rect: fill + stroke → FillRect then StrokeRect ───────────────────

#[test]
fn rect_fill_and_stroke_emits_fill_then_stroke() {
    let src = r##"zenith version=1 {
  project id="proj.rfs" name="RFS"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#112233"
token id="color.stroke" type="color" value="#445566"
token id="size.sw" type="dimension" value=(px)4
  }
  styles {}
  document id="doc.rfs" title="RFS" {
page id="page.rfs" w=(px)100 h=(px)100 {
  rect id="rect.rfs" x=(px)10 y=(px)10 w=(px)40 h=(px)40 fill=(token)"color.fill" stroke=(token)"color.stroke" stroke-width=(token)"size.sw"
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
    // PushClip, FillRect, StrokeRect, PopClip
    assert_eq!(cmds.len(), 4, "expected 4 commands; got: {:?}", cmds);
    match &cmds[1] {
        SceneCommand::FillRect {
            paint: Paint::Solid { color },
            ..
        } => assert_eq!(color.r, 0x11),
        other => panic!("expected FillRect first, got {other:?}"),
    }
    match &cmds[2] {
        SceneCommand::StrokeRect {
            color,
            stroke_width,
            ..
        } => {
            assert_eq!(color.r, 0x44, "stroke color r must be 0x44");
            assert!(
                (*stroke_width - 4.0).abs() < 0.01,
                "stroke-width must be 4px"
            );
        }
        other => panic!("expected StrokeRect on top, got {other:?}"),
    }
}

// ── rect: fill + radius → FillRoundedRect ────────────────────────────

#[test]
fn rect_fill_with_radius_emits_fill_rounded_rect() {
    let src = r##"zenith version=1 {
  project id="proj.rfr" name="RFR"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#112233"
token id="size.r" type="dimension" value=(px)8
  }
  styles {}
  document id="doc.rfr" title="RFR" {
page id="page.rfr" w=(px)100 h=(px)100 {
  rect id="rect.rfr" x=(px)10 y=(px)10 w=(px)40 h=(px)40 fill=(token)"color.fill" radius=(token)"size.r"
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
    // PushClip, FillRoundedRect, PopClip
    assert_eq!(cmds.len(), 3, "expected 3 commands; got: {:?}", cmds);
    match &cmds[1] {
        SceneCommand::FillRoundedRect {
            radius,
            paint: Paint::Solid { color },
            ..
        } => {
            assert_eq!(color.r, 0x11);
            assert!((*radius - 8.0).abs() < 0.01, "radius must be 8px");
        }
        other => panic!("expected FillRoundedRect, got {other:?}"),
    }
}

// ── rect: fill + stroke + radius → FillRoundedRect then StrokeRoundedRect

#[test]
fn rect_fill_stroke_radius_emits_rounded_fill_then_rounded_stroke() {
    let src = r##"zenith version=1 {
  project id="proj.rfsr" name="RFSR"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#112233"
token id="color.stroke" type="color" value="#445566"
token id="size.sw" type="dimension" value=(px)4
token id="size.r" type="dimension" value=(px)8
  }
  styles {}
  document id="doc.rfsr" title="RFSR" {
page id="page.rfsr" w=(px)100 h=(px)100 {
  rect id="rect.rfsr" x=(px)10 y=(px)10 w=(px)40 h=(px)40 fill=(token)"color.fill" stroke=(token)"color.stroke" stroke-width=(token)"size.sw" radius=(token)"size.r"
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
    // PushClip, FillRoundedRect, StrokeRoundedRect, PopClip
    assert_eq!(cmds.len(), 4, "expected 4 commands; got: {:?}", cmds);
    match &cmds[1] {
        SceneCommand::FillRoundedRect { radius, .. } => {
            assert!((*radius - 8.0).abs() < 0.01, "fill radius must be 8px");
        }
        other => panic!("expected FillRoundedRect first, got {other:?}"),
    }
    match &cmds[2] {
        SceneCommand::StrokeRoundedRect {
            radius,
            stroke_width,
            color,
            ..
        } => {
            assert_eq!(color.r, 0x44);
            assert!((*radius - 8.0).abs() < 0.01, "stroke radius must be 8px");
            assert!(
                (*stroke_width - 4.0).abs() < 0.01,
                "stroke-width must be 4px"
            );
        }
        other => panic!("expected StrokeRoundedRect on top, got {other:?}"),
    }
}

// ── rect: stroke only (no fill) → StrokeRect only ────────────────────

#[test]
fn rect_stroke_only_emits_stroke_rect() {
    let src = r##"zenith version=1 {
  project id="proj.rso" name="RSO"
  tokens format="zenith-token-v1" {
token id="color.stroke" type="color" value="#445566"
token id="size.sw" type="dimension" value=(px)2
  }
  styles {}
  document id="doc.rso" title="RSO" {
page id="page.rso" w=(px)100 h=(px)100 {
  rect id="rect.rso" x=(px)10 y=(px)10 w=(px)40 h=(px)40 stroke=(token)"color.stroke" stroke-width=(token)"size.sw"
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
    // PushClip, StrokeRect, PopClip
    assert_eq!(cmds.len(), 3, "expected 3 commands; got: {:?}", cmds);
    match &cmds[1] {
        SceneCommand::StrokeRect {
            color,
            stroke_width,
            ..
        } => {
            assert_eq!(color.r, 0x44);
            assert!(
                (*stroke_width - 2.0).abs() < 0.01,
                "stroke-width must be 2px"
            );
        }
        other => panic!("expected a single StrokeRect, got {other:?}"),
    }
}

#[test]
fn rect_stroke_alignment_inside_and_outside_shift_geometry() {
    // sw = 4 → inside shifts in by 2 (x+2, w-4); outside shifts out by 2.
    let doc_for = |align: &str| {
        let src = format!(
            r##"zenith version=1 {{
  project id="proj.sa" name="SA"
  tokens format="zenith-token-v1" {{
token id="color.stroke" type="color" value="#445566"
token id="size.sw" type="dimension" value=(px)4
  }}
  styles {{}}
  document id="doc.sa" title="SA" {{
page id="page.sa" w=(px)200 h=(px)200 {{
  rect id="rect.sa" x=(px)20 y=(px)20 w=(px)100 h=(px)100 stroke=(token)"color.stroke" stroke-width=(token)"size.sw" stroke-alignment="{align}"
}}
  }}
}}
"##
        );
        let doc = parse(&src);
        compile(&doc, &default_provider())
    };

    let stroke_xywh = |result: &CompileResult| -> (f64, f64, f64, f64) {
        for c in &result.scene.commands {
            if let SceneCommand::StrokeRect { x, y, w, h, .. } = c {
                return (*x, *y, *w, *h);
            }
        }
        panic!("no StrokeRect emitted");
    };

    assert_eq!(
        stroke_xywh(&doc_for("inside")),
        (22.0, 22.0, 96.0, 96.0),
        "inside must inset the box by sw/2 on each side (w - sw)"
    );
    assert_eq!(
        stroke_xywh(&doc_for("outside")),
        (18.0, 18.0, 104.0, 104.0),
        "outside must outset by sw/2"
    );
    assert_eq!(
        stroke_xywh(&doc_for("center")),
        (20.0, 20.0, 100.0, 100.0),
        "center must be unchanged"
    );
}

/// A rect with a LITERAL `radius=(px)16` (no token) must emit a
/// `FillRoundedRect` whose radius is 16.0.
#[test]
fn rect_literal_radius_emits_fill_rounded_rect() {
    let src = r##"zenith version=1 {
  project id="proj.lr" name="LR"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#112233"
  }
  styles {}
  document id="doc.lr" title="LR" {
page id="page.lr" w=(px)100 h=(px)100 {
  rect id="rect.lr" x=(px)10 y=(px)10 w=(px)40 h=(px)40 radius=(px)16 fill=(token)"color.fill"
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let cmds = &result.scene.commands;
    match cmds
        .iter()
        .find(|c| matches!(c, SceneCommand::FillRoundedRect { .. }))
    {
        Some(SceneCommand::FillRoundedRect { radius, .. }) => {
            assert!(
                (*radius - 16.0).abs() < 0.01,
                "literal radius must resolve to 16px, got {radius}"
            );
        }
        other => panic!("expected FillRoundedRect, got {other:?}"),
    }
}

/// A line with a LITERAL `stroke-width=(px)3` must produce a `StrokeLine`
/// whose `stroke_width` is 3.0.
#[test]
fn line_literal_stroke_width_resolves() {
    let src = r##"zenith version=1 {
  project id="proj.lsw" name="LSW"
  tokens format="zenith-token-v1" {
token id="color.rule" type="color" value="#94a3b8"
  }
  styles {}
  document id="doc.lsw" title="LSW" {
page id="page.lsw" w=(px)320 h=(px)200 {
  line id="line.lsw" x1=(px)40 y1=(px)100 x2=(px)280 y2=(px)100 stroke=(token)"color.rule" stroke-width=(px)3
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    match result
        .scene
        .commands
        .iter()
        .find(|c| matches!(c, SceneCommand::StrokeLine { .. }))
    {
        Some(SceneCommand::StrokeLine { stroke_width, .. }) => {
            assert_eq!(
                *stroke_width, 3.0,
                "literal stroke-width must resolve to 3px"
            );
        }
        other => panic!("expected StrokeLine, got {other:?}"),
    }
}

/// A page background, a rect fill, and an ellipse fill all referencing a
/// gradient token must emit the corresponding `*Gradient` commands with the
/// resolved stop colors and angle.
#[test]
fn gradient_fill_emits_gradient_commands() {
    let src = r##"zenith version=1 {
  project id="proj.g" name="G"
  tokens format="zenith-token-v1" {
token id="color.top" type="color" value="#112233"
token id="color.bottom" type="color" value="#445566"
token id="grad.bg" type="gradient" angle=(deg)90 {
  stop offset=0.0 color=(token)"color.top"
  stop offset=1.0 color=(token)"color.bottom"
}
  }
  styles {}
  document id="doc.g" title="G" {
page id="page.g" w=(px)640 h=(px)360 background=(token)"grad.bg" {
  rect id="rect.g" x=(px)10 y=(px)20 w=(px)100 h=(px)50 fill=(token)"grad.bg"
  ellipse id="ell.g" x=(px)10 y=(px)20 w=(px)100 h=(px)50 fill=(token)"grad.bg"
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

    // Page background gradient (full page, no opacity cascade).
    match &cmds[1] {
        SceneCommand::FillRect {
            x,
            y,
            w,
            h,
            paint: Paint::Gradient(gradient),
        } => {
            assert_eq!((*x, *y, *w, *h), (0.0, 0.0, 640.0, 360.0));
            assert_eq!(gradient.angle_deg, 90.0);
            assert_eq!(gradient.stops.len(), 2);
            assert_eq!(gradient.stops[0].offset, 0.0);
            assert_eq!(gradient.stops[0].color.r, 0x11);
            assert_eq!(gradient.stops[0].color.a, 255);
            assert_eq!(gradient.stops[1].color.r, 0x44);
        }
        other => panic!("expected FillRect gradient bg, got {other:?}"),
    }

    // Rect fill gradient.
    let has_rect_grad = cmds.iter().any(|c| {
        matches!(
            c,
            SceneCommand::FillRect { x, w, paint: Paint::Gradient(_), .. } if *x == 10.0 && *w == 100.0
        )
    });
    assert!(has_rect_grad, "expected rect gradient FillRect: {cmds:?}");

    // Ellipse fill gradient.
    let has_ell_grad = cmds.iter().any(|c| {
        matches!(
            c,
            SceneCommand::FillEllipse {
                paint: Paint::Gradient(_),
                ..
            }
        )
    });
    assert!(has_ell_grad, "expected FillEllipse gradient: {cmds:?}");
}

/// A radial gradient token (`radial=#true`) referenced via `fill=(token)` on
/// a rect must compile to a `FillRectGradient` with `GradientPaint { radial: true, … }`.
#[test]
fn radial_gradient_fill_emits_radial_gradient_paint() {
    use zenith_scene::ir::GradientPaint;
    let src = r##"zenith version=1 {
  project id="proj.rg" name="RG"
  tokens format="zenith-token-v1" {
token id="color.inner" type="color" value="#ffffff"
token id="color.outer" type="color" value="#000000"
token id="grad.radial" type="gradient" radial=#true center-x=0.5 center-y=0.5 radius=0.8 {
  stop offset=0.0 color=(token)"color.inner"
  stop offset=1.0 color=(token)"color.outer"
}
  }
  styles {}
  document id="doc.rg" title="RG" {
page id="page.rg" w=(px)100 h=(px)100 {
  rect id="rect.rg" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"grad.radial"
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
    let found = cmds.iter().find_map(|c| {
        if let SceneCommand::FillRect {
            paint: Paint::Gradient(gradient),
            ..
        } = c
        {
            Some(gradient)
        } else {
            None
        }
    });
    let grad: &GradientPaint = found.expect("expected FillRect gradient");
    assert!(grad.radial, "GradientPaint must have radial=true");
    assert_eq!(grad.center_x, Some(0.5));
    assert_eq!(grad.center_y, Some(0.5));
    assert_eq!(grad.radius_frac, Some(0.8));
    assert_eq!(grad.stops.len(), 2);
    assert_eq!(grad.stops[0].color.r, 0xff);
    assert_eq!(grad.stops[1].color.r, 0x00);
}

/// A SOLID color fill must still emit the plain `FillRect` / `FillEllipse`
/// (the gradient path must not perturb the solid path).
#[test]
fn solid_fill_unchanged_by_gradient_support() {
    let src = r##"zenith version=1 {
  project id="proj.s" name="S"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#112233"
  }
  styles {}
  document id="doc.s" title="S" {
page id="page.s" w=(px)640 h=(px)360 {
  rect id="rect.s" x=(px)0 y=(px)0 w=(px)640 h=(px)360 fill=(token)"color.fill"
  ellipse id="ell.s" x=(px)0 y=(px)0 w=(px)100 h=(px)50 fill=(token)"color.fill"
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let cmds = &result.scene.commands;
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            SceneCommand::FillRect {
                paint: Paint::Solid { .. },
                ..
            }
        )),
        "solid rect must emit FillRect: {cmds:?}"
    );
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            SceneCommand::FillEllipse {
                paint: Paint::Solid { .. },
                ..
            }
        )),
        "solid ellipse must emit FillEllipse: {cmds:?}"
    );
    assert!(
        !cmds.iter().any(|c| matches!(
            c,
            SceneCommand::FillRect {
                paint: Paint::Gradient(_),
                ..
            } | SceneCommand::FillEllipse {
                paint: Paint::Gradient(_),
                ..
            }
        )),
        "solid fills must not emit gradient commands: {cmds:?}"
    );
}
